// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

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

/// A single entity-list query, without application parameters.
#[derive(TS, Debug, Clone, Serialize, Deserialize)]
pub struct EntityListEntry {
    pub window: TimeWindow,
    pub filter: EntityListFilter,
    pub sort: Sort,
    /// `None` returns the full filtered set.
    pub page: Option<PageParams>,
}

/// Parameters for listing entities.
#[derive(TS, Debug, Clone, Serialize, Deserialize)]
pub struct EntityListRequest<AppParams> {
    pub entry: EntityListEntry,
    /// Application-specific request parameters, e.g. for filtering.
    pub app_params: AppParams,
}

/// Parameters for listing entities for several queries at once.
#[derive(TS, Debug, Clone, Serialize, Deserialize)]
pub struct BulkEntityListRequest<AppParams> {
    /// The queries, keyed by a caller-chosen id echoed in the response.
    pub entries: HashMap<String, EntityListEntry>,
    /// Application-specific request parameters shared by all entries.
    pub app_params: AppParams,
}
