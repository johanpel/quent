// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model::EntityHandle;
use quent_v2_model::entity_ref::IntoErased;
use source::attributes;
use source::resource_groups::*;

use crate::utils::ROOT;

mod source;
mod utils;

#[test]
fn root_unit_structs() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let unit_obs = RootUnitObserver::try_new(ROOT, None)?;
    let unit_handle: RootUnitHandle = unit_obs.handle();
    unit_handle.root_unit()?;

    let unit_braces_obs: RootUnitBracesObserver = RootUnitBracesObserver::try_new(ROOT, None)?;
    let unit_braces_handle: RootUnitBracesHandle = unit_braces_obs.handle();
    unit_braces_handle.root_unit_braces()?;

    Ok(())
}

#[test]
fn root_structs() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let struct_prim_obs = RootStructPrimObserver::try_new(ROOT, None)?;
    let struct_prim_handle: RootStructPrimHandle = struct_prim_obs.handle();
    struct_prim_handle.root_struct_prim(RootStructPrim { a: 0 })?;

    let struct_multi_attrib_obs = RootStructMultiAttribObserver::try_new(ROOT, None)?;
    let struct_multi_attrib_handle: RootStructMultiAttribHandle = struct_multi_attrib_obs.handle();
    struct_multi_attrib_handle.root_struct_multi_attrib(RootStructMultiAttrib {
        a: 0,
        b: String::new(),
    })?;

    Ok(())
}

#[test]
fn root_enums() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let enum_one_unit_obs = RootEnumOneUnitObserver::try_new(ROOT, None)?;
    let enum_one_unit_handle: RootEnumOneUnitHandle = enum_one_unit_obs.handle();
    enum_one_unit_handle.a()?;

    let enum_multi_unit_obs = RootEnumMultiUnitObserver::try_new(ROOT, None)?;
    let enum_multi_unit_handle: RootEnumMultiUnitHandle = enum_multi_unit_obs.handle();
    enum_multi_unit_handle.a()?;
    enum_multi_unit_handle.b()?;

    let enum_single_attribs_obs = RootEnumSingleAttribsObserver::try_new(ROOT, None)?;
    let enum_single_attribs_handle: RootEnumSingleAttribsHandle = enum_single_attribs_obs.handle();
    enum_single_attribs_handle.a(attributes::OnePrim { a: 0 })?;

    let enum_multi_attribs_obs = RootEnumMultiAttribsObserver::try_new(ROOT, None)?;
    let enum_multi_attribs_handle: RootEnumMultiAttribsHandle = enum_multi_attribs_obs.handle();
    enum_multi_attribs_handle.a(attributes::OnePrim { a: 0 })?;
    enum_multi_attribs_handle.b(attributes::MultiPrim {
        a: 0,
        b: String::new(),
    })?;

    Ok(())
}

#[test]
fn non_root_enums() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Stand up a root RG to act as the parent.
    let root_obs = RootEnumOneUnitObserver::try_new(ROOT, None)?;
    let root_handle: RootEnumOneUnitHandle = root_obs.handle();
    let parent = root_handle.entity_ref().into_erased();

    let single_obs = EnumSingleAttribsObserver::try_new(ROOT, None)?;
    let single_handle: EnumSingleAttribsHandle = single_obs.handle();
    single_handle.a(attributes::OnePrim { a: 0 }, parent)?;

    let multi_obs = EnumMultiAttribsObserver::try_new(ROOT, None)?;
    let multi_handle: EnumMultiAttribsHandle = multi_obs.handle();
    multi_handle.a(attributes::OnePrim { a: 0 }, parent)?;
    multi_handle.b(attributes::MultiPrim {
        a: 0,
        b: String::new(),
    })?;

    Ok(())
}
