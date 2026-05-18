#![allow(unused)]

//! Types used in tests
use quent_v2_model::Entity;

use crate::source::attributes;

#[derive(Entity)]
pub struct Unit;

#[derive(Entity)]
pub struct UnitBraces {}

#[derive(Entity)]
pub struct StructPrim {
    pub a: u8,
}

#[derive(Entity)]
pub struct StructMultiAttrib {
    pub a: u8,
    pub b: attributes::OnePrim,
}

#[derive(Entity)]
pub enum EnumOneUnit {
    A,
}

#[derive(Entity)]
pub enum EnumMultiUnit {
    A,
    B,
}

#[derive(Entity)]
pub enum EnumSingleAttribs {
    A(attributes::OnePrim),
}

#[derive(Entity)]
pub enum EnumMultiAttribs {
    A(attributes::OnePrim),
    B(attributes::MultiPrim),
}
