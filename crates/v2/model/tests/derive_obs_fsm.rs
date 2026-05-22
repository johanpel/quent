// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

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
