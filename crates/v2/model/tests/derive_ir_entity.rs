// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model_ir::{
    entity::{Entity, ModelEntity},
    event::{Cardinality, Event, Field},
    value_type::ValueType,
};

use source::entities::*;

use crate::utils::ident;

mod source;
mod utils;

#[test]
fn unit_struct() {
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
#[allow(unused)]
fn fields_struct() {
    assert_eq!(
        StructPrim::model_entity(),
        Entity::new(
            ident("StructPrim"),
            vec![Event::new(
                ident("StructPrim"),
                Cardinality::Once,
                vec![Field::new(
                    EventValueType::Attribute,
                    ValueType::Attributes(ident("StructPrim")),
                )],
            )],
            vec![],
            utils::rust_path!("source::entities::StructPrim"),
        )
    );
    assert_eq!(
        StructPrim::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("StructPrim"))
    );

    assert_eq!(
        StructMultiAttrib::model_entity(),
        Entity::new(
            ident("StructMultiAttrib"),
            vec![Event::new(
                ident("StructMultiAttrib"),
                Cardinality::Once,
                vec![Field::new(
                    EventValueType::Attribute,
                    ValueType::Attributes(ident("StructMultiAttrib")),
                )],
            )],
            vec![],
            utils::rust_path!("source::entities::StructMultiAttrib"),
        )
    );
    assert_eq!(
        StructMultiAttrib::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("StructMultiAttrib"))
    );

    // TODO: struct with more value types including ref and resource usage
}

#[test]
#[allow(unused)]
fn enums() {
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

    assert_eq!(
        EnumSingleAttribs::model_entity(),
        Entity::new(
            ident("EnumSingleAttribs"),
            vec![Event::new(
                ident("A"),
                Cardinality::Once,
                vec![Field::new(
                    EventValueType::Attribute,
                    ValueType::Attributes(ident("OnePrim")),
                )],
            )],
            vec![],
            utils::rust_path!("source::entities::EnumSingleAttribs"),
        )
    );
    assert_eq!(
        EnumSingleAttribs::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("EnumSingleAttribs"))
    );

    assert_eq!(
        EnumMultiAttribs::model_entity(),
        Entity::new(
            ident("EnumMultiAttribs"),
            vec![
                Event::new(
                    ident("A"),
                    Cardinality::Once,
                    vec![Field::new(
                        EventValueType::Attribute,
                        ValueType::Attributes(ident("OnePrim")),
                    )],
                ),
                Event::new(
                    ident("B"),
                    Cardinality::Once,
                    vec![Field::new(
                        EventValueType::Attribute,
                        ValueType::Attributes(ident("MultiPrim")),
                    )],
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
