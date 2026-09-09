// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/job_workload.rs"));
}

use instrumentation::{Context, Job, JobWorkload, Noop, Worker, WorkerBounds, WorkerUsage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<JobWorkload>::try_new(Noop)?;

    let mut worker = context.observer::<Worker>().handle();
    worker.ready("worker-1".to_owned(), WorkerBounds { threads: 16 })?;

    let _job = context
        .observer::<Job>()
        .handle()
        .queued("compile".to_owned(), 4)
        .running(worker.as_entity_ref_with(WorkerUsage { threads: 4 }))
        .completed();

    Ok(())
}
