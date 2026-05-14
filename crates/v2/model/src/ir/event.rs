use crate::ir::{attributes::Field, value_type::ValueType};

/// The cardinality of an event.
pub enum Cardinality {
    Once,
    Multi,
}

/// The type of payload of an event.
pub enum Payload {
    Unit,
    Value(ValueType),
    Named(Vec<Field>),
}

/// An event.
pub struct Event {
    /// The name of the event.
    pub name: String,
    /// The cardinality of the event.
    pub cardinality: Cardinality,
    /// The type of payload of the event.
    pub payload: Payload,
}
