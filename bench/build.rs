// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use quent_instrumentation_build::{Options, generate};
use quent_schema::builder::{EntityBuilder, EventBuilder, SchemaBuilder};

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
            event_derives: &["Debug", "::serde::Serialize", "::serde::Deserialize"],
            record_derives: &["Debug", "::serde::Serialize", "::serde::Deserialize"],
            file_name: Some("bench.rs".to_owned()),
            ..Default::default()
        },
    )?;
    for warning in generated.warnings {
        println!("cargo:warning={warning}");
    }

    // Native Bitcode derives cannot cross the benchmark's SmallVec/Cow-backed
    // dynamic payload. Generate an otherwise identical static model with the
    // unused dynamic event retained as a payload-free state so FSM annotations
    // and static event semantics stay unchanged.
    let mut native = SchemaBuilder::try_new("bench_native")?
        .with_annotations(parsed.schema.annotations().clone());
    for entity in parsed.schema.entities() {
        let mut native_entity = EntityBuilder::new(entity.name().clone())
            .with_annotations(entity.annotations().clone());
        for event in entity.events() {
            let native_event = if event.name() == "dynamic" {
                EventBuilder::new(event.name().clone(), event.cardinality())
                    .with_annotations(event.annotations().clone())
                    .build()
            } else {
                event.clone()
            };
            native_entity.try_insert_event(native_event)?;
        }
        native.try_insert_entity(native_entity.build())?;
    }
    for record in parsed.schema.records() {
        native.try_insert_record(record.clone())?;
    }
    let generated = generate(
        &native.build(),
        &Options {
            event_derives: &[
                "Debug",
                "::serde::Serialize",
                "::serde::Deserialize",
                "::bitcode::Encode",
                "::bincode::Encode",
                "::bincode::Decode",
            ],
            record_derives: &[
                "Debug",
                "::serde::Serialize",
                "::serde::Deserialize",
                "::bitcode::Encode",
                "::bincode::Encode",
                "::bincode::Decode",
            ],
            file_name: Some("bench_native.rs".to_owned()),
            ..Default::default()
        },
    )?;
    for warning in generated.warnings {
        println!("cargo:warning={warning}");
    }
    Ok(())
}
