// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generic entity-list query over any application FSM type.

use std::collections::HashSet;

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
        request::{EntityListFilter, EntityScope, EntitySortKey, Sort, SortDir},
        response::EntityListResponse,
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
/// `keep` is an application predicate applied before any other filter, for
/// filters the generic contract does not model (e.g. by operator). Filters by
/// type name and by `min_usage_s`, ranks by the sort key with the entity UUID
/// ascending as the stable tiebreaker, sets `total` to the matched count before
/// paging, and converts the requested page to UI FSMs.
#[allow(clippy::too_many_arguments)]
pub fn list_entities<M, P>(
    model: &M,
    keep: P,
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
    P: Fn(&M::Fsm) -> bool,
{
    let ranked = model
        .fsms()
        .filter(|f| keep(f))
        .filter_map(|f| entry_matches(f, scope_resources, window, filter).map(|m| (f, m)))
        .collect();
    finalize(ranked, sort, page, epoch)
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
