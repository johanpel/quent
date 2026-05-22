// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    identifier::Identifier,
    value_type::{ModelValueType, ValueType},
};

/// Trait to obtain the IR of types that can be used as event fields.
pub trait ModelEventFieldType {
    fn model_event_field_type() -> EventFieldType;
}

/// Trait to obtain the IR of an [`crate::entity::EntityRef`] target.
pub trait ModelEntityRefTarget {
    fn model_entity_ref_target() -> EntityRefTarget;
}

/// Trait to obtain the IR of an [`quent_v2_model::EntityRef`] role.
pub trait ModelEntityRefRole {
    fn model_entity_ref_role() -> EntityRefRole;
}

/// IR of the cardinality of an event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cardinality {
    /// The event can only be emitted once.
    Once,
    /// The event can be emitted multiple times.
    Multi,
}

/// IR of the types of entities targeted by an entity reference.
#[derive(Clone, Debug, PartialEq)]
pub enum EntityRefTarget {
    /// The entity reference targets any entity.
    Any,
    /// The entity reference targets one specific entity type.
    Specific(Identifier),
}

/// IR of the role of an entity reference.
#[derive(Clone, Debug, PartialEq)]
pub enum EntityRefRole {
    /// No specific role.
    Plain,
    /// A structural role.
    Scope,
    /// A role defined by an arbitrary type that is an attribute set.
    User(Identifier),
}

/// The type of an event field.
#[derive(Debug, PartialEq)]
pub enum EventFieldType {
    /// An arbitrary payload event field, application-specific.
    Payload(ValueType),
    /// A reference to another entity.
    EntityRef {
        /// The type of the data associated with the role of this reference
        role_type: EntityRefRole,
        /// The entity type this reference can target.
        entity_type: EntityRefTarget,
    },
    /// A usage of a resource
    ResourceUsage {
        /// The resource
        resource: Identifier,
    },
    /// A bound of a resource
    ResourceBound {
        /// The resource
        resource: Identifier,
        /// The field of the resource
        field: Identifier,
    },
}

impl EventFieldType {
    /// Returns the default name for a field with this type.
    pub fn default_name(&self) -> &'static str {
        match self {
            EventFieldType::Payload(_) => "payload",
            EventFieldType::EntityRef { .. } => "entity",
            EventFieldType::ResourceUsage { .. } => "usage",
            EventFieldType::ResourceBound { .. } => "bound",
        }
    }
}

/// Anything that's a [`ModelValueType`] is wrapped into
/// [`EventFieldType::Payload`] if used as an event field type, as it is an
/// application-specific type.
impl<T: ModelValueType> ModelEventFieldType for T {
    fn model_event_field_type() -> EventFieldType {
        EventFieldType::Payload(T::model_value_type())
    }
}

/// IR of a type of event payload field
///
/// Not to be confused with fields of attribute sets, which are always
/// user-defined and have no special meaning as far as the IR is concerned.
#[derive(Debug, PartialEq)]
pub struct EventField {
    /// The name of the event field.
    pub name: Identifier,
    /// The type of the event field.
    pub ty: EventFieldType,
}

impl EventField {
    pub fn new(name: Identifier, ty: EventFieldType) -> Self {
        Self { name, ty }
    }

    /// Construct an EventField using the [`EventFieldType::default_name`] of
    /// the type `ty`.
    pub fn from_type(ty: EventFieldType) -> Self {
        Self {
            name: Identifier::new_unchecked(ty.default_name()),
            ty,
        }
    }
}

/// IR of an event.
#[derive(Debug, PartialEq)]
pub struct Event {
    /// The name of the event.
    pub name: Identifier,
    /// The [`Cardinality`] of the event.
    pub cardinality: Cardinality,
    /// The fields of the event.
    pub payload: Vec<EventField>,
}

impl Event {
    pub fn new(name: Identifier, cardinality: Cardinality, payload: Vec<EventField>) -> Self {
        Self {
            name,
            cardinality,
            payload,
        }
    }
}
