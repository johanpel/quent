// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model::entity::Entity as _;
use quent_v2_model_ir::{
    data_type::DataType,
    entity::Entity,
    event::{Cardinality, EntityRefRole, EntityRefTarget, Event, EventField, EventFieldType},
    identifier::Identifier,
};

use source::entities::*;

use crate::utils::ident;

mod source;
mod utils;

fn payload_field(record_name: &str) -> EventField {
    EventField {
        name: Identifier::new_unchecked("payload"),
        docs: None,
        ty: EventFieldType::Payload(DataType::Record(ident(record_name))),
        conventions: Vec::new(),
    }
}

fn named_field(name: &str, ty: EventFieldType) -> EventField {
    EventField {
        name: Identifier::new_unchecked(name),
        docs: None,
        ty,
        conventions: Vec::new(),
    }
}

fn event(name: &str, cardinality: Cardinality, payload: Vec<EventField>) -> Event {
    Event {
        name: ident(name),
        docs: None,
        cardinality,
        payload,
        conventions: Vec::new(),
    }
}

fn entity(name: &str, events: Vec<Event>) -> Entity {
    Entity {
        name: ident(name),
        docs: None,
        events,
        fsm: None,
        conventions: Vec::new(),
    }
}

#[test]
fn unit() {
    assert_eq!(
        Unit::ir(),
        entity("Unit", vec![event("Unit", Cardinality::Once, vec![])]),
    );
    assert_eq!(
        Unit::ir_ref_target(),
        EntityRefTarget::Specific(ident("Unit"))
    );
}

#[test]
fn unit_braces() {
    assert_eq!(
        UnitBraces::ir(),
        entity(
            "UnitBraces",
            vec![event("UnitBraces", Cardinality::Once, vec![])],
        ),
    );
    assert_eq!(
        UnitBraces::ir_ref_target(),
        EntityRefTarget::Specific(ident("UnitBraces"))
    );
}

#[test]
fn struct_prim() {
    assert_eq!(
        StructPrim::ir(),
        entity(
            "StructPrim",
            vec![event(
                "StructPrim",
                Cardinality::Once,
                vec![payload_field("StructPrim")],
            )],
        ),
    );
    assert_eq!(
        StructPrim::ir_ref_target(),
        EntityRefTarget::Specific(ident("StructPrim"))
    );
}

#[test]
fn struct_multi_attrib() {
    assert_eq!(
        StructMultiAttrib::ir(),
        entity(
            "StructMultiAttrib",
            vec![event(
                "StructMultiAttrib",
                Cardinality::Once,
                vec![payload_field("StructMultiAttrib")],
            )],
        ),
    );
    assert_eq!(
        StructMultiAttrib::ir_ref_target(),
        EntityRefTarget::Specific(ident("StructMultiAttrib"))
    );
}

#[test]
fn enum_one_unit() {
    assert_eq!(
        EnumOneUnit::ir(),
        entity("EnumOneUnit", vec![event("A", Cardinality::Once, vec![])],),
    );
    assert_eq!(
        EnumOneUnit::ir_ref_target(),
        EntityRefTarget::Specific(ident("EnumOneUnit"))
    );
}

#[test]
fn enum_multi_unit() {
    assert_eq!(
        EnumMultiUnit::ir(),
        entity(
            "EnumMultiUnit",
            vec![
                event("A", Cardinality::Once, vec![]),
                event("B", Cardinality::Once, vec![]),
            ],
        ),
    );
    assert_eq!(
        EnumMultiUnit::ir_ref_target(),
        EntityRefTarget::Specific(ident("EnumMultiUnit"))
    );
}

#[test]
fn enum_single_attribs() {
    assert_eq!(
        EnumSingleAttribs::ir(),
        entity(
            "EnumSingleAttribs",
            vec![event(
                "A",
                Cardinality::Once,
                vec![payload_field("OnePrim")],
            )],
        ),
    );
    assert_eq!(
        EnumSingleAttribs::ir_ref_target(),
        EntityRefTarget::Specific(ident("EnumSingleAttribs"))
    );
}

#[test]
fn enum_multi_attribs() {
    assert_eq!(
        EnumMultiAttribs::ir(),
        entity(
            "EnumMultiAttribs",
            vec![
                event("A", Cardinality::Once, vec![payload_field("OnePrim")],),
                event("B", Cardinality::Once, vec![payload_field("MultiPrim")],),
            ],
        ),
    );
    assert_eq!(
        EnumMultiAttribs::ir_ref_target(),
        EntityRefTarget::Specific(ident("EnumMultiAttribs"))
    );
}

#[test]
fn enum_inline_attribs() {
    assert_eq!(
        EnumInlineAttribs::ir(),
        entity(
            "EnumInlineAttribs",
            vec![
                event(
                    "A",
                    Cardinality::Once,
                    vec![
                        named_field("x", EventFieldType::Payload(DataType::U8)),
                        named_field("y", EventFieldType::Payload(DataType::String)),
                    ],
                ),
                event("B", Cardinality::Once, vec![]),
            ],
        ),
    );
    assert_eq!(
        EnumInlineAttribs::ir_ref_target(),
        EntityRefTarget::Specific(ident("EnumInlineAttribs"))
    );
}

#[test]
fn entity_docstring_flows_to_ir() {
    let ir = DocumentedEntity::ir();
    assert_eq!(
        ir.docs.as_deref(),
        Some("A documented entity.\nLine two of the docstring."),
    );
    // The single struct-event also inherits the same docs.
    assert_eq!(
        ir.events[0].docs.as_deref(),
        Some("A documented entity.\nLine two of the docstring."),
    );
}

#[test]
fn variant_and_field_docstrings_flow_to_ir() {
    let ir = DocumentedVariantEnum::ir();
    // First variant: docs on the variant -> Event.docs.
    let alpha = ir.events.iter().find(|e| e.name == ident("Alpha")).unwrap();
    assert_eq!(alpha.docs.as_deref(), Some("Doc on a variant."));
    // Second variant: docs on the named field inside -> EventField.docs.
    let beta = ir.events.iter().find(|e| e.name == ident("Beta")).unwrap();
    let x = beta
        .payload
        .iter()
        .find(|f| f.name == Identifier::new_unchecked("x"))
        .unwrap();
    assert_eq!(x.docs.as_deref(), Some("Doc on an inline event field."));
}

#[test]
fn enum_builtin_attribs() {
    assert_eq!(
        EnumBuiltinAttribs::ir(),
        entity(
            "EnumBuiltinAttribs",
            vec![
                event(
                    "A",
                    Cardinality::Once,
                    vec![
                        named_field("x", EventFieldType::Payload(DataType::U8)),
                        named_field("y", EventFieldType::Payload(DataType::String)),
                        named_field(
                            "z",
                            EventFieldType::EntityRef {
                                role_type: EntityRefRole::Plain,
                                entity_type: EntityRefTarget::Specific(ident("EnumInlineAttribs")),
                            },
                        ),
                        named_field(
                            "d",
                            EventFieldType::EntityRef {
                                role_type: EntityRefRole::Scope,
                                entity_type: EntityRefTarget::Specific(ident("Unit")),
                            },
                        ),
                    ],
                ),
                event(
                    "B",
                    Cardinality::Once,
                    vec![named_field("k", EventFieldType::Payload(DataType::Uuid))],
                ),
            ],
        ),
    );
    assert_eq!(
        EnumBuiltinAttribs::ir_ref_target(),
        EntityRefTarget::Specific(ident("EnumBuiltinAttribs"))
    );
}
