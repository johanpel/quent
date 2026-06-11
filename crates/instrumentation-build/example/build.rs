// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Builds a demo schema and generates instrumentation source into `OUT_DIR`.
//! The path is reported via a `cargo:warning`, and `src/main.rs` prints the
//! source so it can be inspected with `cargo run`.

use std::{env, fs, path::Path};

use quent_instrumentation_build::{CodegenOptions, generate_str};
use quent_schema::builder::{
    AnnotationsBuilder, EntityBuilder, EventBuilder, RecordBuilder, SchemaBuilder,
};
use quent_schema::{Cardinality, DataType, Field, Identifier};

fn id(s: &str) -> Identifier {
    Identifier::try_new(s).expect("valid identifier")
}

fn field(name: &str, ty: DataType) -> Field {
    Field::new(id(name), ty, Default::default())
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let endpoint = RecordBuilder::new(id("Endpoint"))
        .annotations(
            AnnotationsBuilder::new()
                .docs("A network endpoint.")
                .build(),
        )
        .fields([
            field("host", DataType::String),
            field("port", DataType::U16),
        ])
        .unwrap()
        .build();

    let meta = RecordBuilder::new(id("Meta"))
        .fields([
            field("tags", DataType::List(Box::new(DataType::String))),
            field("extra", DataType::DynamicRecord),
        ])
        .unwrap()
        .build();

    let connection = EntityBuilder::new(id("Connection"))
        .annotations(
            AnnotationsBuilder::new()
                .docs("A client connection.")
                .build(),
        )
        .events([
            EventBuilder::new(id("opened"), Cardinality::Once)
                .fields([
                    field("peer", DataType::Record(id("Endpoint"))),
                    field("session", DataType::Uuid),
                ])
                .unwrap()
                .build(),
            EventBuilder::new(id("data"), Cardinality::Multi)
                .fields([
                    field("bytes", DataType::U64),
                    field(
                        "meta",
                        DataType::Option(Box::new(DataType::Record(id("Meta")))),
                    ),
                ])
                .unwrap()
                .build(),
            EventBuilder::new(id("closed"), Cardinality::Once).build(),
        ])
        .unwrap()
        .build();

    let schema = SchemaBuilder::new(id("Demo"))
        .records([endpoint, meta])
        .unwrap()
        .entities([connection])
        .unwrap()
        .build();

    let serde = vec![
        "Debug".to_owned(),
        "Clone".to_owned(),
        "::serde::Serialize".to_owned(),
        "::serde::Deserialize".to_owned(),
    ];
    let opts = CodegenOptions {
        event_derives: serde.clone(),
        record_derives: serde,
    };

    let source = generate_str(&schema, &opts);

    // prost/tonic style: emit one file named after the schema into OUT_DIR; the
    // consumer pulls it in with `include!`.
    let file_name = format!("{}.rs", schema.name().to_string().to_lowercase());
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join(&file_name);
    fs::write(&out, source).expect("write generated source");
    println!(
        "cargo:warning=generated instrumentation written to {}",
        out.display()
    );
}
