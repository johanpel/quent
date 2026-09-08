// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/repeated_events.rs"));
}

use instrumentation::{Context, Noop, RepeatedEvents, Task};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<RepeatedEvents>::try_new(Noop)?;
    let mut task = context.observer::<Task>().handle();

    task.started("compile".to_owned())?;
    // `started` is a once event, so a second call would return an error.
    // task.started("compile".to_owned())?;
    task.progress(64)?;
    task.progress(128)?;
    task.ended(true)?;

    Ok(())
}
