// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/untyped_references.rs"));
}

use instrumentation::{Context, Noop, Task, UntypedReferences, Worker};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<UntypedReferences>::try_new(Noop)?;

    let mut worker = context.observer::<Worker>().handle();
    worker.started()?;

    let mut task = context.observer::<Task>().handle();
    task.started(worker.as_any_entity_ref())?;
    task.ended()?;

    worker.ended()?;

    Ok(())
}
