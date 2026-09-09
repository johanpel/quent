// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/unit_resource.rs"));
}

use instrumentation::{Context, Noop, Task, Thread, ThreadPool, ThreadUsage, UnitResource};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<UnitResource>::try_new(Noop)?;

    let mut pool = context.observer::<ThreadPool>().handle();
    pool.created()?;

    let mut thread = context.observer::<Thread>().handle();
    thread.registered(pool.as_entity_ref())?;

    let _task = context
        .observer::<Task>()
        .handle()
        .running(thread.as_entity_ref_with(ThreadUsage))
        .completed();

    Ok(())
}
