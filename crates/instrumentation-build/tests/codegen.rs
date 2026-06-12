// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Schema -> generated source tests.
//!
//! Generated source is compared against `quote!`-built expectations, both
//! normalised through `prettyplease`, so the assertions are robust to formatting.

use std::path::Path;

use quent_instrumentation_build::{
    GenerateOptions, generate, generate_event_types_str, generate_record_types_str, generate_str,
};
use quent_schema::builder::{AnnotationsBuilder, EntityBuilder, EventBuilder, SchemaBuilder};
use quent_schema::test_utils::{entity, event, field, ident, record, schema};
use quent_schema::{Annotations, Cardinality, DataType, Field, Schema};
use quote::quote;

/// Pretty-print tokens the same way the generator does.
fn pretty(tokens: proc_macro2::TokenStream) -> String {
    prettyplease::unparse(&syn::parse2::<syn::File>(tokens).expect("tokens form a valid file"))
}

fn debug_opts() -> GenerateOptions {
    GenerateOptions {
        event_derives: &["Debug"],
        record_derives: &["Debug"],
        ..Default::default()
    }
}

fn serde_opts() -> GenerateOptions {
    GenerateOptions {
        event_derives: &[
            "Debug",
            "Clone",
            "::serde::Serialize",
            "::serde::Deserialize",
        ],
        record_derives: &[
            "Debug",
            "Clone",
            "::serde::Serialize",
            "::serde::Deserialize",
        ],
        ..Default::default()
    }
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
fn event_enum_with_serde_derives() {
    let expected = quote! {
        #[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
        pub enum ConnectionEvent {
            Opened { peer: String, port: u16 },
            BytesSent { count: u64 },
            Closed
        }
    };
    assert_eq!(
        generate_event_types_str(&connection_schema(), &serde_opts()).unwrap(),
        pretty(expected),
    );
}

#[test]
fn default_options_emit_no_derive_attribute() {
    // `GenerateOptions::default()` carries no derives.
    let expected = quote! {
        pub enum ConnectionEvent {
            Opened { peer: String, port: u16 },
            BytesSent { count: u64 },
            Closed
        }
    };
    assert_eq!(
        generate_event_types_str(&connection_schema(), &GenerateOptions::default()).unwrap(),
        pretty(expected),
    );
}

#[test]
fn data_type_mapping_covers_every_variant() {
    let s = schema(
        "M",
        [entity(
            "E",
            [event(
                "ev",
                [
                    field("b", DataType::Bool),
                    field("id", DataType::Uuid),
                    field("text", DataType::String),
                    field("n", DataType::U32),
                    field("opt", DataType::Option(Box::new(DataType::I32))),
                    field("list", DataType::List(Box::new(DataType::String))),
                    field("rec", DataType::Record(ident("SomeRecord"))),
                    field("dynrec", DataType::DynamicRecord),
                    field(
                        "eref",
                        DataType::EntityRef {
                            data: None,
                            annotations: Annotations::default(),
                        },
                    ),
                    field(
                        "eref_payload",
                        DataType::EntityRef {
                            data: Some(Box::new(DataType::U64)),
                            annotations: Annotations::default(),
                        },
                    ),
                ],
            )],
        )],
        [],
    );
    let expected = quote! {
        #[derive(Debug)]
        pub enum EEvent {
            Ev {
                b: bool,
                id: ::uuid::Uuid,
                text: String,
                n: u32,
                opt: Option<i32>,
                list: Vec<String>,
                rec: SomeRecord,
                dynrec: ::quent_attributes::CustomAttributes,
                eref: ::quent_instrumentation_runtime::EntityRef,
                eref_payload: ::quent_instrumentation_runtime::EntityRef<u64>
            }
        }
    };
    assert_eq!(
        generate_event_types_str(&s, &debug_opts()).unwrap(),
        pretty(expected)
    );
}

#[test]
fn docs_annotations_become_doc_attributes() {
    let docs = |text: &str| AnnotationsBuilder::new().docs(text).build();
    let field_x = Field::new(ident("x"), DataType::U8, docs("field doc"));
    let ev = EventBuilder::new(ident("ev"), Cardinality::Once)
        .fields([field_x])
        .unwrap()
        .annotations(docs("event doc"))
        .build();
    let en = EntityBuilder::new(ident("E"))
        .events([ev])
        .unwrap()
        .annotations(docs("entity doc"))
        .build();
    let s = SchemaBuilder::new(ident("M"))
        .entities([en])
        .unwrap()
        .build();

    let expected = quote! {
        #[doc = "entity doc"]
        #[derive(Debug)]
        pub enum EEvent {
            #[doc = "event doc"]
            Ev {
                #[doc = "field doc"]
                x: u8
            }
        }
    };
    assert_eq!(
        generate_event_types_str(&s, &debug_opts()).unwrap(),
        pretty(expected)
    );
}

#[test]
fn multiple_entities_emit_in_declaration_order() {
    let s = schema(
        "M",
        [
            entity("Alpha", [event("started", [field("id", DataType::U32)])]),
            entity("Beta", [event("ended", [])]),
        ],
        [],
    );
    let expected = quote! {
        #[derive(Debug)]
        pub enum AlphaEvent {
            Started { id: u32 }
        }
        #[derive(Debug)]
        pub enum BetaEvent {
            Ended
        }
    };
    assert_eq!(
        generate_event_types_str(&s, &debug_opts()).unwrap(),
        pretty(expected)
    );
}

#[test]
fn entity_without_events_emits_empty_enum() {
    let s = schema("M", [entity("E", [])], []);
    let expected = quote! {
        #[derive(Debug)]
        pub enum EEvent {}
    };
    assert_eq!(
        generate_event_types_str(&s, &debug_opts()).unwrap(),
        pretty(expected)
    );
}

#[test]
fn empty_schema_produces_empty_output() {
    let s = schema("M", [], []);
    assert_eq!(generate_event_types_str(&s, &debug_opts()).unwrap(), "");
    assert_eq!(generate_str(&s, &debug_opts()).unwrap(), "");
}

#[test]
fn nested_container_types_recurse() {
    let s = schema(
        "M",
        [entity(
            "E",
            [event(
                "ev",
                [
                    field(
                        "nested",
                        DataType::Option(Box::new(DataType::List(Box::new(DataType::Option(
                            Box::new(DataType::U8),
                        ))))),
                    ),
                    field(
                        "eref_list",
                        DataType::EntityRef {
                            data: Some(Box::new(DataType::List(Box::new(DataType::String)))),
                            annotations: Annotations::default(),
                        },
                    ),
                ],
            )],
        )],
        [],
    );
    let expected = quote! {
        #[derive(Debug)]
        pub enum EEvent {
            Ev {
                nested: Option<Vec<Option<u8>>>,
                eref_list: ::quent_instrumentation_runtime::EntityRef<Vec<String>>
            }
        }
    };
    assert_eq!(
        generate_event_types_str(&s, &debug_opts()).unwrap(),
        pretty(expected)
    );
}

#[test]
fn keyword_and_digit_identifiers_are_handled() {
    let s = schema(
        "M",
        [entity(
            "Sig",
            // event named after a keyword -> Pascal "Type" needs no escape
            [event(
                "type",
                [
                    field("u8", DataType::U8),     // digit-safe: stays u8
                    field("type", DataType::U8),   // keyword field -> r#type
                    field("self", DataType::U8),   // un-rawable keyword -> self_
                    field("http2", DataType::U32), // digit-safe: stays http2
                ],
            )],
        )],
        [],
    );
    let expected = quote! {
        #[derive(Debug)]
        pub enum SigEvent {
            Type {
                u8: u8,
                r#type: u8,
                self_: u8,
                http2: u32
            }
        }
    };
    assert_eq!(
        generate_event_types_str(&s, &debug_opts()).unwrap(),
        pretty(expected)
    );
}

#[test]
#[should_panic(expected = "maximum depth")]
fn excessive_type_nesting_panics() {
    let mut ty = DataType::U8;
    for _ in 0..(quent_instrumentation_build::MAX_TYPE_DEPTH + 5) {
        ty = DataType::Option(Box::new(ty));
    }
    let s = schema("M", [entity("E", [event("ev", [field("deep", ty)])])], []);
    let _ = generate_event_types_str(&s, &debug_opts());
}

#[test]
fn record_structs_emit_public_fields() {
    let s = schema(
        "M",
        [],
        [
            record("OnePrim", [field("a", DataType::U8)]),
            record(
                "Nested",
                [
                    field("inner", DataType::Record(ident("OnePrim"))),
                    field("list", DataType::List(Box::new(DataType::String))),
                ],
            ),
            record("Empty", []),
        ],
    );
    let expected = quote! {
        #[derive(Debug)]
        pub struct OnePrim {
            pub a: u8
        }
        #[derive(Debug)]
        pub struct Nested {
            pub inner: OnePrim,
            pub list: Vec<String>
        }
        #[derive(Debug)]
        pub struct Empty {}
    };
    assert_eq!(
        generate_record_types_str(&s, &debug_opts()).unwrap(),
        pretty(expected)
    );
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
        #[derive(Debug)]
        pub struct OnePrim {
            pub a: u8
        }
        #[derive(Debug)]
        pub enum ConnEvent {
            Opened { info: OnePrim }
        }
    };
    assert_eq!(generate_str(&s, &debug_opts()).unwrap(), pretty(expected));
}

#[test]
fn generate_writes_default_file_name() {
    let s = connection_schema();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("gen_default");
    std::fs::create_dir_all(&dir).unwrap();
    let opts = GenerateOptions {
        out_dir: dir.clone(),
        ..debug_opts()
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
        ..debug_opts()
    };
    let path = generate(&s, &opts).unwrap();
    assert_eq!(path, dir.join("custom.rs"));
    assert!(path.exists());
}
