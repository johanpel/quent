// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model_ir::{
    entity::{Entity, ModelEntity},
    event::{
        Cardinality, EntityRefRole, EntityRefTarget, Event, EventField, EventFieldType,
        ModelEntityRefTarget,
    },
    identifier::Identifier,
    value_type::ValueType,
};

use source::entities::*;

use crate::utils::ident;

mod source;
mod utils;

#[test]
fn unit() {
    assert_eq!(
        Unit::model_entity(),
        Entity::new(
            ident("Unit"),
            vec![Event::new(ident("Unit"), Cardinality::Once, vec![])],
            vec![],
            utils::rust_path!("source::entities::Unit"),
        )
    );
    assert_eq!(
        Unit::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("Unit"))
    );
}

#[test]
fn unit_braces() {
    assert_eq!(
        UnitBraces::model_entity(),
        Entity::new(
            ident("UnitBraces"),
            vec![Event::new(ident("UnitBraces"), Cardinality::Once, vec![])],
            vec![],
            utils::rust_path!("source::entities::UnitBraces"),
        )
    );
    assert_eq!(
        UnitBraces::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("UnitBraces"))
    );
}

#[test]
fn struct_prim() {
    assert_eq!(
        StructPrim::model_entity(),
        Entity::new(
            ident("StructPrim"),
            vec![Event::new(
                ident("StructPrim"),
                Cardinality::Once,
                vec![EventField::from_type(EventFieldType::Payload(
                    ValueType::Attributes(ident("StructPrim")),
                ))],
            )],
            vec![],
            utils::rust_path!("source::entities::StructPrim"),
        )
    );
    assert_eq!(
        StructPrim::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("StructPrim"))
    );
}

#[test]
fn struct_multi_attrib() {
    assert_eq!(
        StructMultiAttrib::model_entity(),
        Entity::new(
            ident("StructMultiAttrib"),
            vec![Event::new(
                ident("StructMultiAttrib"),
                Cardinality::Once,
                vec![EventField::from_type(EventFieldType::Payload(
                    ValueType::Attributes(ident("StructMultiAttrib")),
                ))],
            )],
            vec![],
            utils::rust_path!("source::entities::StructMultiAttrib"),
        )
    );
    assert_eq!(
        StructMultiAttrib::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("StructMultiAttrib"))
    );
}

#[test]
fn enum_one_unit() {
    assert_eq!(
        EnumOneUnit::model_entity(),
        Entity::new(
            ident("EnumOneUnit"),
            vec![Event::new(ident("A"), Cardinality::Once, vec![])],
            vec![],
            utils::rust_path!("source::entities::EnumOneUnit"),
        )
    );
    assert_eq!(
        EnumOneUnit::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("EnumOneUnit"))
    );
}

#[test]
fn enum_multi_unit() {
    assert_eq!(
        EnumMultiUnit::model_entity(),
        Entity::new(
            ident("EnumMultiUnit"),
            vec![
                Event::new(ident("A"), Cardinality::Once, vec![]),
                Event::new(ident("B"), Cardinality::Once, vec![]),
            ],
            vec![],
            utils::rust_path!("source::entities::EnumMultiUnit"),
        )
    );
    assert_eq!(
        EnumMultiUnit::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("EnumMultiUnit"))
    );
}

#[test]
fn enum_single_attribs() {
    assert_eq!(
        EnumSingleAttribs::model_entity(),
        Entity::new(
            ident("EnumSingleAttribs"),
            vec![Event::new(
                ident("A"),
                Cardinality::Once,
                vec![EventField::from_type(EventFieldType::Payload(
                    ValueType::Attributes(ident("OnePrim")),
                ))],
            )],
            vec![],
            utils::rust_path!("source::entities::EnumSingleAttribs"),
        )
    );
    assert_eq!(
        EnumSingleAttribs::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("EnumSingleAttribs"))
    );
}

#[test]
fn enum_multi_attribs() {
    assert_eq!(
        EnumMultiAttribs::model_entity(),
        Entity::new(
            ident("EnumMultiAttribs"),
            vec![
                Event::new(
                    ident("A"),
                    Cardinality::Once,
                    vec![EventField::from_type(EventFieldType::Payload(
                        ValueType::Attributes(ident("OnePrim")),
                    ))],
                ),
                Event::new(
                    ident("B"),
                    Cardinality::Once,
                    vec![EventField::from_type(EventFieldType::Payload(
                        ValueType::Attributes(ident("MultiPrim")),
                    ))],
                ),
            ],
            vec![],
            utils::rust_path!("source::entities::EnumMultiAttribs"),
        )
    );
    assert_eq!(
        EnumMultiAttribs::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("EnumMultiAttribs"))
    );
}

#[test]
fn enum_inline_attribs() {
    assert_eq!(
        EnumInlineAttribs::model_entity(),
        Entity::new(
            ident("EnumInlineAttribs"),
            vec![
                Event::new(
                    ident("A"),
                    Cardinality::Once,
                    vec![
                        EventField::new(
                            Identifier::new_unchecked("x"),
                            EventFieldType::Payload(ValueType::U8),
                        ),
                        EventField::new(
                            Identifier::new_unchecked("y"),
                            EventFieldType::Payload(ValueType::String),
                        ),
                    ],
                ),
                Event::new(ident("B"), Cardinality::Once, vec![]),
            ],
            vec![],
            utils::rust_path!("source::entities::EnumInlineAttribs"),
        )
    );
    assert_eq!(
        EnumInlineAttribs::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("EnumInlineAttribs"))
    );
}

#[test]
fn enum_builtin_attribs() {
    assert_eq!(
        EnumBuiltinAttribs::model_entity(),
        Entity::new(
            ident("EnumBuiltinAttribs"),
            vec![
                Event::new(
                    ident("A"),
                    Cardinality::Once,
                    vec![
                        EventField::new(
                            Identifier::new_unchecked("x"),
                            EventFieldType::Payload(ValueType::U8),
                        ),
                        EventField::new(
                            Identifier::new_unchecked("y"),
                            EventFieldType::Payload(ValueType::String),
                        ),
                        EventField::new(
                            Identifier::new_unchecked("z"),
                            EventFieldType::EntityRef {
                                role_type: EntityRefRole::Plain,
                                entity_type: EntityRefTarget::Specific(ident("EnumInlineAttribs")),
                            },
                        ),
                        EventField::new(
                            Identifier::new_unchecked("d"),
                            EventFieldType::EntityRef {
                                role_type: EntityRefRole::Scope,
                                entity_type: EntityRefTarget::Specific(ident("Unit")),
                            },
                        ),
                    ],
                ),
                Event::new(
                    ident("B"),
                    Cardinality::Once,
                    vec![EventField::new(
                        Identifier::new_unchecked("k"),
                        EventFieldType::Payload(ValueType::Uuid),
                    )],
                ),
            ],
            vec![],
            utils::rust_path!("source::entities::EnumBuiltinAttribs"),
        )
    );
    assert_eq!(
        EnumBuiltinAttribs::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("EnumBuiltinAttribs"))
    );
}
