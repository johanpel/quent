// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Intermediate Representation of an application event model.
//!
//! This module holds definitions of an Intermediate Representation (IR) of an
//! application event model using Quent's core concepts: records, events,
//! entities, FSMs, and entity references.
//!
//! The IR is leveraged for code generation, model validation, and
//! serialization.
//!
//! Code generation involves generating cross-language compatible bridge code,
//! e.g. for C++ through CXX, or for Python through PyO3.
//!
//! Model validation involves checking certain constraints, e.g. that
//! [`Identifier`]s are accepted by the prescribed grammar or that all
//! [`fsm::Fsm`] states are reachable from the entry transition and an exit
//! transition is reachable from all states.
//!
//! Serialization involves the ability to store a model, which can be leveraged
//! for model re-use, sharing, and archival purposes. As such, this IR could be
//! considered a "schema" for the telemetry that applications can emit.
//!
//! The IR is kept as minimialistic as possible in order to not pollute itself
//! with any application-specific semantics, conventions or constraints, yet it
//! does provide the means to carry metadata for such conventions with the IR at
//! various levels.
//!
//! A lightweight canonical mechanism exists for validating conventions in the
//! `quent-ir-validation` crate. It is strongly recommended to perform this
//! validation after constructing the IR from any source that isn't inherently
//! guaranteed to validate.

use crate::{convention::Convention, entity::Entity, identifier::Identifier, record::Record};

pub mod convention;
pub mod data_type;
pub mod entity;
pub mod event;
pub mod fsm;
pub mod identifier;
pub mod record;

/// IR of an application model.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Model {
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
