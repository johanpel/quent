// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use source::attributes;
use source::entities::*;

use crate::utils::ROOT;
mod source;

mod utils;

#[test]
fn unit_structs() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let unit_obs = UnitObserver::try_new(ROOT, None)?;
    let unit_handle: UnitHandle = unit_obs.handle();
    unit_handle.unit()?;

    let unit_braces_obs: UnitBracesObserver = UnitBracesObserver::try_new(ROOT, None)?;
    let unit_braces_handle: UnitBracesHandle = unit_braces_obs.handle();
    unit_braces_handle.unit_braces()?;

    Ok(())
}

#[test]
fn structs() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let struct_prim_obs = StructPrimObserver::try_new(ROOT, None)?;
    let struct_prim_handle: StructPrimHandle = struct_prim_obs.handle();
    struct_prim_handle.struct_prim(StructPrim { a: 0 })?;

    let struct_multi_attrib_obs = StructMultiAttribObserver::try_new(ROOT, None)?;
    let struct_multi_attrib_handle: StructMultiAttribHandle = struct_multi_attrib_obs.handle();
    struct_multi_attrib_handle.struct_multi_attrib(StructMultiAttrib {
        a: 0,
        b: attributes::OnePrim { a: 0 },
    })?;

    Ok(())
}

#[test]
fn enums() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let enum_one_unit_obs = EnumOneUnitObserver::try_new(ROOT, None)?;
    let enum_one_unit_handle: EnumOneUnitHandle = enum_one_unit_obs.handle();
    enum_one_unit_handle.a()?;

    let enum_multi_unit_obs = EnumMultiUnitObserver::try_new(ROOT, None)?;
    let enum_multi_unit_handle: EnumMultiUnitHandle = enum_multi_unit_obs.handle();
    enum_multi_unit_handle.a()?;
    enum_multi_unit_handle.b()?;

    let enum_single_attribs_obs = EnumSingleAttribsObserver::try_new(ROOT, None)?;
    let enum_single_attribs_handle: EnumSingleAttribsHandle = enum_single_attribs_obs.handle();
    enum_single_attribs_handle.a(attributes::OnePrim { a: 0 })?;

    let enum_multi_attribs_obs = EnumMultiAttribsObserver::try_new(ROOT, None)?;
    let enum_multi_attribs_handle: EnumMultiAttribsHandle = enum_multi_attribs_obs.handle();
    enum_multi_attribs_handle.a(attributes::OnePrim { a: 0 })?;
    enum_multi_attribs_handle.b(attributes::MultiPrim {
        a: 0,
        b: String::new(),
    })?;

    Ok(())
}
