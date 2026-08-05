// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::{fs::OpenOptions, io::Write, path::Path};

use quent_instrumentation_build::{Options, generate};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = Path::new(env!("CARGO_MANIFEST_DIR")).join("model.yaml");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", model.display());

    let parsed = quent_yaml::parse_from_file(&model)?;
    for warning in &parsed.warnings {
        println!("cargo:warning={warning}");
    }

    let generated = generate(
        &parsed.schema,
        &Options {
            serde: true,
            umbrella_event: true,
            analyzer_package: Some("quent-simulator-analyzer".to_owned()),
            collector_sink: true,
            ..Default::default()
        },
    )?;
    let store_impls = quent_store_build::generate_impls_str(&parsed.schema, true)?;
    OpenOptions::new()
        .append(true)
        .open(generated.path)?
        .write_all(store_impls.as_bytes())?;

    Ok(())
}
