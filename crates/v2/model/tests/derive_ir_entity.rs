use quent_v2_model_ir::{
    attributes::EntityRefTarget,
    entity::{Entity, ModelEntity},
    event::{Cardinality, Event, Field},
    value_type::{ModelEntityRefTarget, ValueType},
};

use source::entities::*;

mod source;
mod utils;

#[test]
fn unit_struct() {
    assert_eq!(
        Unit::model_entity(),
        Entity {
            name: "Unit".into(),
            events: vec![Event {
                name: "Unit".into(),
                cardinality: Cardinality::Once,
                payload: vec![],
            }],
            qualifications: vec![],
            rust_path: utils::rust_path!("source::entities::Unit")
        }
    );
    assert_eq!(
        Unit::model_entity_ref_target(),
        EntityRefTarget::Specific("Unit".into())
    );

    assert_eq!(
        UnitBraces::model_entity(),
        Entity {
            name: "UnitBraces".into(),
            events: vec![Event {
                name: "UnitBraces".into(),
                cardinality: Cardinality::Once,
                payload: vec![],
            }],
            qualifications: vec![],
            rust_path: utils::rust_path!("source::entities::UnitBraces")
        }
    );
    assert_eq!(
        UnitBraces::model_entity_ref_target(),
        EntityRefTarget::Specific("UnitBraces".into())
    );
}

#[test]
#[allow(unused)]
fn fields_struct() {
    assert_eq!(
        StructPrim::model_entity(),
        Entity {
            name: "StructPrim".into(),
            events: vec![Event {
                name: "StructPrim".into(),
                cardinality: Cardinality::Once,
                payload: vec![Field::new(
                    "payload",
                    ValueType::Attributes("StructPrim".into())
                )]
            }],
            qualifications: vec![],
            rust_path: utils::rust_path!("source::entities::StructPrim")
        }
    );
    assert_eq!(
        StructPrim::model_entity_ref_target(),
        EntityRefTarget::Specific("StructPrim".into())
    );

    assert_eq!(
        StructMultiAttrib::model_entity(),
        Entity {
            name: "StructMultiAttrib".into(),
            events: vec![Event {
                name: "StructMultiAttrib".into(),
                cardinality: Cardinality::Once,
                payload: vec![Field::new(
                    "payload",
                    ValueType::Attributes("StructMultiAttrib".into())
                )]
            }],
            qualifications: vec![],
            rust_path: utils::rust_path!("source::entities::StructMultiAttrib")
        }
    );
    assert_eq!(
        StructMultiAttrib::model_entity_ref_target(),
        EntityRefTarget::Specific("StructMultiAttrib".into())
    );

    // TODO: struct with more value types including ref and resource usage
}

#[test]
#[allow(unused)]
fn enums() {
    assert_eq!(
        EnumOneUnit::model_entity(),
        Entity {
            name: "EnumOneUnit".into(),
            events: vec![Event {
                name: "A".into(),
                cardinality: Cardinality::Once,
                payload: vec![],
            }],
            qualifications: vec![],
            rust_path: utils::rust_path!("source::entities::EnumOneUnit"),
        }
    );
    assert_eq!(
        EnumOneUnit::model_entity_ref_target(),
        EntityRefTarget::Specific("EnumOneUnit".into())
    );

    assert_eq!(
        EnumMultiUnit::model_entity(),
        Entity {
            name: "EnumMultiUnit".into(),
            events: vec![
                Event {
                    name: "A".into(),
                    cardinality: Cardinality::Once,
                    payload: vec![],
                },
                Event {
                    name: "B".into(),
                    cardinality: Cardinality::Once,
                    payload: vec![],
                },
            ],
            qualifications: vec![],
            rust_path: utils::rust_path!("source::entities::EnumMultiUnit"),
        }
    );
    assert_eq!(
        EnumMultiUnit::model_entity_ref_target(),
        EntityRefTarget::Specific("EnumMultiUnit".into())
    );

    assert_eq!(
        EnumSingleAttribs::model_entity(),
        Entity {
            name: "EnumSingleAttribs".into(),
            events: vec![Event {
                name: "A".into(),
                cardinality: Cardinality::Once,
                payload: vec![Field::new(
                    "payload",
                    ValueType::Attributes("OnePrim".into())
                )],
            }],
            qualifications: vec![],
            rust_path: utils::rust_path!("source::entities::EnumSingleAttribs"),
        }
    );
    assert_eq!(
        EnumSingleAttribs::model_entity_ref_target(),
        EntityRefTarget::Specific("EnumSingleAttribs".into())
    );

    assert_eq!(
        EnumMultiAttribs::model_entity(),
        Entity {
            name: "EnumMultiAttribs".into(),
            events: vec![
                Event {
                    name: "A".into(),
                    cardinality: Cardinality::Once,
                    payload: vec![Field::new(
                        "payload",
                        ValueType::Attributes("OnePrim".into())
                    )],
                },
                Event {
                    name: "B".into(),
                    cardinality: Cardinality::Once,
                    payload: vec![Field::new(
                        "payload",
                        ValueType::Attributes("MultiPrim".into())
                    )],
                },
            ],
            qualifications: vec![],
            rust_path: utils::rust_path!("source::entities::EnumMultiAttribs"),
        }
    );
    assert_eq!(
        EnumMultiAttribs::model_entity_ref_target(),
        EntityRefTarget::Specific("EnumMultiAttribs".into())
    );
}
