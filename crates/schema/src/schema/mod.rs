// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Plain schema data types

use crate::schema::{
    annotations::Annotations, entity::Entity, identifier::Identifier, record::Record,
};

pub mod annotations;
pub mod constraint;
pub mod data_type;
pub mod entity;
pub mod event;
pub mod field;
pub mod identifier;
pub mod metadata;
pub mod record;

/// Container type for named elements.
pub type Map<K, V> = indexmap::IndexMap<K, V, rustc_hash::FxBuildHasher>;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Schema {
    /// The name of the model.
    pub name: Identifier,
    /// The [`Entity`]s of the model.
    pub entities: Map<Identifier, Entity>,
    /// The [`Record`]s of the model.
    pub records: Map<Identifier, Record>,
    /// Annotations of this schema.
    pub annotations: Annotations,
}
