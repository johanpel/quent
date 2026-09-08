// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/bounded_resource.rs"));
}

use instrumentation::{BoundedResource, Context, Memory, MemoryBounds, MemoryUsage, Noop, Task};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<BoundedResource>::try_new(Noop)?;
    let memory = context.observer::<Memory>().handle();

    memory.resized(MemoryBounds {
        bytes: 8_000_000_000,
    })?;

    let mut task = context.observer::<Task>().handle();
    task.running(
        0,
        memory.as_entity_ref_with(MemoryUsage { bytes: 512_000_000 }),
    )?;
    task.completed(1)?;

    Ok(())
}
