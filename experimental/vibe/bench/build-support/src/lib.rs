// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Shared generation support for the instrumentation latency benchmarks.

use std::path::{Path, PathBuf};

use quent_schema_codegen_cpp::quent_schema::Schema;

/// Generate the Rust instrumentation model into the current build script's `OUT_DIR`.
pub fn generate_rust(manifest_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let schema = load_schema(manifest_dir.as_ref())?;
    generate_instrumentation(&schema)?;
    Ok(())
}

/// Generate and compile the CXX bridge for the benchmark model.
pub fn generate_cpp(manifest_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let schema = load_schema(manifest_dir.as_ref())?;
    generate_instrumentation(&schema)?;

    let package = std::env::var("CARGO_PKG_NAME")?;
    let options = quent_schema_codegen_cpp::Options {
        crate_name: package.clone(),
        instrumentation_path: "crate::instrumentation".to_owned(),
        exporters: quent_schema_codegen_cpp::Exporters {
            ndjson: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let files = quent_schema_codegen_cpp::emit(&schema, &options)?;
    let bridges = quent_schema_codegen_cpp::write_bridge_files(&files, &options)?;
    let mut build = cxx_build::bridges(bridges);
    let include_dir = quent_schema_codegen_cpp::stage_cxx_headers(&options)?;
    build
        .include(&include_dir)
        .std("c++20")
        .compile("quent_bench_cpp_bridge");
    println!("cargo:include={}", include_dir.display());
    Ok(())
}

/// Generate the PyO3 bridge and type stubs for the benchmark model.
pub fn generate_python(
    manifest_dir: impl AsRef<Path>,
    module_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let schema = load_schema(manifest_dir.as_ref())?;
    generate_instrumentation(&schema)?;

    let options = quent_schema_codegen_python::Options {
        module_name: module_name.to_owned(),
        instrumentation_path: "crate::instrumentation".to_owned(),
        exporters: quent_schema_codegen_python::Exporters {
            ndjson: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    quent_schema_codegen_python::write_generated_files(
        &quent_schema_codegen_python::emit(&schema, &options)?,
        &out_dir,
    )?;
    quent_schema_codegen_python::write_generated_files(
        &quent_schema_codegen_python::emit_stubs(&schema, &options)?,
        &out_dir,
    )?;
    Ok(())
}

fn load_schema(manifest_dir: &Path) -> Result<Schema, Box<dyn std::error::Error>> {
    let root = manifest_dir
        .ancestors()
        .find(|path| path.join("model.yaml").is_file())
        .ok_or("could not locate benchmark model.yaml")?;
    let model = root.join("model.yaml");
    println!("cargo:rerun-if-changed={}", model.display());
    let parsed = quent_yaml::parse_from_file(model)?;
    for warning in parsed.warnings {
        println!("cargo:warning={warning}");
    }
    Ok(parsed.schema)
}

fn generate_instrumentation(
    schema: &Schema,
) -> Result<(), quent_instrumentation_build::GenerateError> {
    quent_instrumentation_build::generate(
        schema,
        &quent_instrumentation_build::Options {
            file_name: Some("instrumentation.rs".to_owned()),
            serde: true,
            ..Default::default()
        },
    )?;
    Ok(())
}
