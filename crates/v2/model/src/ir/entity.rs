use crate::{
    ir::{event::Event, qualifications::Qualification},
    validator::qualifications::QualificationCheck,
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
}

impl Entity {
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
