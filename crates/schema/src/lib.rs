// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! # Schema of Application Event Models
//!
//! This module defines types of core concepts necessary to express the schema
//! of an application event model. The schema captures the information necessary
//! to write and read all events without further interpretation.
//!
//! ## Core Concepts
//!
//! The schema core concepts are:
//!
//! - [`Identifier`]: defines the name of things
//! - [`crate::data_type::DataType`]: defines common data types (bool, integer,
//!   string, etc.) plus Quent-specific types, such as:
//!   - [`crate::data_type::DataType::EntityRef`]
//!   - [`crate::data_type::DataType::DynamicRecord`]
//! - [`event::Event`]: defines an event type that applications can emit
//! - [`entity::Entity`]: defines a uniquely identifiable type of thing that
//!   emits a set of related events
//! - [`Schema`]: defines a type of uniquely identifiable collection of entities
//!   that are somehow related, the top-level of an application event model
//! - [`convention::Convention`]: defines opaque metadata for "conventions" (see
//!   below)
//!
//! ## Purpose
//!
//! The schema is leveraged for (cross-language) code generation, model
//! validation, and serialization.
//!
//! Code generation involves generating cross-language compatible bridge code,
//! e.g. for C++ through a CXX bridge, or for Python through PyO3.
//!
//! Model validation involves checking certain constraints placed on a model
//! captured in the schema. These might need to be met to even succeed in
//! constructing the schema in-memory using this crate, e.g. by ensuring that
//! [`Identifier`]s are accepted by the prescribed grammar. Some constraints may
//! be about event attributes or order, e.g. that FSM states are reachable from
//! the entry transition and an exit transition is reachable from all states.
//! The latter is an example of a constraint that is not expressed through a
//! core schema concept, because it does not contribute to the ability to write
//! or read events.
//!
//! Serialization involves the ability to store a model, which can be leveraged
//! for model re-use, sharing, and archival purposes.
//!
//! ## Conventions
//!
//! The schema is kept as minimal as possible in order to prevent contamination
//! and complexity from concerns imbued by application-specific semantics,
//! conventions, or constraints, as well as concerns from modeling APIs or DSLs,
//! as well as concerns from code generation flows for either instrumentation or
//! analysis APIs.
//!
//! However, all of these types of concerns can be addressed by adding opaque
//! metadata to most core schema types as "conventions". Especially those
//! conventions that constrain the model in certain ways to ensure logical
//! soundness, or that can be leveraged to produce a more user-friendly
//! instrumentation API, can be added by modeling APIs or DSL parsers to feed
//! through the schema into code generation and onward.
//!
//! Some example of built-in provided conventions by Quent include:
//! - FSMs: constrains the sequence of events to adhere to a certain topology,
//!   plus code generation can apply a typestate pattern to entity event handles
//! - Reference roles: constrains that an event with a reference of the
//!   tree-forming "Scope" role can only appear once in an entity event, and
//!   that entities form a tree.
//!
//! Note that this approach promotes a stronger guarantee against breaking
//! changes. For example, even if a new convention is added, but code generation
//! does not yet support that convention, it will still be able to produce an
//! instrumentation API that allows users to emit events that may have been
//! defined as a result of the new convention. Users may not yet get the benefit
//! of some potential elegant type-safe API better expressing these constraints,
//! but everything will "still work".
//!
//! In order to validate potential constraints of conventions against the
//! schema, a lightweight canonical mechanism exists for validating conventions
//! in the `quent-convention` crate. It is strongly recommended to perform this
//! validation after constructing the schema from any source that isn't
//! inherently guaranteed to validate.
//!
//! ## Binary Format
//!
//! There is no stable binary format for schemas yet. As a stop-gap solution for
//! serializing schemas, this crate has a `serde` feature.

use crate::{convention::Convention, entity::Entity, identifier::Identifier, record::Record};

pub mod convention;
pub mod data_type;
pub mod entity;
pub mod event;
pub mod identifier;
pub mod record;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Schema {
    /// The name of the model.
    pub name: Identifier,
    /// Potential documentation that can be added in code generation.
    pub docs: Option<String>,
    /// The [`Entity`]s of the model.
    pub entities: Vec<Entity>,
    /// The [`Record`]s of the model.
    pub records: Vec<Record>,
    /// Convention-specific metadata.
    pub conventions: Vec<Convention>,
}
