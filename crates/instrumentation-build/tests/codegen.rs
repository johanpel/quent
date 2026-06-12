// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Public-API tests for the schema -> generated source pipeline.
//!
//! These exercise [`generate`] and [`generate_str`]. Generated source is
//! compared against `quote!`-built expectations, both normalised through
//! `prettyplease`, so the assertions are robust to formatting. Per-module
//! generation (events, records, type mapping) is covered by unit tests inside
//! those modules.
//!
//! Structural tests run with `GenerateOptions::default()` so no derive
//! attributes appear; derive behaviour is covered separately in its own cluster
//! of tests below.

use std::path::Path;

use quent_instrumentation_build::{GenerateError, GenerateOptions, generate, generate_str};
use quent_schema::DataType;
use quent_schema::Schema;
use quent_schema::test_utils::{entity, event, field, ident, record, schema};
use quote::quote;

/// Pretty-print tokens the same way the generator does.
fn pretty(tokens: proc_macro2::TokenStream) -> String {
    prettyplease::unparse(&syn::parse2::<syn::File>(tokens).expect("tokens form a valid file"))
}

fn connection_schema() -> Schema {
    schema(
        "Net",
        [entity(
            "Connection",
            [
                event(
                    "opened",
                    [
                        field("peer", DataType::String),
                        field("port", DataType::U16),
                    ],
                ),
                event("bytes_sent", [field("count", DataType::U64)]),
                event("closed", []),
            ],
        )],
        [],
    )
}

#[test]
fn generate_str_emits_records_then_events() {
    let s = schema(
        "M",
        [entity(
            "Conn",
            [event(
                "opened",
                [field("info", DataType::Record(ident("OnePrim")))],
            )],
        )],
        [record("OnePrim", [field("a", DataType::U8)])],
    );
    let expected = quote! {
        pub struct OnePrim {
            pub a: u8
        }
        pub enum ConnEvent {
            Opened { info: OnePrim }
        }
    };
    assert_eq!(
        generate_str(&s, &GenerateOptions::default()).unwrap(),
        pretty(expected)
    );
}

#[test]
fn empty_schema_produces_empty_output() {
    let s = schema("M", [], []);
    assert_eq!(generate_str(&s, &GenerateOptions::default()).unwrap(), "");
}

#[test]
fn generate_writes_default_file_name() {
    let s = connection_schema();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("gen_default");
    std::fs::create_dir_all(&dir).unwrap();
    let opts = GenerateOptions {
        out_dir: dir.clone(),
        ..Default::default()
    };
    let path = generate(&s, &opts).unwrap();
    // schema name "Net" -> "net.rs"
    assert_eq!(path, dir.join("net.rs"));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        generate_str(&s, &opts).unwrap(),
    );
}

#[test]
fn generate_honours_file_name_override() {
    let s = connection_schema();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("gen_override");
    std::fs::create_dir_all(&dir).unwrap();
    let opts = GenerateOptions {
        out_dir: dir.clone(),
        file_name: Some("custom.rs".to_owned()),
        ..Default::default()
    };
    let path = generate(&s, &opts).unwrap();
    assert_eq!(path, dir.join("custom.rs"));
    assert!(path.exists());
}

// Derive behaviour, isolated from the structural tests above.

/// A schema with both a record and an entity, so a single generation exercises
/// `record_derives` and `event_derives` independently.
fn record_and_entity_schema() -> Schema {
    schema(
        "M",
        [entity("E", [event("ev", [field("a", DataType::U8)])])],
        [record("R", [field("a", DataType::U8)])],
    )
}

#[test]
fn event_derives_apply_to_enums_only() {
    let opts = GenerateOptions {
        event_derives: &["Debug", "Clone"],
        ..Default::default()
    };
    let expected = quote! {
        pub struct R {
            pub a: u8
        }
        #[derive(Debug, Clone)]
        pub enum EEvent {
            Ev { a: u8 }
        }
    };
    assert_eq!(
        generate_str(&record_and_entity_schema(), &opts).unwrap(),
        pretty(expected)
    );
}

#[test]
fn record_derives_apply_to_structs_only() {
    let opts = GenerateOptions {
        record_derives: &["Debug", "Clone"],
        ..Default::default()
    };
    let expected = quote! {
        #[derive(Debug, Clone)]
        pub struct R {
            pub a: u8
        }
        pub enum EEvent {
            Ev { a: u8 }
        }
    };
    assert_eq!(
        generate_str(&record_and_entity_schema(), &opts).unwrap(),
        pretty(expected)
    );
}

#[test]
fn default_options_emit_no_derives() {
    let expected = quote! {
        pub struct R {
            pub a: u8
        }
        pub enum EEvent {
            Ev { a: u8 }
        }
    };
    assert_eq!(
        generate_str(&record_and_entity_schema(), &GenerateOptions::default()).unwrap(),
        pretty(expected)
    );
}

#[test]
fn invalid_derive_path_is_an_error() {
    let opts = GenerateOptions {
        event_derives: &["not a path!"],
        ..Default::default()
    };
    let err = generate_str(&record_and_entity_schema(), &opts).unwrap_err();
    assert!(matches!(err, GenerateError::InvalidDerive { .. }));
}
