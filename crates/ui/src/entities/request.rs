// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_time::{TimeError, TimeSec, TimeUnixNanoSec, span::SpanUnixNanoSec, to_nanosecs};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::paginate::PageParams;

/// Selects which entities are listed: those that have at least one resource
/// usage on the scope.
#[derive(TS, Debug, Clone, Serialize, Deserialize)]
pub enum EntityScope {
    /// Entities with a usage of this resource.
    Resource { resource_id: Uuid },
    /// Entities with a usage of any leaf resource of `resource_type_name` within
    /// this group.
    ResourceGroup {
        resource_group_id: Uuid,
        resource_type_name: String,
    },
}

/// A time window in seconds relative to the query epoch.
#[derive(TS, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TimeWindow {
    pub start: TimeSec,
    pub end: TimeSec,
}

impl TimeWindow {
    /// Resolve to an absolute span by offsetting from `epoch`.
    pub fn try_into_span(self, epoch: TimeUnixNanoSec) -> Result<SpanUnixNanoSec, TimeError> {
        SpanUnixNanoSec::try_new(
            epoch + to_nanosecs(self.start),
            epoch + to_nanosecs(self.end),
        )
    }
}

/// Entity filters. Every set field must match; a `None` field does not filter.
#[derive(TS, Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityListFilter {
    /// Restrict to entities with a usage on this scope. `None` lists entities
    /// regardless of which resource they used.
    pub scope: Option<EntityScope>,
    pub entity_type_name: Option<String>,
    /// Keep only entities with resource usages longer than this threshold. Note
    /// that only Fsm-type entities can have usages.
    pub min_usage_s: Option<TimeSec>,
}

/// The key entities are ranked by.
#[derive(TS, Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EntitySortKey {
    /// The longest single resource-usage span within the window — on the scope
    /// resource if one is set, otherwise on any resource.
    UsageDuration,
}

#[derive(TS, Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(TS, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Sort {
    pub key: EntitySortKey,
    pub dir: SortDir,
}

/// A single entity-list query.
#[derive(TS, Debug, Clone, Serialize, Deserialize)]
pub struct EntityListEntry<EntryParams> {
    pub window: TimeWindow,
    pub filter: EntityListFilter,
    pub sort: Sort,
    /// `None` returns the full filtered set.
    pub page: Option<PageParams>,
    /// Per-query application parameters, e.g. an operator filter.
    pub application: EntryParams,
}

/// Parameters for listing entities.
#[derive(TS, Debug, Clone, Serialize, Deserialize)]
pub struct EntityListRequest<GlobalParams, EntryParams> {
    pub entry: EntityListEntry<EntryParams>,
    /// Global application parameters shared by the query, e.g. the query id.
    pub app_params: GlobalParams,
}
