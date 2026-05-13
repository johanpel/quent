use std::collections::HashMap;

pub mod attributes;
pub mod entity;
pub mod event;
pub mod qualifications;

pub struct ModelDef {
    pub name: String,
    pub entities: HashMap<String, entity::EntityDef>,
    pub attributes: HashMap<String, attributes::AttributesDef>,
}
