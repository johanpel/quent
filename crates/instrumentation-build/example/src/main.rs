// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Demo of `quent-instrumentation-build`, used prost/tonic-style: the schema
//! lives in `build.rs`, which writes the generated source to `OUT_DIR/<schema>.rs`
//! (here `demo.rs`); the consumer pulls it in with `include!`. Run with
//! `cargo run -p quent-instrumentation-build-example` to print that source and
//! a sample serialized event.

/// Generated instrumentation for the `Demo` schema.
pub mod demo {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/demo.rs"));
}

fn main() {
    print!("{}", include_str!(concat!(env!("OUT_DIR"), "/demo.rs")));

    let opened = demo::ConnectionEvent::Opened {
        peer: demo::Endpoint {
            host: "localhost".to_owned(),
            port: 8080,
        },
        session: uuid::Uuid::nil(),
    };
    eprintln!("sample event: {}", serde_json::to_string(&opened).unwrap());
}
