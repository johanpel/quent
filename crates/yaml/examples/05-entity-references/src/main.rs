// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/references.rs"));
}

use instrumentation::{Context, Noop, References, Task, Worker};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<References>::try_new(Noop)?;

    let mut worker = context.observer::<Worker>().handle();
    worker.registered()?;

    let mut task = context.observer::<Task>().handle();
    task.started(worker.as_entity_ref())?;
    task.ended()?;

    Ok(())
}
