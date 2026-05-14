use quent_v2_model::{
    Attributes, Entity,
    ir::{
        attributes::EntityRefTarget,
        entity::{Entity, ModelEntity},
        event::{Cardinality, Event, Payload},
        value_type::{ModelEntityRefTarget, ValueType},
    },
};

mod utils;

#[test]
fn unit_struct() {
    #[derive(Entity)]
    struct Unit0;
    assert_eq!(
        Unit0::model_entity(),
        Entity {
            name: "Unit0".into(),
            events: vec![Event {
                name: "Unit0".into(),
                cardinality: Cardinality::Once,
                payload: Payload::Value(ValueType::Attributes("Unit0".into()))
            }],
            qualifications: vec![],
            rust_path: utils::rust_path!("Unit0")
        }
    );
    assert_eq!(
        Unit0::model_entity_ref_target(),
        EntityRefTarget::Specific("Unit0".into())
    );

    #[derive(Entity)]
    struct Unit1 {}
    assert_eq!(
        Unit1::model_entity(),
        Entity {
            name: "Unit1".into(),
            events: vec![Event {
                name: "Unit1".into(),
                cardinality: Cardinality::Once,
                payload: Payload::Value(ValueType::Attributes("Unit1".into()))
            }],
            qualifications: vec![],
            rust_path: utils::rust_path!("Unit1")
        }
    );
    assert_eq!(
        Unit1::model_entity_ref_target(),
        EntityRefTarget::Specific("Unit1".into())
    );
}

#[test]
#[allow(unused)]
fn fields_struct() {
    #[derive(Entity)]
    struct StructSinglePrim {
        a: u8,
    };
    assert_eq!(
        StructSinglePrim::model_entity(),
        Entity {
            name: "StructSinglePrim".into(),
            events: vec![Event {
                name: "StructSinglePrim".into(),
                cardinality: Cardinality::Once,
                payload: Payload::Value(ValueType::Attributes("StructSinglePrim".into()))
            }],
            qualifications: vec![],
            rust_path: utils::rust_path!("StructSinglePrim")
        }
    );
    assert_eq!(
        StructSinglePrim::model_entity_ref_target(),
        EntityRefTarget::Specific("StructSinglePrim".into())
    );

    // TODO(johanpel): The Entity derive itself doesn't check whether X has an
    // IR, but model! does. This is not ideal from a compilation error pointing
    // to the right source perspective, so we could come up with a work-around
    // at some point. Keeping it simple for now by moving the error to model!.
    #[derive(Attributes)]
    struct X {
        a: u8,
    };
    #[derive(Entity)]
    struct StructMultiFieldWithAttrib {
        a: u8,
        b: X,
    };
    assert_eq!(
        StructMultiFieldWithAttrib::model_entity(),
        Entity {
            name: "StructMultiFieldWithAttrib".into(),
            events: vec![Event {
                name: "StructMultiFieldWithAttrib".into(),
                cardinality: Cardinality::Once,
                payload: Payload::Value(ValueType::Attributes("StructMultiFieldWithAttrib".into()))
            }],
            qualifications: vec![],
            rust_path: utils::rust_path!("StructMultiFieldWithAttrib")
        }
    );
    assert_eq!(
        StructMultiFieldWithAttrib::model_entity_ref_target(),
        EntityRefTarget::Specific("StructMultiFieldWithAttrib".into())
    );

    // TODO: struct with more value types including ref and resource usage
}
