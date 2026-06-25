// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generic entity-list query over any application FSM type.

use std::collections::{HashMap, HashSet};

use quent_analyzer::{
    AnalyzerResult, Model,
    fsm::{FsmUsages, collection::FsmCollection},
    resource::Usage,
    resource::tree::ResourceTreeNode,
};
use quent_time::{TimeNanoSec, TimeUnixNanoSec, span::SpanUnixNanoSec, to_nanosecs};
use quent_ui::{
    FiniteStateMachine,
    entities::{
        request::{EntityListEntry, EntityListFilter, EntityScope, EntitySortKey, Sort, SortDir},
        response::{BulkEntityListResponseEntry, EntityListResponse},
    },
    paginate::PageParams,
};
use uuid::Uuid;

/// Resolve an entity scope to the leaf resource IDs it covers.
///
/// A single resource yields itself; a group yields its leaf resources of the
/// requested type.
pub fn resolve_scope(model: &impl Model, scope: &EntityScope) -> AnalyzerResult<HashSet<Uuid>> {
    match scope {
        EntityScope::Resource { resource_id } => {
            model.resource(*resource_id)?;
            Ok([*resource_id].into_iter().collect())
        }
        EntityScope::ResourceGroup {
            resource_group_id,
            resource_type_name,
        } => {
            let tree = ResourceTreeNode::try_new(model, *resource_group_id)?;
            Ok(tree
                .iter_leaf_ids()
                .filter(|&id| {
                    model
                        .resource(id)
                        .is_ok_and(|r| r.type_name() == resource_type_name)
                })
                .collect())
        }
    }
}

/// List the FSMs that use the scope within the window, ranked and paged.
///
/// Filters by type name and by `min_usage_s`, ranks by the sort key with the
/// entity UUID ascending as the stable tiebreaker, sets `total` to the matched
/// count before paging, and converts the requested page to UI FSMs.
pub fn list_entities<M>(
    model: &M,
    scope_resources: Option<&HashSet<Uuid>>,
    window: SpanUnixNanoSec,
    filter: &EntityListFilter,
    sort: Sort,
    page: Option<PageParams>,
    epoch: TimeUnixNanoSec,
) -> AnalyzerResult<EntityListResponse>
where
    M: FsmCollection,
    M::Fsm: for<'a> FsmUsages<'a>,
{
    let ranked = model
        .fsms()
        .filter_map(|f| entry_matches(f, scope_resources, window, filter).map(|m| (f, m)))
        .collect();
    finalize(ranked, sort, page, epoch)
}

/// List entities for several queries in a single pass over the model's FSMs.
///
/// Each query's window and scope are resolved up front; a per-query resolution
/// failure becomes an [`Error`](BulkEntityListResponseEntry::Error) entry
/// without affecting its peers. Every FSM is then visited once and scored
/// against every resolved query.
pub fn bulk_list_entities<M>(
    model: &M,
    entries: HashMap<String, EntityListEntry>,
    epoch: TimeUnixNanoSec,
) -> HashMap<String, BulkEntityListResponseEntry>
where
    M: FsmCollection + Model,
    M::Fsm: for<'a> FsmUsages<'a>,
{
    let mut out: HashMap<String, BulkEntityListResponseEntry> = HashMap::new();

    // Resolve window + scope per entry; record failures and keep the rest.
    let mut resolved: Vec<(
        String,
        EntityListEntry,
        SpanUnixNanoSec,
        Option<HashSet<Uuid>>,
    )> = Vec::new();
    for (key, entry) in entries {
        let window = match entry.window.try_into_span(epoch) {
            Ok(window) => window,
            Err(e) => {
                out.insert(key, error_entry(e));
                continue;
            }
        };
        let scope = match &entry.filter.scope {
            Some(scope) => match resolve_scope(model, scope) {
                Ok(resources) => Some(resources),
                Err(e) => {
                    out.insert(key, error_entry(e));
                    continue;
                }
            },
            None => None,
        };
        resolved.push((key, entry, window, scope));
    }

    // Single pass: score each FSM against every resolved query.
    let mut buckets: Vec<Vec<(&M::Fsm, TimeNanoSec)>> =
        (0..resolved.len()).map(|_| Vec::new()).collect();
    for fsm in model.fsms() {
        for (i, (_, entry, window, scope)) in resolved.iter().enumerate() {
            if let Some(metric) = entry_matches(fsm, scope.as_ref(), *window, &entry.filter) {
                buckets[i].push((fsm, metric));
            }
        }
    }

    for ((key, entry, _, _), ranked) in resolved.into_iter().zip(buckets) {
        let result = finalize(ranked, entry.sort, entry.page, epoch);
        let response = match result {
            Ok(response) => BulkEntityListResponseEntry::Ok(response),
            Err(e) => error_entry(e),
        };
        out.insert(key, response);
    }

    out
}

fn error_entry(e: impl std::fmt::Display) -> BulkEntityListResponseEntry {
    BulkEntityListResponseEntry::Error {
        message: e.to_string(),
    }
}

/// The ranking metric for an FSM under one query, or `None` if it does not
/// match (wrong type, out of scope, or below `min_usage_s`).
fn entry_matches<'a, F>(
    fsm: &'a F,
    scope: Option<&HashSet<Uuid>>,
    window: SpanUnixNanoSec,
    filter: &EntityListFilter,
) -> Option<TimeNanoSec>
where
    F: FsmUsages<'a>,
{
    if filter
        .entity_type_name
        .as_deref()
        .is_some_and(|name| fsm.type_name() != name)
    {
        return None;
    }
    let metric = usage_metric(fsm, scope, window)?;
    let min_usage = filter.min_usage_s.map(to_nanosecs);
    min_usage.is_none_or(|t| metric >= t).then_some(metric)
}

/// Sort the scored candidates, slice the page, and convert to UI FSMs.
fn finalize<'a, F>(
    mut ranked: Vec<(&'a F, TimeNanoSec)>,
    sort: Sort,
    page: Option<PageParams>,
    epoch: TimeUnixNanoSec,
) -> AnalyzerResult<EntityListResponse>
where
    F: FsmUsages<'a>,
{
    ranked.sort_by(|(fa, ma), (fb, mb)| {
        let by_key = match sort.key {
            EntitySortKey::UsageDuration => ma.cmp(mb),
        };
        let by_key = match sort.dir {
            SortDir::Asc => by_key,
            SortDir::Desc => by_key.reverse(),
        };
        by_key.then_with(|| fa.id().cmp(&fb.id()))
    });

    let total = ranked.len() as u32;

    let page_iter: Box<dyn Iterator<Item = (&F, TimeNanoSec)>> = match page {
        Some(p) => Box::new(
            ranked
                .into_iter()
                .skip((p.page as usize) * (p.max as usize))
                .take(p.max as usize),
        ),
        None => Box::new(ranked.into_iter()),
    };

    let items = page_iter
        .map(|(f, _)| FiniteStateMachine::try_from_fsm(f, epoch))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(EntityListResponse { items, total })
}

/// The ranking metric: the longest single usage span within the window, on a
/// scope resource (or any resource when `scope` is `None`).
///
/// Returns `None` only when a `scope` is set and the FSM has no usage on it —
/// such entities are out of scope. With no scope every FSM is ranked, scoring
/// `0` when it has no usage in the window.
fn usage_metric<'a, F>(
    fsm: &'a F,
    scope: Option<&HashSet<Uuid>>,
    window: SpanUnixNanoSec,
) -> Option<TimeNanoSec>
where
    F: FsmUsages<'a>,
{
    let longest = fsm
        .usages_with_state_names()
        .filter(|(_, u)| scope.is_none_or(|s| s.contains(&u.resource_id())))
        .filter_map(|(_, u)| u.span().intersection(&window))
        .map(|s| s.duration())
        .max();

    match scope {
        Some(_) => longest,
        None => Some(longest.unwrap_or(0)),
    }
}
