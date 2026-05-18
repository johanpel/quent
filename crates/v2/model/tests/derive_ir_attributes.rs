use quent_v2_model_ir::{
    attributes::{Attributes, EntityRefKind, EntityRefTarget, Field, ModelAttributes},
    value_type::{ModelValueType, ValueType},
};

mod source;
mod utils;

use source::attributes::*;

// Unit structs
#[test]
fn unit() {
    assert_eq!(
        Unit::model_attributes(),
        Attributes::new(
            "Unit",
            vec![],
            utils::rust_path!("source::attributes::Unit")
        )
    );
    assert_eq!(Unit::model_value_type(), ValueType::attributes("Unit"));

    assert_eq!(
        UnitBraces::model_attributes(),
        Attributes::new(
            "UnitBraces",
            vec![],
            utils::rust_path!("source::attributes::UnitBraces"),
        )
    );
    assert_eq!(
        UnitBraces::model_value_type(),
        ValueType::attributes("UnitBraces")
    );
}

// Single field structs
#[test]
#[allow(unused)]
fn single() {
    assert_eq!(
        OnePrim::model_attributes(),
        Attributes::new(
            "OnePrim",
            vec![Field {
                name: String::from("a"),
                ty: ValueType::U8,
            }],
            utils::rust_path!("source::attributes::OnePrim"),
        )
    );
    assert_eq!(
        OnePrim::model_value_type(),
        ValueType::attributes("OnePrim")
    );

    assert_eq!(
        OneNested::model_attributes(),
        Attributes::new(
            "OneNested",
            vec![Field {
                name: String::from("a"),
                ty: ValueType::Attributes(String::from("OnePrim")),
            }],
            utils::rust_path!("source::attributes::OneNested"),
        )
    );
    assert_eq!(
        OneNested::model_value_type(),
        ValueType::attributes("OneNested")
    );

    assert_eq!(
        OneList::model_attributes(),
        Attributes::new(
            "OneList",
            vec![Field {
                name: String::from("a"),
                ty: ValueType::List(Box::new(ValueType::U8)),
            }],
            utils::rust_path!("source::attributes::OneList"),
        )
    );
    assert_eq!(
        OneList::model_value_type(),
        ValueType::attributes("OneList")
    );

    assert_eq!(
        OneListNested::model_attributes(),
        Attributes::new(
            "OneListNested",
            vec![Field {
                name: String::from("a"),
                ty: ValueType::List(Box::new(ValueType::Attributes(String::from("OnePrim")))),
            }],
            utils::rust_path!("source::attributes::OneListNested"),
        )
    );
    assert_eq!(
        OneListNested::model_value_type(),
        ValueType::attributes("OneListNested")
    );

    assert_eq!(
        OneListListPrim::model_attributes(),
        Attributes::new(
            "OneListListPrim",
            vec![Field {
                name: String::from("a"),
                ty: ValueType::List(Box::new(ValueType::List(Box::new(ValueType::U8)))),
            }],
            utils::rust_path!("source::attributes::OneListListPrim"),
        )
    );
    assert_eq!(
        OneListListPrim::model_value_type(),
        ValueType::attributes("OneListListPrim")
    );

    assert_eq!(
        OneListListNested::model_attributes(),
        Attributes::new(
            "OneListListNested",
            vec![Field {
                name: String::from("a"),
                ty: ValueType::List(Box::new(ValueType::List(Box::new(ValueType::Attributes(
                    String::from("OnePrim"),
                ))))),
            }],
            utils::rust_path!("source::attributes::OneListListNested"),
        )
    );
    assert_eq!(
        OneListListNested::model_value_type(),
        ValueType::attributes("OneListListNested")
    );
}

// Multiple field structs
#[test]
#[allow(unused)]
fn multi() {
    assert_eq!(
        MultiPrim::model_attributes(),
        Attributes::new(
            "MultiPrim",
            vec![
                Field {
                    name: String::from("a"),
                    ty: ValueType::U8,
                },
                Field {
                    name: String::from("b"),
                    ty: ValueType::String,
                },
            ],
            utils::rust_path!("source::attributes::MultiPrim"),
        )
    );
    assert_eq!(
        MultiPrim::model_value_type(),
        ValueType::attributes("MultiPrim")
    );

    assert_eq!(
        MultiNested::model_attributes(),
        Attributes::new(
            "MultiNested",
            vec![
                Field {
                    name: String::from("a"),
                    ty: ValueType::U8,
                },
                Field {
                    name: String::from("b"),
                    ty: ValueType::Attributes(String::from("MultiPrim")),
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
            utils::rust_path!("source::attributes::MultiNested"),
        )
    );
    assert_eq!(
        MultiNested::model_value_type(),
        ValueType::attributes("MultiNested")
    );

    assert_eq!(
        MultiOption::model_attributes(),
        Attributes::new(
            "MultiOption",
            vec![
                Field {
                    name: String::from("a"),
                    ty: ValueType::U8,
                },
                Field {
                    name: String::from("b"),
                    ty: ValueType::Option(Box::new(ValueType::String)),
                },
            ],
            utils::rust_path!("source::attributes::MultiOption"),
        )
    );
    assert_eq!(
        MultiOption::model_value_type(),
        ValueType::attributes("MultiOption")
    );
}

// All value types
#[test]
#[allow(unused)]
fn all_value_types() {
    assert_eq!(
        AllTypes::model_attributes(),
        Attributes::new(
            "AllTypes",
            vec![
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
                    ty: ValueType::Attributes(String::from("MultiNested")),
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
            utils::rust_path!("source::attributes::AllTypes"),
        )
    );
    assert_eq!(
        AllTypes::model_value_type(),
        ValueType::attributes("AllTypes")
    );
}
