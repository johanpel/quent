// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Umbrella request bundling several UI queries into one round-trip.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    entities::{request::EntityListEntry, response::BulkEntityListResponse},
    timeline::{request::TimelineRequest, response::BulkTimelinesResponse},
};

/// A single UI refresh: any combination of timeline and entity-list queries,
/// sharing one set of application parameters.
#[derive(TS, Debug, Clone, Serialize, Deserialize)]
pub struct BulkRequest<GlobalParams, TimelineParams> {
    /// Application parameters shared by every query in the bundle.
    pub app_params: GlobalParams,
    /// Timeline queries, keyed by a caller-chosen id.
    pub timelines: Option<HashMap<String, TimelineRequest<TimelineParams>>>,
    /// Entity-list queries, keyed by a caller-chosen id.
    pub entities: Option<HashMap<String, EntityListEntry>>,
}

/// Response to a [`BulkRequest`]; each section is present iff requested.
#[derive(TS, Debug, Serialize)]
pub struct BulkResponse {
    pub timelines: Option<BulkTimelinesResponse>,
    pub entities: Option<BulkEntityListResponse>,
}
