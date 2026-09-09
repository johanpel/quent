// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/finite_state_machine.rs"));
}

use instrumentation::{Context, FiniteStateMachine, Job, Noop};

#[rustfmt::skip]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<FiniteStateMachine>::try_new(Noop)?;
    let _job = context
        .observer::<Job>()
        .handle()         // FsmHandle<Job>
        .queued()         // FsmHandle<Job, job_state::Queued>
        .loading_input()  // FsmHandle<Job, job_state::LoadingInput>
        .running()        // FsmHandle<Job, job_state::Running>
        .completed();     // FsmHandle<Job, job_state::Completed>

    Ok(())
}
