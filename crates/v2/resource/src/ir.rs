// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Serde data carried under the `"Resource"` convention key on entities.

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceData {
    pub capacities: Vec<CapacityData>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityData {
    pub name: String,
    pub kind: CapacityKindData,
    pub boundedness: BoundednessData,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapacityKindData {
    Occupancy,
    Rate,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundednessData {
    Fixed,
    Resizable,
    Unbounded,
}
