// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use serde::Serialize;
use ts_rs::TS;

use crate::FiniteStateMachine;

/// A ranked, paged list of entities.
#[derive(TS, Debug, Clone, Serialize)]
pub struct EntityListResponse {
    // TODO(johanpel): generalize to other entity types; only FSMs are
    // represented today.
    pub items: Vec<FiniteStateMachine>,
    /// The count of entities matching the filter before paging.
    pub total: u32,
}

/// A single entry in a bulk entity-list response.
#[derive(TS, Debug, Clone, Serialize)]
#[serde(tag = "status")]
pub enum BulkEntityListResponseEntry {
    #[serde(rename = "ok")]
    Ok(EntityListResponse),
    #[serde(rename = "error")]
    Error {
        /// A message describing the error.
        message: String,
    },
}

/// Response for a bulk entity-list request, keyed by the request entry ids.
#[derive(TS, Debug, Clone, Serialize)]
pub struct BulkEntityListResponse {
    pub entries: HashMap<String, BulkEntityListResponseEntry>,
}
