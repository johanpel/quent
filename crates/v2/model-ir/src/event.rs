use crate::{identifier::Identifier, value_type::ValueType};

/// IR of the cardinality of an event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cardinality {
    /// The event can only be emitted once.
    Once,
    /// The event can be emitted multiple times.
    Multi,
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
    /// The name of the field.
    pub name: String,
    /// The type of the field.
    pub ty: ValueType,
}

impl Field {
    pub fn new(name: impl Into<String>, ty: ValueType) -> Self {
        Self {
            name: name.into(),
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
