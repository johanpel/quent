use crate::ir::{attributes::Field, value_type::ValueType};

/// IR of the cardinality of an event.
#[derive(Debug, PartialEq, Eq)]
pub enum Cardinality {
    /// The event can only be emitted once.
    Once,
    /// The event can be emitted multiple times.
    Multi,
}

/// IR of the type of payload of an event.
#[derive(Debug, PartialEq)]
pub enum Payload {
    /// The event only has one value as payload.
    Value(ValueType),
    /// The event has a set of named fields as a payload.
    ///
    /// [`crate::ir::qualifications::Qualification`]s can require certain named
    /// fields to exist in the payload.
    Named(Vec<Field>),
}

/// IR of an event.
#[derive(Debug, PartialEq)]
pub struct Event {
    /// The name of the event.
    pub name: String,
    /// The [`Cardinality`] of the event.
    pub cardinality: Cardinality,
    /// The type of [`Payload`] of the event.
    pub payload: Payload,
}
