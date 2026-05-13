use std::collections::HashSet;

use crate::{
    ir::{event::EventDef, qualifications::QualificationProps},
    validator::qualifications::Qualification,
};

pub struct EntityDef {
    pub name: String,
    pub rust_path: String,
    pub events: Vec<EventDef>,
    pub qualifications: HashSet<QualificationProps>,
}

impl EntityDef {
    pub fn qualification<T>(&self) -> Option<&T>
    where
        T: Qualification,
        for<'a> &'a T: TryFrom<&'a QualificationProps>,
    {
        self.qualifications
            .iter()
            .find_map(|q| <&T>::try_from(q).ok())
    }
}
