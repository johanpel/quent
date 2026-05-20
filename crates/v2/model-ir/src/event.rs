// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{IrError, identifier::Identifier, value_type::ValueType};

/// IR of the cardinality of an event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cardinality {
    /// The event can only be emitted once.
    Once,
    /// The event can be emitted multiple times.
    Multi,
}

/// The role of an event field
#[derive(Debug, PartialEq)]
pub enum FieldRole {
    /// The field carries the user payload.
    // TODO(johanpel): bad name, since a parent field will also be part of the event payload, so change later
    Payload,
    /// The field carries a parent relation, e.g. for resource or resource group.
    Parent,
}

impl FieldRole {
    /// Required field name for the user payload field of an event.
    pub const PAYLOAD: &'static str = "payload";
    /// Required field name for events that carry a parent-child relation, e.g. for resource group trees.
    pub const PARENT: &'static str = "parent";
}

impl From<FieldRole> for Identifier {
    fn from(value: FieldRole) -> Self {
        match value {
            FieldRole::Payload => Identifier::new_unchecked(FieldRole::PAYLOAD),
            FieldRole::Parent => Identifier::new_unchecked(FieldRole::PARENT),
        }
    }
}

impl TryFrom<&str> for FieldRole {
    type Error = IrError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            FieldRole::PAYLOAD => Ok(FieldRole::Payload),
            FieldRole::PARENT => Ok(FieldRole::Parent),
            other => Err(IrError::UnknownFieldRole(other.to_string())),
        }
    }
}

/// IR of a type of event payload field
///
/// Not to be confused with fields of attribute sets, which are always
/// user-defined and have no special meaning as far as the IR is concerned.
// TODO(johanpel): we could make this more strict and modular by turning it into
// an enum with variants like User(String, ValueType),
// Qualification(<qualification-related payload enum>), but this requires moving
// more logic into the derive macro.
#[derive(Debug, PartialEq)]
pub struct Field {
    /// The role of the field.
    pub role: FieldRole,
    /// The type of the field.
    pub ty: ValueType,
}

impl Field {
    pub fn new(role: FieldRole, ty: ValueType) -> Self {
        Self { role, ty }
    }
}

/// IR of an event.
#[derive(Debug, PartialEq)]
pub struct Event {
    /// The name of the event.
    pub name: Identifier,
    /// The [`Cardinality`] of the event.
    pub cardinality: Cardinality,
    /// The fields of the [`Payload`] of the event.
    pub payload: Vec<Field>,
}

impl Event {
    pub fn new(name: Identifier, cardinality: Cardinality, payload: Vec<Field>) -> Self {
        Self {
            name,
            cardinality,
            payload,
        }
    }
}
