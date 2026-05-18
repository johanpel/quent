use crate::{
    Span,
    validator::qualifications::QualificationCheck,
    {event::Event, qualifications::Qualification},
};

/// Trait to obtain the IR of a type representing an entity.
pub trait ModelEntity {
    fn model_entity() -> Entity;
}

/// IR of an Entity
#[derive(Debug, PartialEq)]
pub struct Entity {
    /// The name of the entity.
    pub name: String,
    /// The [`Event`]s types that this entity can emit.
    pub events: Vec<Event>,
    /// The [`Qualification`]s of the entity.
    pub qualifications: Vec<Qualification>,

    /// The Rust path of the entity.
    pub rust_path: String,

    /// Optional span for use within proc macros
    pub span: Span,
}

impl Entity {
    pub fn new(
        name: impl Into<String>,
        events: Vec<Event>,
        qualifications: Vec<Qualification>,
        rust_path: impl Into<String>,
    ) -> Self {
        Self::with_span(name, events, qualifications, rust_path, Span::default())
    }

    pub fn with_span(
        name: impl Into<String>,
        events: Vec<Event>,
        qualifications: Vec<Qualification>,
        rust_path: impl Into<String>,
        span: Span,
    ) -> Self {
        Self {
            name: name.into(),
            events,
            qualifications,
            rust_path: rust_path.into(),
            span,
        }
    }

    pub fn qualification<T>(&self) -> Option<&T>
    where
        T: QualificationCheck,
        for<'a> &'a T: TryFrom<&'a Qualification>,
    {
        self.qualifications
            .iter()
            .find_map(|q| <&T>::try_from(q).ok())
    }
}
