// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/resource_capacity.rs"));
}

use instrumentation::{Context, Memory, MemoryUsage, Noop, ResourceCapacity, Task};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<ResourceCapacity>::try_new(Noop)?;

    let mut memory = context.observer::<Memory>().handle();
    memory.created()?;

    let _task = context
        .observer::<Task>()
        .handle()
        .running(memory.as_entity_ref_with(MemoryUsage { bytes: 512_000_000 }))
        .completed();

    Ok(())
}
