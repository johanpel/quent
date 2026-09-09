// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/records.rs"));
}

use instrumentation::{Batch, Context, Noop, Records, Task, WorkResult};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<Records>::try_new(Noop)?;
    let mut task = context.observer::<Task>().handle();
    let mut batch = context.observer::<Batch>().handle();

    task.started()?;
    task.ended(WorkResult {
        success: true,
        items_processed: 128,
    })?;

    batch.started()?;
    batch.ended(WorkResult {
        success: true,
        items_processed: 512,
    })?;

    Ok(())
}
