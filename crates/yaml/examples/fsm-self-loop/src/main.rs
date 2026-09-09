// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/fsm_self_loop.rs"));
}

use instrumentation::{Context, FsmSelfLoop, Noop, Task};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<FsmSelfLoop>::try_new(Noop)?;
    let task = context.observer::<Task>().handle().running(0);

    // Direct self-loop: running -> running.
    let task = task.running(64);
    // Indirect cycle: running -> paused -> running.
    let task = task.paused().running(128);
    let _task = task.completed();

    Ok(())
}
