// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Parses the YAML model and generates the instrumentation library into `OUT_DIR`.

use std::path::Path;

use quent_instrumentation_build::{GenerateInfo, Options, generate};
use quent_schema::builder::SchemaBuilder;
use quent_schema::test_utils::{entity, event};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = Path::new(env!("CARGO_MANIFEST_DIR")).join("model.yaml");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", model.display());

    // YAML source -> quent_schema::Schema. Errors carry file:line:column.
    let parsed = quent_yaml::parse_from_file(&model)?;
    // Constraints no validator handles (none in this model).
    for warning in &parsed.warnings {
        println!("cargo:warning={warning}");
    }

    // Schema -> generated Rust instrumentation source.
    let opts = Options {
        // To just print the events in this example, we'll be using the callback
        // exporter. This exporter takes a type-erased event, so in order to
        // simplify downcasting back to a statically-typed event, this features
        // enables the generation of the "AnyEvent" helper type (see main.rs).
        // This is typically left false when using "real" exporters.
        any_event: true,
        ..Default::default()
    };
    let GenerateInfo { path, warnings } = generate(&parsed.schema, &opts)?;

    if !warnings.is_empty() {
        println!("cargo:warning= {}", warnings.join("\n"));
    }
    println!(
        "cargo:warning=instrumentation library written to {}",
        path.display()
    );

    // Compile a nested event-only model so rustc checks umbrella conversions.
    let fixture = SchemaBuilder::try_new("UmbrellaFixture")?
        .with_entity(entity("Foo::Query", [event("created", [])]))
        .with_entity(entity("Foo::Nested::Task", [event("created", [])]))
        .build()?;
    generate(
        &fixture,
        &Options {
            instrumentation: false,
            umbrella_event: true,
            file_name: Some("umbrella_fixture.rs".to_owned()),
            ..Options::default()
        },
    )?;

    Ok(())
}
