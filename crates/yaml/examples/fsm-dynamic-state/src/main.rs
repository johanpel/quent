// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/fsm_dynamic_state.rs"));
}

use instrumentation::{
    Context, DynamicFsmHandle, FsmDynamicState, FsmHandle, Job, Noop, job_state,
};

fn prepare_job(
    job: FsmHandle<Job, job_state::Queued>,
    restore_from_checkpoint: bool,
) -> DynamicFsmHandle<Job> {
    if restore_from_checkpoint {
        job.restoring_checkpoint().into_dynamic()
    } else {
        job.loading_input().into_dynamic()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let restore_from_checkpoint = std::env::args().any(|arg| arg == "--restore");
    let context = Context::<FsmDynamicState>::try_new(Noop)?;
    let queued = context.observer::<Job>().handle().queued();
    let mut job = prepare_job(queued, restore_from_checkpoint);
    job.running()?;
    let job = job.try_into::<job_state::Running>()?;
    job.completed();
    Ok(())
}
