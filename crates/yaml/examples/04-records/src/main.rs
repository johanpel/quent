// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/records.rs"));
}

use instrumentation::{Context, Noop, Records, Task, TaskResult};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<Records>::try_new(Noop)?;
    let mut task = context.observer::<Task>().handle();

    task.started()?;
    task.ended(TaskResult {
        success: true,
        items_processed: 128,
    })?;

    Ok(())
}
