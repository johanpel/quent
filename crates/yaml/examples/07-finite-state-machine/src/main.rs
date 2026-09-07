// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/finite_state_machine.rs"));
}

use instrumentation::{Context, FiniteStateMachine, Noop, Task};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<FiniteStateMachine>::try_new(Noop)?;
    let mut task = context.observer::<Task>().handle();

    task.running()?;
    task.completed()?;

    Ok(())
}
