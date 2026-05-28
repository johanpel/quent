// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(unused)]

//! Types used in tests
use quent_v2_model::{
    Entity, EntityRef,
    entity_ref::{Plain, Scope},
};
use uuid::Uuid;

use crate::source::records;

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
    pub b: records::OnePrim,
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
    A(records::OnePrim),
}

#[derive(Entity)]
pub enum EnumMultiAttribs {
    A(records::OnePrim),
    B(records::MultiPrim),
}

#[derive(Entity)]
pub enum EnumInlineAttribs {
    A { x: u8, y: String },
    B,
}

/// A documented entity.
/// Line two of the docstring.
#[derive(Entity)]
pub struct DocumentedEntity {
    /// Doc on a field.
    pub a: u8,
}

#[derive(Entity)]
pub enum DocumentedVariantEnum {
    /// Doc on a variant.
    Alpha,
    Beta {
        /// Doc on an inline event field.
        x: u8,
    },
}

#[derive(Entity)]
pub enum EnumBuiltinAttribs {
    A {
        x: u8,
        y: String,
        z: EntityRef<Plain, EnumInlineAttribs>,
        d: EntityRef<Scope, Unit>,
    },
    B {
        k: Uuid,
    },
}
