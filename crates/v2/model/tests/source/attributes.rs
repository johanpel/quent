// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(unused)]

use quent_v2_model::{
    Attributes,
    entity_ref::{AnyEntity, EntityRef},
};

#[derive(Attributes)]
pub struct Unit;

#[derive(Attributes)]
pub struct UnitBraces {}

#[derive(Attributes)]
pub struct OnePrim {
    pub a: u8,
}

#[derive(Attributes)]
pub struct OneNested {
    a: OnePrim,
}

#[derive(Attributes)]
pub struct OneList {
    a: Vec<u8>,
}

#[derive(Attributes)]
pub struct OneListNested {
    a: Vec<OnePrim>,
}

#[derive(Attributes)]
pub struct OneListListPrim {
    a: Vec<Vec<u8>>,
}

#[derive(Attributes)]
pub struct OneListListNested {
    a: Vec<Vec<OnePrim>>,
}

#[derive(Attributes)]
pub struct MultiPrim {
    pub a: u8,
    pub b: String,
}

#[derive(Attributes)]
pub struct MultiNested {
    a: u8,
    b: MultiPrim,
    c: Vec<u16>,
    d: String,
}

#[derive(Attributes)]
pub struct MultiOption {
    a: u8,
    b: Option<String>,
}

// TODO(johanpel): a usage in below
// #[derive(Resource)]
// struct UnitResource;

#[derive(Attributes)]
pub struct AllTypes {
    a_bool: bool,
    a_uuid: uuid::Uuid,
    a_string: String,
    a_u8: u8,
    a_u16: u16,
    a_u32: u32,
    a_u64: u64,
    a_i8: i8,
    a_i16: i16,
    a_i32: i32,
    a_i64: i64,
    a_f32: f32,
    a_f64: f64,
    a_option: Option<u64>,
    a_list: Vec<u64>,
    a_nested: MultiNested,
    a_custom: quent_attributes::CustomAttributes,
    a_entity_ref: EntityRef<AnyEntity>,
    // pub a_usage: Usage<UnitResource>,
}
