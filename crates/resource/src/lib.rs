// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The Quent built-in resource constraint.

use quent_schema::{Annotations, Identifier};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// The data a `quent.resource.v1` constraint carries.
///
/// A resource is an entity with one or more capacities that other entities can claim.
///
/// This data is placed on several schema elements. Each variant explains the
/// role of the annotated element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    /// Placed on a resource entity, declaring it a resource exposing
    /// `capacities`. An empty list denotes a unit resource.
    // Common-case: one capacity.
    Definition { capacities: SmallVec<[Capacity; 1]> },
    /// Placed on the record type conveying the bounds of resource `resource`.
    ///
    /// This record is used within the resource's own FSM.
    Bounds { resource: Identifier },
    /// Placed on the record type conveying a usage of resource `resource`.
    ///
    /// The usage is perceived as held for the duration of the FSM state of the
    /// FSM entity claiming it. This record type can only be used on FSM state
    /// transition events besides exit to ensure the usage is released.
    Usage { resource: Identifier },
}

impl Resource {
    /// The constraint name under which the data is carried.
    pub const NAME: &'static str = "quent.resource.v1";

    /// Deserialize [`Self`] from `annotations`, if it exists.
    pub fn from_annotations(annotations: &Annotations) -> Option<Self> {
        serde_json::from_str(annotations.constraint(Self::NAME)?.data()?).ok()
    }
}

/// A named, quantified dimension of a resource that usages claim against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capacity {
    /// The unique name of the capacity within the resource.
    name: Identifier,
    /// The type of capacity.
    kind: CapacityKind,
    /// Whether the capacity is bounded. If all capacities of a resource are
    /// unbounded, then no bounds need to be set, so no bound record type should
    /// exist, and the FSM transition into "operating" shall not have a bounds
    /// argument.
    bounded: bool,
}

impl Capacity {
    pub fn new(name: Identifier, kind: CapacityKind, bounded: bool) -> Self {
        Self {
            name,
            kind,
            bounded,
        }
    }

    pub fn name(&self) -> &Identifier {
        &self.name
    }

    pub fn kind(&self) -> CapacityKind {
        self.kind
    }

    pub fn bounded(&self) -> bool {
        self.bounded
    }
}

/// How a capacity relates to the span over which it is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapacityKind {
    /// A quantity held for the duration of a usage span, e.g. bytes of a
    /// memory.
    Occupancy,
    /// A total quantity processed over a usage span, e.g. bytes sent over a
    /// channel. Dividing it by the span's duration yields a perceived rate. The
    /// true rate may be hidden from Quent (e.g. when the work is performed
    /// asynchronously).
    Rate,
}
