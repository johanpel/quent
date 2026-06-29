// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Loads the QDL model and generates the instrumentation library into `OUT_DIR`.

use std::path::Path;

use quent_instrumentation_build::{GenerateInfo, Options, generate};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = Path::new(env!("CARGO_MANIFEST_DIR")).join("model.qdl");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", model.display());

    // QDL source -> quent_schema::Schema.
    let schema = quent_qdl::load(&model)?;

    // Schema -> generated Rust instrumentation source.
    let opts = Options {
        event_derives: &["Debug"],
        record_derives: &["Debug"],
        ..Default::default()
    };
    let GenerateInfo { path, warnings } = generate(&schema, &opts)?;

    if !warnings.is_empty() {
        println!("cargo:warning={}", warnings.join("\n"));
    }
    println!(
        "cargo:warning=instrumentation library written to {}",
        path.display()
    );

    Ok(())
}
