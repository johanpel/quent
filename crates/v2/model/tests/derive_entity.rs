use quent_v2_model::{
    Entity,
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
    struct Unit1 {};
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
