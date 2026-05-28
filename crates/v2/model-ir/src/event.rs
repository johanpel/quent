// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::fmt::Debug;

use crate::{convention::Convention, data_type::DataType, identifier::Identifier};

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Cardinality {
    /// The event can be emitted zero or one time.
    Once,
    /// The event can be emitted zero or multiple times.
    Multi,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EntityRefTarget {
    /// The entity reference targets any entity.
    Any,
    /// The entity reference targets one specific entity type by name.
    ///
    /// An entity with this name must be present in the
    /// [`crate::Model::entities`] field.
    Specific(Identifier),
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EntityRefRole {
    /// No specific role.
    Plain,
    /// The core tree-forming role.
    Scope,
    /// A role defined by an arbitrary type that is a record.
    User(Identifier),
}

/// The type of an event field.
///
/// This differs from [`DataType`] because event fields are the mechanism
/// through which event data with Quent core semantics are carried.
// TODO(johanpel): consider dropping this and put everything in DataType. The
// reason it is kept here for now, is that it does provide a very simple
// structured pattern as to where things can be figured out. This may simplify
// and provide certain opportunities for code generation for cross-lang bridges
// as well as analyzers. Another advantage exists for pure Rust flows, where
// validation of constraints that cross multiple types and that are non-trivial
// / unwieldy to express through the type system can take place from within a
// derive macros, such that in pure Rust workflows using only core features, no
// build.rs is required to validate anything, e.g. "only one scope-roled ref per
// entity exists".
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EventFieldType {
    /// An application-specific event payload field.
    Payload(DataType),
    /// A reference to another entity.
    EntityRef {
        /// The type of the data associated with the role of this reference
        role_type: EntityRefRole,
        /// The entity type this reference can target.
        entity_type: EntityRefTarget,
    },
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventField {
    /// The name of the event field.
    pub name: Identifier,
    /// Potential documentation.
    pub docs: Option<String>,
    /// The type of the event field.
    pub ty: EventFieldType,
    /// Convention-specific metadata attached to this event field.
    pub conventions: Vec<Convention>,
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Event {
    /// The name of the event.
    pub name: Identifier,
    /// Potential documentation.
    pub docs: Option<String>,
    /// The [`Cardinality`] of the event.
    pub cardinality: Cardinality,
    /// The fields of the event.
    pub payload: Vec<EventField>,
    /// Convention-specific metadata attached to this event.
    pub conventions: Vec<Convention>,
}
