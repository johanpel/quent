// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::qualifications::{fsm::Fsm, resource::Resource};

pub mod fsm;
pub mod resource;

/// IR of entity qualifications
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QualificationKind {
    /// Finite-State-Machine
    ///
    /// The entity emits events in an order prescribed by a topology of states
    /// and transitions.
    Fsm,
    /// The entity qualifies  as a Resource.
    ///
    /// It is an FSM that goes through states determined by its capacities.
    Resource,
}

/// IR of a Qualification of an [`crate::ir::entity::Entity`].
///
/// Qualifications are constraints an entity's events must satisfy. This may
/// include constraints on any property an entities' events, either their
/// fields or their order. If the constraints of a qualification "X" are
/// satisfied, an entity is said to "qualify" as an "X".
///
/// Through these requirements, specialized semantics can be added over plain
/// event-emitting entities. These specializations can be used to e.g. generate
/// instrumentation API code in a certain way. For example, by qualifying as a
/// Finite-State-Machine, an entity handle can be specialized to follow the
/// Typestate pattern which prevents illegal transitions at compile-time.
///
/// Qualifications can depend on each other. For example, in order for an entity
/// to qualify as a resource, it must also qualify as an FSM. The resource
/// qualification then puts additional constraints on the FSMs topology.
///
/// See [`QualificationKind`] for supported qualifications.
///
/// Qualifications are somewhat similar in spirit to Rust traits, but are named
/// differently to prevent the obvious terminology clashing. Qualifications and
/// its terminology is mostly visible in the IR and code generation to capture
/// constraints.
///
/// # Rust modeling API
///
/// In the Rust modeling API, entities can be marked as having a qualification
/// of a certain type through attributes. For example `#[quent(fsm(...))] sets
/// the [`Fsm`] qualification, and #[quent(resource_group(...))] the
/// [`ResourceGroup`] qualification.
///
/// Certain constraints of qualifications can ONLY be set through two
/// mechanisms:
///
/// 1. Through arguments to the #[quent(...)] attribute of that qualification
///     - e.g. #[quent(fsm(...))] to specify FSM topology
/// 2. Through named fields of enum variants
///     - e.g. `FooVariant { payload: X, parent: EntityRef<...> }`. Here, the
///       constraint that one event must declare which entity is its parent is
///       met through the existence of the `parent` field.
///
/// Qualifications CANNOT require types other than the entity declaration type
/// to capture the necessary properties, because the `#[derive(Entity)]` macro
/// can only inspect the tokens stream of the type it is applied to, but not of
/// other types. If such properties exist, they must be expressed and validated
/// through constraints imposed by the Rust type system and the compiler's type
/// system validation checks instead.
#[derive(Debug, PartialEq, Eq)]
pub enum Qualification {
    Fsm(Fsm),
    Resource(Resource),
}

impl Qualification {
    pub fn kind(&self) -> QualificationKind {
        match self {
            Qualification::Fsm(_) => QualificationKind::Fsm,
            Qualification::Resource(_) => QualificationKind::Resource,
        }
    }
}
