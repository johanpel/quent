// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/finite_state_machine.rs"));
}

use instrumentation::{Context, FiniteStateMachine, Job, Noop};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<FiniteStateMachine>::try_new(Noop)?;
    let mut job = context.observer::<Job>().handle();

    job.queued()?;
    job.loading_input()?;
    job.running()?;
    job.completed()?;

    Ok(())
}
