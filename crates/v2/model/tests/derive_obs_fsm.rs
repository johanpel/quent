// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use source::attributes::{MultiNested, MultiPrim, OnePrim};
use source::fsms::*;
use uuid::Uuid;

mod source;

const ROOT: Uuid = Uuid::nil();

#[test]
fn one_unit() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let obs = OneUnitObserver::try_new(ROOT, None)?;
    let handle: OneUnitHandle<one_unit_state::A> = obs.a()?;
    let id = handle.exit()?;
    assert!(!id.is_nil());
    Ok(())
}

#[test]
fn multi_unit() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let obs = MultiUnitObserver::try_new(ROOT, None)?;
    let handle: MultiUnitHandle<multi_unit_state::A> = obs.a()?;
    let id = handle.b()?.c()?.exit()?;
    assert!(!id.is_nil());
    Ok(())
}

#[test]
fn one_attribs() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let obs = OneAttribsObserver::try_new(ROOT, None)?;
    let handle: OneAttribsHandle<one_attribs_state::A> = obs.a(OnePrim { a: 0 })?;
    let id = handle.exit()?;
    assert!(!id.is_nil());
    Ok(())
}

#[test]
fn multi_attribs() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let obs = MultiAttribsObserver::try_new(ROOT, None)?;
    let handle: MultiAttribsHandle<multi_attribs_state::A> = obs.a(OnePrim { a: 0 })?;
    let id = handle
        .b(MultiNested {
            a: 0,
            b: MultiPrim {
                a: 0,
                b: String::new(),
            },
            c: vec![],
            d: String::new(),
        })?
        .exit()?;
    assert!(!id.is_nil());
    Ok(())
}

#[test]
fn self_loop() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let obs = SelfLoopObserver::try_new(ROOT, None)?;
    let handle: SelfLoopHandle<self_loop_state::A> = obs.a()?;
    let id = handle.a()?.a()?.exit()?;
    assert!(!id.is_nil());
    Ok(())
}

#[test]
fn loop_() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let obs = LoopObserver::try_new(ROOT, None)?;
    let handle: LoopHandle<loop_state::A> = obs.a()?;
    let id = handle.b()?.c()?.a()?.exit()?;
    assert!(!id.is_nil());
    Ok(())
}
