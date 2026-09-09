// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/scoped_references.rs"));
}

use instrumentation::{Context, Noop, Pipeline, ScopedReferences, Task};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<ScopedReferences>::try_new(Noop)?;

    let mut pipeline = context.observer::<Pipeline>().handle();
    pipeline.created()?;

    let mut task = context.observer::<Task>().handle();
    task.started(pipeline.as_entity_ref())?;
    task.ended()?;

    Ok(())
}
