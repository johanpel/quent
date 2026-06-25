// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Functional test of `UiAnalyzer::list_entities` over the fixed scenario.
//!
//! Emits the fixed 7-second telemetry to a temp dir, imports it into a
//! `SimulatorUiAnalyzer`, and asserts the entity-list query against the
//! scenario's known tasks and their resource usage.
//!
//! Ground truth: every task holds its memory for the `computing→exit` span
//! (0.75s). `MEMORY_W0` is used by 8 tasks, `MEMORY_W1` by 4. Because the spans
//! are equal, results are ordered by the UUID tiebreaker.

use quent_exporter::{ExporterOptions, FileSystemExporterOptions, FileSystemFormat};
use quent_query_engine_analyzer::ui::UiAnalyzer;
use quent_query_engine_fixed as fixed;
use quent_query_engine_ui::QueryFilter;
use quent_simulator_analyzer::SimulatorUiAnalyzer;
use quent_simulator_instrumentation::{Simulator, SimulatorContext};
use quent_ui::entities::request::{
    BulkEntityListRequest, EntityListEntry, EntityListFilter, EntityListRequest, EntityScope,
    EntitySortKey, Sort, SortDir, TimeWindow,
};
use quent_ui::entities::response::{
    BulkEntityListResponse, BulkEntityListResponseEntry, EntityListResponse,
};
use quent_ui::paginate::PageParams;
use std::collections::HashMap;
use uuid::Uuid;

// Tasks using MEMORY_W0, in ascending UUID order (the tiebreaker).
const MEMORY_W0_TASKS: [Uuid; 8] = [
    fixed::TASK_0,
    fixed::TASK_1,
    fixed::TASK_4,
    fixed::TASK_5,
    fixed::TASK_8,
    fixed::TASK_9,
    fixed::TASK_10,
    fixed::TASK_11,
];

// Tasks using MEMORY_W1, in ascending UUID order.
const MEMORY_W1_TASKS: [Uuid; 4] = [fixed::TASK_2, fixed::TASK_3, fixed::TASK_6, fixed::TASK_7];

// All 12 tasks ranked by longest usage, descending. TASK_6 and TASK_7 sort
// last: their `computing` is cut short by a `sending` transition, so their
// longest single usage span is 0.5s (the send) versus 0.75s for every other
// task. The remaining ten tie at 0.75s and fall back to ascending UUID order.
const ALL_TASKS_RANKED: [Uuid; 12] = [
    fixed::TASK_0,
    fixed::TASK_1,
    fixed::TASK_2,
    fixed::TASK_3,
    fixed::TASK_4,
    fixed::TASK_5,
    fixed::TASK_8,
    fixed::TASK_9,
    fixed::TASK_10,
    fixed::TASK_11,
    fixed::TASK_6,
    fixed::TASK_7,
];

/// Emit the fixed scenario to a temp dir and build an analyzer from it.
fn fixed_analyzer() -> SimulatorUiAnalyzer {
    let tmp = tempfile::tempdir().unwrap();
    let context_id = {
        let ctx = SimulatorContext::try_new(Some(ExporterOptions::FileSystem(
            FileSystemExporterOptions {
                format: FileSystemFormat::Postcard,
                root: tmp.path().to_path_buf(),
            },
        )))
        .unwrap();
        let id = ctx.id();
        fixed::emit(&ctx);
        id
        // ctx dropped here, flushing all events to disk.
    };

    // Per-observer exporters write entity subdirectories under the context dir;
    // import_events reconstructs the umbrella event stream from them.
    let dir = tmp.path().join(context_id.to_string());
    let importer = Simulator::import_events(&dir, FileSystemFormat::Postcard).unwrap();

    // try_new drains the importer into an in-memory model before `tmp` drops.
    SimulatorUiAnalyzer::try_new(fixed::ENGINE, importer).unwrap()
}

/// An entity-list entry over the whole query window, ranked by usage duration.
fn entry(
    scope: Option<EntityScope>,
    min_usage_s: Option<f64>,
    page: Option<PageParams>,
) -> EntityListEntry {
    EntityListEntry {
        window: TimeWindow {
            start: 0.0,
            end: 7.0,
        },
        filter: EntityListFilter {
            scope,
            entity_type_name: None,
            min_usage_s,
        },
        sort: Sort {
            key: EntitySortKey::UsageDuration,
            dir: SortDir::Desc,
        },
        page,
    }
}

fn request_scoped(
    scope: Option<EntityScope>,
    min_usage_s: Option<f64>,
    page: Option<PageParams>,
) -> EntityListRequest<QueryFilter> {
    EntityListRequest {
        entry: entry(scope, min_usage_s, page),
        app_params: QueryFilter {
            query_id: fixed::QUERY,
        },
    }
}

/// A request scoped to a single resource.
fn request(
    resource_id: Uuid,
    min_usage_s: Option<f64>,
    page: Option<PageParams>,
) -> EntityListRequest<QueryFilter> {
    request_scoped(
        Some(EntityScope::Resource { resource_id }),
        min_usage_s,
        page,
    )
}

fn ids(resp: &EntityListResponse) -> Vec<Uuid> {
    resp.items.iter().map(|fsm| fsm.id).collect()
}

fn ok<'a>(resp: &'a BulkEntityListResponse, key: &str) -> &'a EntityListResponse {
    match resp.entries.get(key).expect("missing key") {
        BulkEntityListResponseEntry::Ok(response) => response,
        BulkEntityListResponseEntry::Error { message } => {
            panic!("entry '{key}' errored: {message}")
        }
    }
}

#[test]
fn lists_all_tasks_on_a_resource_ranked_by_uuid_tiebreak() {
    let analyzer = fixed_analyzer();
    let resp = analyzer
        .list_entities(request(fixed::MEMORY_W0, None, None))
        .unwrap();

    assert_eq!(resp.total, 8);
    assert_eq!(ids(&resp), MEMORY_W0_TASKS);
}

#[test]
fn no_scope_lists_every_entity() {
    let analyzer = fixed_analyzer();
    let resp = analyzer
        .list_entities(request_scoped(None, None, None))
        .unwrap();

    // Every task is ranked regardless of which resource it used.
    assert_eq!(resp.total, 12);
    assert_eq!(ids(&resp), ALL_TASKS_RANKED);
}

#[test]
fn scope_restricts_to_the_resources_tasks() {
    let analyzer = fixed_analyzer();
    let resp = analyzer
        .list_entities(request(fixed::MEMORY_W1, None, None))
        .unwrap();

    assert_eq!(resp.total, 4);
    assert_eq!(ids(&resp), MEMORY_W1_TASKS);
}

#[test]
fn min_usage_filter_includes_or_excludes_by_threshold() {
    let analyzer = fixed_analyzer();

    // Each task's memory usage is 0.75s; a lower threshold keeps all.
    let kept = analyzer
        .list_entities(request(fixed::MEMORY_W0, Some(0.5), None))
        .unwrap();
    assert_eq!(kept.total, 8);

    // A threshold above 0.75s drops every task.
    let dropped = analyzer
        .list_entities(request(fixed::MEMORY_W0, Some(1.0), None))
        .unwrap();
    assert_eq!(dropped.total, 0);
    assert!(dropped.items.is_empty());
}

#[test]
fn pagination_slices_the_ranked_set_with_stable_total() {
    let analyzer = fixed_analyzer();

    let page0 = analyzer
        .list_entities(request(
            fixed::MEMORY_W0,
            None,
            Some(PageParams { max: 3, page: 0 }),
        ))
        .unwrap();
    assert_eq!(page0.total, 8);
    assert_eq!(ids(&page0), MEMORY_W0_TASKS[0..3]);

    let page2 = analyzer
        .list_entities(request(
            fixed::MEMORY_W0,
            None,
            Some(PageParams { max: 3, page: 2 }),
        ))
        .unwrap();
    assert_eq!(page2.total, 8);
    assert_eq!(ids(&page2), MEMORY_W0_TASKS[6..8]);
}

#[test]
fn bulk_runs_every_entry_in_one_pass() {
    let analyzer = fixed_analyzer();

    let request = BulkEntityListRequest {
        entries: HashMap::from([
            (
                "w0".to_string(),
                entry(
                    Some(EntityScope::Resource {
                        resource_id: fixed::MEMORY_W0,
                    }),
                    None,
                    None,
                ),
            ),
            (
                "w1".to_string(),
                entry(
                    Some(EntityScope::Resource {
                        resource_id: fixed::MEMORY_W1,
                    }),
                    None,
                    None,
                ),
            ),
            ("all".to_string(), entry(None, None, None)),
        ]),
        app_params: QueryFilter {
            query_id: fixed::QUERY,
        },
    };

    let resp = analyzer.bulk_list_entities(request).unwrap();
    assert_eq!(resp.entries.len(), 3);

    // Each keyed entry matches its standalone single-request result.
    assert_eq!(ids(ok(&resp, "w0")), MEMORY_W0_TASKS);
    assert_eq!(ids(ok(&resp, "w1")), MEMORY_W1_TASKS);
    assert_eq!(ids(ok(&resp, "all")), ALL_TASKS_RANKED);
}
