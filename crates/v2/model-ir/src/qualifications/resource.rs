// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::identifier::Identifier;

#[derive(Debug, PartialEq, Eq)]
pub enum CapacityKind {
    Occupancy,
    Rate,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Boundedness {
    Fixed,
    Resizable,
    Unbounded,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Capacity {
    pub name: Identifier,
    pub kind: CapacityKind,
    pub boundedness: Boundedness,
    // TODO(johanpel): consider introducing this, but for now everything is u64
    // value_type: ValueType
}

/// Resource qualification.
///
/// An entity qualifies as a resource if it satisfies the following constraints:
///
/// 1. It has at least one [`Capacity`].
/// 2. It is an FSM.
/// 3. Depending on the capacities, its FSM topology is:
///    - If none of its capacities have [`Boundedness::Resizable`]:
///       - entry -> init -> operating -> finalizing -> exit
///    - If at least one of its capacities have [`Boundedness::Resizable`]:
///       - entry -> init -> operating -> finalizing -> exit
///       - operating -> resizing -> operating
/// 4. TODO(johanpel): feels like im forgetting something
#[derive(Debug, PartialEq, Eq)]
pub struct Resource {
    pub capacities: Vec<Capacity>,
}
