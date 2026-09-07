// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/event_data.rs"));
}

use instrumentation::{Context, EventData, Noop, Task};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<EventData>::try_new(Noop)?;
    let mut task = context.observer::<Task>().handle();

    task.started("compile".to_owned(), 1)?;
    task.ended(true, 128)?;

    Ok(())
}
