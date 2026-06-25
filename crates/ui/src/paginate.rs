// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Pagination parameters shared across list endpoints.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A zero-based page of at most `max` items.
#[derive(TS, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PageParams {
    pub max: u32,
    pub page: u32,
}
