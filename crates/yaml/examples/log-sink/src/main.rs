// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused, clippy::too_many_arguments)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/logging_sink.rs"));
}

use instrumentation::{AppLog, Context, LoggingSink, Noop};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<LoggingSink>::try_new(Noop)?;
    let log = context.observer::<AppLog>().handle();

    // A logging API would typically use a macro to capture `file!()`, `line!()`,
    // and any other call-site context required by the schema.
    log.info(
        "application started".to_owned(),
        "startup".to_owned(),
        file!().to_owned(),
        line!(),
        module_path!().to_owned(),
        "main".to_owned(),
    )?;
    log.warning(
        "retrying request".to_owned(),
        "network".to_owned(),
        file!().to_owned(),
        line!(),
        module_path!().to_owned(),
        "main".to_owned(),
        "transient".to_owned(),
    )?;

    Ok(())
}
