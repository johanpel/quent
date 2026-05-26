// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model::{
    EntityHandle, EntityRef,
    entity_ref::{Plain, Scope},
};
use source::entities::*;
use source::records;
use uuid::Uuid;

use crate::utils::ROOT;
mod source;

mod utils;

#[test]
fn unit() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let unit_obs = UnitObserver::try_new(ROOT, None)?;
    let unit_handle: UnitHandle = unit_obs.handle();
    unit_handle.unit()?;

    Ok(())
}

#[test]
fn unit_braces() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let unit_braces_obs: UnitBracesObserver = UnitBracesObserver::try_new(ROOT, None)?;
    let unit_braces_handle: UnitBracesHandle = unit_braces_obs.handle();
    unit_braces_handle.unit_braces()?;

    Ok(())
}

#[test]
fn struct_prim() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let struct_prim_obs = StructPrimObserver::try_new(ROOT, None)?;
    let struct_prim_handle: StructPrimHandle = struct_prim_obs.handle();
    struct_prim_handle.struct_prim(StructPrim { a: 0 })?;

    Ok(())
}

#[test]
fn struct_multi_attrib() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let struct_multi_attrib_obs = StructMultiAttribObserver::try_new(ROOT, None)?;
    let struct_multi_attrib_handle: StructMultiAttribHandle = struct_multi_attrib_obs.handle();
    struct_multi_attrib_handle.struct_multi_attrib(StructMultiAttrib {
        a: 0,
        b: records::OnePrim { a: 0 },
    })?;

    Ok(())
}

#[test]
fn enum_one_unit() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let enum_one_unit_obs = EnumOneUnitObserver::try_new(ROOT, None)?;
    let enum_one_unit_handle: EnumOneUnitHandle = enum_one_unit_obs.handle();
    enum_one_unit_handle.a()?;

    Ok(())
}

#[test]
fn enum_multi_unit() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let enum_multi_unit_obs = EnumMultiUnitObserver::try_new(ROOT, None)?;
    let enum_multi_unit_handle: EnumMultiUnitHandle = enum_multi_unit_obs.handle();
    enum_multi_unit_handle.a()?;
    enum_multi_unit_handle.b()?;

    Ok(())
}

#[test]
fn enum_single_attribs() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let enum_single_attribs_obs = EnumSingleAttribsObserver::try_new(ROOT, None)?;
    let enum_single_attribs_handle: EnumSingleAttribsHandle = enum_single_attribs_obs.handle();
    enum_single_attribs_handle.a(records::OnePrim { a: 0 })?;

    Ok(())
}

#[test]
fn enum_multi_attribs() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let enum_multi_attribs_obs = EnumMultiAttribsObserver::try_new(ROOT, None)?;
    let enum_multi_attribs_handle: EnumMultiAttribsHandle = enum_multi_attribs_obs.handle();
    enum_multi_attribs_handle.a(records::OnePrim { a: 0 })?;
    enum_multi_attribs_handle.b(records::MultiPrim {
        a: 0,
        b: String::new(),
    })?;

    Ok(())
}

#[test]
fn enum_inline_attribs() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let obs = EnumInlineAttribsObserver::try_new(ROOT, None)?;
    let handle: EnumInlineAttribsHandle = obs.handle();
    handle.a(0, String::from("hello"))?;
    handle.b()?;
    Ok(())
}

#[test]
fn enum_builtin_attribs() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let inline_obs = EnumInlineAttribsObserver::try_new(ROOT, None)?;
    let inline_handle: EnumInlineAttribsHandle = inline_obs.handle();
    let z_ref: EntityRef<Plain, EnumInlineAttribs> = EntityRef::new(inline_handle.id(), Plain);

    let unit_obs = UnitObserver::try_new(ROOT, None)?;
    let unit_handle: UnitHandle = unit_obs.handle();
    let d_ref: EntityRef<Scope, Unit> = EntityRef::new(unit_handle.id(), Scope);

    let obs = EnumBuiltinAttribsObserver::try_new(ROOT, None)?;
    let handle: EnumBuiltinAttribsHandle = obs.handle();
    handle.a(0, String::from("hello"), z_ref, d_ref)?;
    handle.b(Uuid::nil())?;
    Ok(())
}
