// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The Quent built-in resource constraint.

use quent_schema::{Annotations, Identifier};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// The data a `quent.resource.v1` constraint carries.
///
/// A resource is an entity with a finite supply that other entities claim
/// against. The same constraint is placed on several schema elements. Each
/// variant says what role the annotated element plays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    /// Placed on a resource entity, declaring it a resource exposing
    /// `capacities`. An empty list denotes a unit resource: one claimed for
    /// mutual exclusion, with no quantified dimension.
    Definition { capacities: SmallVec<[Capacity; 1]> },
    /// Placed on the record type conveying a usage of resource `resource`. The
    /// usage is held for the duration of a state of the FSM entity claiming it.
    Usage { resource: Identifier },
    /// Placed on the record type conveying the bounds of resource `resource`.
    /// This record is used within the resource's own FSM.
    Bounds { resource: Identifier },
}

impl Resource {
    /// The constraint name under which this data is carried.
    pub const NAME: &'static str = "quent.resource.v1";

    /// The resource role declared on `annotations`, if any.
    pub fn from_annotations(annotations: &Annotations) -> Option<Self> {
        serde_json::from_str(annotations.constraint(Self::NAME)?.data()?).ok()
    }
}

/// A named, quantified dimension of a resource that usages claim against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capacity {
    /// Unique within the resource.
    name: Identifier,
    kind: CapacityKind,
}

impl Capacity {
    pub fn new(name: Identifier, kind: CapacityKind) -> Self {
        Self { name, kind }
    }

    pub fn name(&self) -> &Identifier {
        &self.name
    }

    pub fn kind(&self) -> CapacityKind {
        self.kind
    }
}

/// How a claimed quantity relates to the span over which it is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapacityKind {
    /// A quantity held for the duration of a usage span, e.g. bytes of memory.
    Occupancy,
    /// A total quantity processed over a usage span, e.g. messages over a
    /// channel. Dividing it by the span's duration yields a perceived rate, not
    /// the true rate, which Quent cannot observe.
    Rate,
}
