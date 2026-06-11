// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Schema -> event type definition tests.
//!
//! Generated token streams are compared against `quote!`-built expectations via
//! their `to_string()` rendering, which normalises whitespace through
//! proc-macro2's `Display`, so the assertions are robust to formatting.

use quent_instrumentation_build::{
    CodegenOptions, generate, generate_event_types, generate_event_types_str, generate_record_types,
};
use quent_schema::builder::{AnnotationsBuilder, EntityBuilder, EventBuilder, SchemaBuilder};
use quent_schema::test_utils::{entity, event, field, ident, record, schema};
use quent_schema::{Annotations, Cardinality, DataType, Field, Schema};
use quote::quote;

fn serde_opts() -> CodegenOptions {
    let serde = vec![
        "Debug".to_owned(),
        "Clone".to_owned(),
        "::serde::Serialize".to_owned(),
        "::serde::Deserialize".to_owned(),
    ];
    CodegenOptions {
        event_derives: serde.clone(),
        record_derives: serde,
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
        generate_event_types(&connection_schema(), &serde_opts()).to_string(),
        expected.to_string(),
    );
}

#[test]
fn event_enum_default_derives_have_no_serde() {
    let expected = quote! {
        #[derive(Debug, Clone)]
        pub enum ConnectionEvent {
            Opened { peer: String, port: u16 },
            BytesSent { count: u64 },
            Closed
        }
    };
    assert_eq!(
        generate_event_types(&connection_schema(), &CodegenOptions::default()).to_string(),
        expected.to_string(),
    );
}

#[test]
fn empty_derives_list_emits_no_derive_attribute() {
    let opts = CodegenOptions {
        event_derives: vec![],
        ..CodegenOptions::default()
    };
    let expected = quote! {
        pub enum ConnectionEvent {
            Opened { peer: String, port: u16 },
            BytesSent { count: u64 },
            Closed
        }
    };
    assert_eq!(
        generate_event_types(&connection_schema(), &opts).to_string(),
        expected.to_string(),
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
        #[derive(Debug, Clone)]
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
        generate_event_types(&s, &CodegenOptions::default()).to_string(),
        expected.to_string(),
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
        #[derive(Debug, Clone)]
        pub enum EEvent {
            #[doc = "event doc"]
            Ev {
                #[doc = "field doc"]
                x: u8
            }
        }
    };
    assert_eq!(
        generate_event_types(&s, &CodegenOptions::default()).to_string(),
        expected.to_string(),
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
        #[derive(Debug, Clone)]
        pub enum AlphaEvent {
            Started { id: u32 }
        }
        #[derive(Debug, Clone)]
        pub enum BetaEvent {
            Ended
        }
    };
    assert_eq!(
        generate_event_types(&s, &CodegenOptions::default()).to_string(),
        expected.to_string(),
    );
}

#[test]
fn entity_without_events_emits_empty_enum() {
    let s = schema("M", [entity("E", [])], []);
    let expected = quote! {
        #[derive(Debug, Clone)]
        pub enum EEvent {}
    };
    assert_eq!(
        generate_event_types(&s, &CodegenOptions::default()).to_string(),
        expected.to_string(),
    );
}

#[test]
fn empty_schema_produces_empty_output() {
    let s = schema("M", [], []);
    assert_eq!(
        generate_event_types(&s, &CodegenOptions::default()).to_string(),
        "",
    );
    assert_eq!(generate_event_types_str(&s, &CodegenOptions::default()), "");
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
    // Compared via prettyplease: a literal `>>>` lexes as joined shift tokens,
    // whereas the generator emits separate `>` puncts; both normalise equally.
    let expected = quote! {
        #[derive(Debug, Clone)]
        pub enum EEvent {
            Ev {
                nested: Option<Vec<Option<u8>>>,
                eref_list: ::quent_instrumentation_runtime::EntityRef<Vec<String>>
            }
        }
    };
    let expected =
        prettyplease::unparse(&syn::parse2::<syn::File>(expected).expect("expected parses"));
    assert_eq!(
        generate_event_types_str(&s, &CodegenOptions::default()),
        expected,
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
        #[derive(Debug, Clone)]
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
        generate_event_types(&s, &CodegenOptions::default()).to_string(),
        expected.to_string(),
    );
}

#[test]
fn docs_emitted_only_for_documented_elements() {
    let docs = |text: &str| AnnotationsBuilder::new().docs(text).build();
    let documented = Field::new(ident("a"), DataType::U8, docs("a doc"));
    let plain = Field::new(ident("b"), DataType::U8, Annotations::default());
    let ev = EventBuilder::new(ident("ev"), Cardinality::Once)
        .fields([documented, plain])
        .unwrap()
        .build(); // event itself undocumented
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
        #[derive(Debug, Clone)]
        pub enum EEvent {
            Ev {
                #[doc = "a doc"]
                a: u8,
                b: u8
            }
        }
    };
    assert_eq!(
        generate_event_types(&s, &CodegenOptions::default()).to_string(),
        expected.to_string(),
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
    let _ = generate_event_types(&s, &CodegenOptions::default());
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
        #[derive(Debug, Clone)]
        pub struct OnePrim {
            pub a: u8
        }
        #[derive(Debug, Clone)]
        pub struct Nested {
            pub inner: OnePrim,
            pub list: Vec<String>
        }
        #[derive(Debug, Clone)]
        pub struct Empty {}
    };
    assert_eq!(
        generate_record_types(&s, &CodegenOptions::default()).to_string(),
        expected.to_string(),
    );
}

#[test]
fn generate_emits_records_then_events() {
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
        #[derive(Debug, Clone)]
        pub struct OnePrim {
            pub a: u8
        }
        #[derive(Debug, Clone)]
        pub enum ConnEvent {
            Opened { info: OnePrim }
        }
    };
    assert_eq!(
        generate(&s, &CodegenOptions::default()).to_string(),
        expected.to_string(),
    );
}
