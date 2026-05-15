use quent_v2_model::{
    Attributes,
    entity_ref::{AnyEntity, EntityRef},
};
use quent_v2_model_ir::{
    attributes::{Attributes, EntityRefKind, EntityRefTarget, Field, ModelAttributes},
    value_type::{ModelValueType, ValueType},
};

mod utils;

// Unit structs
#[test]
fn unit() {
    #[derive(Attributes)]
    struct Unit0;
    assert_eq!(
        Unit0::model_attributes(),
        Attributes {
            name: String::from("Unit0"),
            fields: vec![],
            rust_path: utils::rust_path!("Unit0"),
        }
    );
    assert_eq!(Unit0::model_value_type(), ValueType::attributes("Unit0"));

    #[derive(Attributes)]
    struct Unit1 {}
    assert_eq!(
        Unit1::model_attributes(),
        Attributes {
            name: String::from("Unit1"),
            fields: vec![],
            rust_path: utils::rust_path!("Unit1"),
        }
    );
    assert_eq!(Unit1::model_value_type(), ValueType::attributes("Unit1"));
}

// Single field structs
#[test]
#[allow(unused)]
fn single() {
    #[derive(Attributes)]
    struct SinglePrimitive {
        a: u8,
    }
    assert_eq!(
        SinglePrimitive::model_attributes(),
        Attributes {
            name: String::from("SinglePrimitive"),
            fields: vec![Field {
                name: String::from("a"),
                ty: ValueType::U8,
            }],
            rust_path: utils::rust_path!("SinglePrimitive"),
        }
    );
    assert_eq!(
        SinglePrimitive::model_value_type(),
        ValueType::attributes("SinglePrimitive")
    );

    #[derive(Attributes)]
    struct SingleNested {
        a: SinglePrimitive,
    }
    assert_eq!(
        SingleNested::model_attributes(),
        Attributes {
            name: String::from("SingleNested"),
            fields: vec![Field {
                name: String::from("a"),
                ty: ValueType::Attributes(String::from("SinglePrimitive")),
            }],
            rust_path: utils::rust_path!("SingleNested"),
        }
    );
    assert_eq!(
        SingleNested::model_value_type(),
        ValueType::attributes("SingleNested")
    );

    #[derive(Attributes)]
    struct SingleList {
        a: Vec<u8>,
    }
    assert_eq!(
        SingleList::model_attributes(),
        Attributes {
            name: String::from("SingleList"),
            fields: vec![Field {
                name: String::from("a"),
                ty: ValueType::List(Box::new(ValueType::U8)),
            }],
            rust_path: utils::rust_path!("SingleList"),
        }
    );
    assert_eq!(
        SingleList::model_value_type(),
        ValueType::attributes("SingleList")
    );

    #[derive(Attributes)]
    struct SingleListNested {
        a: Vec<SinglePrimitive>,
    }
    assert_eq!(
        SingleListNested::model_attributes(),
        Attributes {
            name: String::from("SingleListNested"),
            fields: vec![Field {
                name: String::from("a"),
                ty: ValueType::List(Box::new(ValueType::Attributes(String::from(
                    "SinglePrimitive"
                ),))),
            }],
            rust_path: utils::rust_path!("SingleListNested"),
        }
    );
    assert_eq!(
        SingleListNested::model_value_type(),
        ValueType::attributes("SingleListNested")
    );

    #[derive(Attributes)]
    struct SingleListListPrimitive {
        a: Vec<Vec<u8>>,
    }
    assert_eq!(
        SingleListListPrimitive::model_attributes(),
        Attributes {
            name: String::from("SingleListListPrimitive"),
            fields: vec![Field {
                name: String::from("a"),
                ty: ValueType::List(Box::new(ValueType::List(Box::new(ValueType::U8,)))),
            }],
            rust_path: utils::rust_path!("SingleListListPrimitive"),
        }
    );
    assert_eq!(
        SingleListListPrimitive::model_value_type(),
        ValueType::attributes("SingleListListPrimitive")
    );

    #[derive(Attributes)]
    struct SingleListListNested {
        a: Vec<Vec<SinglePrimitive>>,
    }
    assert_eq!(
        SingleListListNested::model_attributes(),
        Attributes {
            name: String::from("SingleListListNested"),
            fields: vec![Field {
                name: String::from("a"),
                ty: ValueType::List(Box::new(ValueType::List(Box::new(ValueType::Attributes(
                    String::from("SinglePrimitive")
                ),)))),
            }],
            rust_path: utils::rust_path!("SingleListListNested"),
        }
    );
    assert_eq!(
        SingleListListNested::model_value_type(),
        ValueType::attributes("SingleListListNested")
    );
}

// Multiple field structs
#[test]
#[allow(unused)]
fn multi() {
    #[derive(Attributes)]
    struct MultiPrimitives {
        a: u8,
        b: String,
    }
    assert_eq!(
        MultiPrimitives::model_attributes(),
        Attributes {
            name: String::from("MultiPrimitives"),
            fields: vec![
                Field {
                    name: String::from("a"),
                    ty: ValueType::U8,
                },
                Field {
                    name: String::from("b"),
                    ty: ValueType::String,
                },
            ],
            rust_path: utils::rust_path!("MultiPrimitives"),
        }
    );
    assert_eq!(
        MultiPrimitives::model_value_type(),
        ValueType::attributes("MultiPrimitives")
    );

    #[derive(Attributes)]
    struct MultiMixed {
        a: u8,
        b: MultiPrimitives,
        c: Vec<u16>,
        d: String,
    }
    assert_eq!(
        MultiMixed::model_attributes(),
        Attributes {
            name: String::from("MultiMixed"),
            fields: vec![
                Field {
                    name: String::from("a"),
                    ty: ValueType::U8,
                },
                Field {
                    name: String::from("b"),
                    ty: ValueType::Attributes(String::from("MultiPrimitives")),
                },
                Field {
                    name: String::from("c"),
                    ty: ValueType::List(Box::new(ValueType::U16)),
                },
                Field {
                    name: String::from("d"),
                    ty: ValueType::String,
                },
            ],
            rust_path: utils::rust_path!("MultiMixed"),
        }
    );
    assert_eq!(
        MultiMixed::model_value_type(),
        ValueType::attributes("MultiMixed")
    );

    #[derive(Attributes)]
    struct MultiWithOption {
        a: u8,
        b: Option<String>,
    }
    assert_eq!(
        MultiWithOption::model_attributes(),
        Attributes {
            name: String::from("MultiWithOption"),
            fields: vec![
                Field {
                    name: String::from("a"),
                    ty: ValueType::U8,
                },
                Field {
                    name: String::from("b"),
                    ty: ValueType::Option(Box::new(ValueType::String)),
                },
            ],
            rust_path: utils::rust_path!("MultiWithOption"),
        }
    );
    assert_eq!(
        MultiWithOption::model_value_type(),
        ValueType::attributes("MultiWithOption")
    );
}

// All value types
#[test]
#[allow(unused)]
fn all_value_types() {
    #[derive(Attributes)]
    struct Nested {
        x: u8,
    }

    // TODO(johanpel):
    // #[derive(Resource)]
    // struct UnitResource;

    #[derive(Attributes)]
    struct AllValues {
        a_bool: bool,
        a_uuid: uuid::Uuid,
        a_string: String,
        a_u8: u8,
        a_u16: u16,
        a_u32: u32,
        a_u64: u64,
        a_i8: i8,
        a_i16: i16,
        a_i32: i32,
        a_i64: i64,
        a_f32: f32,
        a_f64: f64,
        a_option: Option<u64>,
        a_list: Vec<u64>,
        a_nested: Nested,
        a_custom: quent_attributes::CustomAttributes,
        a_entity_ref: EntityRef<AnyEntity>,
        // a_usage: Usage<UnitResource>,
    }

    assert_eq!(
        AllValues::model_attributes(),
        Attributes {
            name: String::from("AllValues"),
            fields: vec![
                Field {
                    name: String::from("a_bool"),
                    ty: ValueType::Bool,
                },
                Field {
                    name: String::from("a_uuid"),
                    ty: ValueType::Uuid,
                },
                Field {
                    name: String::from("a_string"),
                    ty: ValueType::String,
                },
                Field {
                    name: String::from("a_u8"),
                    ty: ValueType::U8,
                },
                Field {
                    name: String::from("a_u16"),
                    ty: ValueType::U16,
                },
                Field {
                    name: String::from("a_u32"),
                    ty: ValueType::U32,
                },
                Field {
                    name: String::from("a_u64"),
                    ty: ValueType::U64,
                },
                Field {
                    name: String::from("a_i8"),
                    ty: ValueType::I8,
                },
                Field {
                    name: String::from("a_i16"),
                    ty: ValueType::I16,
                },
                Field {
                    name: String::from("a_i32"),
                    ty: ValueType::I32,
                },
                Field {
                    name: String::from("a_i64"),
                    ty: ValueType::I64,
                },
                Field {
                    name: String::from("a_f32"),
                    ty: ValueType::F32,
                },
                Field {
                    name: String::from("a_f64"),
                    ty: ValueType::F64,
                },
                Field {
                    name: String::from("a_option"),
                    ty: ValueType::Option(Box::new(ValueType::U64)),
                },
                Field {
                    name: String::from("a_list"),
                    ty: ValueType::List(Box::new(ValueType::U64)),
                },
                Field {
                    name: String::from("a_nested"),
                    ty: ValueType::Attributes(String::from("Nested")),
                },
                Field {
                    name: String::from("a_custom"),
                    ty: ValueType::CustomAttributes,
                },
                Field {
                    name: String::from("a_entity_ref"),
                    ty: ValueType::EntityRef {
                        entity_type: EntityRefTarget::Any,
                        role_type: EntityRefKind::Plain,
                    },
                },
            ],
            rust_path: utils::rust_path!("AllValues"),
        }
    );
    assert_eq!(
        AllValues::model_value_type(),
        ValueType::attributes("AllValues")
    );
}
