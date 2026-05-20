// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use source::fsms::*;
use uuid::Uuid;

mod source;

const ROOT: Uuid = Uuid::nil();

#[test]
fn unit_states() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let one_unit_obs = OneUnitObserver::try_new(ROOT, None)?;
    let one_unit_handle: OneUnitHandle<one_unit_state::A> = one_unit_obs.a()?;
    one_unit_handle.exit()?;

    let multi_unit_obs = MultiUnitObserver::try_new(ROOT, None)?;
    let multi_unit_handle: MultiUnitHandle<multi_unit_state::A> = multi_unit_obs.a()?;
    let multi_unit_id = multi_unit_handle.b()?.c()?.exit()?;
    assert!(!multi_unit_id.is_nil());

    Ok(())
}
