use quent_exporter::NdjsonExporterOptions;
use quent_v2_instrumentation::{ExporterOptions, Observer};
use quent_v2_model::{Attributes, Entity};
use quent_v2_model_ir::{
    attributes::EntityRefTarget,
    entity::{Entity, ModelEntity},
    event::{Cardinality, Event, Field},
    value_type::{ModelEntityRefTarget, ValueType},
};
use uuid::Uuid;

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
                payload: vec![]
            }],
            qualifications: vec![],
            rust_path: utils::rust_path!("Unit0")
        }
    );
    assert_eq!(
        Unit0::model_entity_ref_target(),
        EntityRefTarget::Specific("Unit0".into())
    );

    let foo_obs = Unit0Observer::new(
        Uuid::now_v7(),
        ExporterOptions::Ndjson(NdjsonExporterOptions {
            output_dir: "foo".into(),
        }),
    )
    .unwrap();
    let foo_inst = foo_obs.handle();
    foo_inst.emit(Unit0).unwrap();

    #[derive(Entity)]
    struct Unit1 {}
    assert_eq!(
        Unit1::model_entity(),
        Entity {
            name: "Unit1".into(),
            events: vec![Event {
                name: "Unit1".into(),
                cardinality: Cardinality::Once,
                payload: vec![]
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
                payload: vec![Field::new(
                    "payload",
                    ValueType::Attributes("StructSinglePrim".into())
                )]
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
                payload: vec![Field::new(
                    "payload",
                    ValueType::Attributes("StructMultiFieldWithAttrib".into())
                )]
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

#[test]
#[allow(unused)]
fn enums() {
    #[derive(Entity)]
    enum SingleUnit {
        A,
    }
    assert_eq!(
        SingleUnit::model_entity(),
        Entity {
            name: "SingleUnit".into(),
            events: vec![Event {
                name: "A".into(),
                cardinality: Cardinality::Once,
                payload: vec![],
            }],
            qualifications: vec![],
            rust_path: utils::rust_path!("SingleUnit"),
        }
    );
    assert_eq!(
        SingleUnit::model_entity_ref_target(),
        EntityRefTarget::Specific("SingleUnit".into())
    );

    #[derive(Entity)]
    enum MultiUnit {
        A,
        B,
    }
    assert_eq!(
        MultiUnit::model_entity(),
        Entity {
            name: "MultiUnit".into(),
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
            rust_path: utils::rust_path!("MultiUnit"),
        }
    );
    assert_eq!(
        MultiUnit::model_entity_ref_target(),
        EntityRefTarget::Specific("MultiUnit".into())
    );

    #[derive(Attributes)]
    struct X;
    #[derive(Entity)]
    enum SingleUserPayload {
        A(X),
    }
    assert_eq!(
        SingleUserPayload::model_entity(),
        Entity {
            name: "SingleUserPayload".into(),
            events: vec![Event {
                name: "A".into(),
                cardinality: Cardinality::Once,
                payload: vec![Field::new("payload", ValueType::Attributes("X".into()))],
            }],
            qualifications: vec![],
            rust_path: utils::rust_path!("SingleUserPayload"),
        }
    );
    assert_eq!(
        SingleUserPayload::model_entity_ref_target(),
        EntityRefTarget::Specific("SingleUserPayload".into())
    );

    #[derive(Attributes)]
    struct Y {
        a: u8,
        b: String,
    };
    #[derive(Entity)]
    enum MultiUserPayload {
        A(X),
        B(Y),
    }
    assert_eq!(
        MultiUserPayload::model_entity(),
        Entity {
            name: "MultiUserPayload".into(),
            events: vec![
                Event {
                    name: "A".into(),
                    cardinality: Cardinality::Once,
                    payload: vec![Field::new("payload", ValueType::Attributes("X".into()))],
                },
                Event {
                    name: "B".into(),
                    cardinality: Cardinality::Once,
                    payload: vec![Field::new("payload", ValueType::Attributes("Y".into()))],
                },
            ],
            qualifications: vec![],
            rust_path: utils::rust_path!("MultiUserPayload"),
        }
    );
    assert_eq!(
        MultiUserPayload::model_entity_ref_target(),
        EntityRefTarget::Specific("MultiUserPayload".into())
    );
}
