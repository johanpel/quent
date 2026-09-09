// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused, clippy::too_many_arguments)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/event_data.rs"));
}

use instrumentation::{Context, DynamicAttributes, EventData, Noop, Task, Uuid};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<EventData>::try_new(Noop)?;
    let mut task = context.observer::<Task>().handle();

    task.started(true, 1, 2, 3, 4, -1, -2, -3, -4)?;

    let mut extra = DynamicAttributes::new();
    extra.add_string("worker", "alpha");
    extra.add_u64("queue_depth", 3);

    task.ended(
        0.5,
        0.95,
        "complete".to_owned(),
        Uuid::now_v7(),
        None,
        vec!["batch".to_owned(), "priority".to_owned()],
        extra,
    )?;

    Ok(())
}
