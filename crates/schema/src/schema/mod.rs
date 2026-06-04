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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Schema {
    /// The name of the model.
    pub name: Identifier,
    /// The [`Entity`]s of the model.
    pub entities: Vec<Entity>,
    /// The [`Record`]s of the model.
    pub records: Vec<Record>,
    /// Annotations of this schema.
    pub annotations: Annotations,
}
