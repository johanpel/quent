use std::collections::HashSet;

use crate::{
    ir::{event::Event, qualifications::Qualification},
    validator::qualifications::QualificationCheck,
};

pub struct Entity {
    pub name: String,
    pub rust_path: String,
    pub events: Vec<Event>,
    pub qualifications: HashSet<Qualification>,
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
