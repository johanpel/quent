// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/dynamic_data.rs"));
}

use instrumentation::{Context, DynamicAttributes, DynamicData, Noop, Task};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<DynamicData>::try_new(Noop)?;
    let mut task = context.observer::<Task>().handle();

    let mut started_details = DynamicAttributes::new();
    started_details.add("queue", "priority");
    started_details.add("attempt", 2_u64);
    task.started(started_details)?;

    let mut ended_details = DynamicAttributes::new();
    ended_details.add("cached", false);
    ended_details.add("items_processed", 128_u64);
    task.ended(ended_details)?;

    Ok(())
}
