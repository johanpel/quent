// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model_ir::{
    attributes::{Attributes, Field, ModelAttributes},
    value_type::{ModelValueType, ValueType},
};

mod source;
mod utils;

use source::attributes::*;

use crate::utils::ident;

// Unit structs
#[test]
fn unit() {
    assert_eq!(
        Unit::model_attributes(),
        Attributes::new(
            ident("Unit"),
            vec![],
            utils::rust_path!("source::attributes::Unit")
        )
    );
    assert_eq!(
        Unit::model_value_type(),
        ValueType::Attributes(ident("Unit"))
    );

    assert_eq!(
        UnitBraces::model_attributes(),
        Attributes::new(
            ident("UnitBraces"),
            vec![],
            utils::rust_path!("source::attributes::UnitBraces"),
        )
    );
    assert_eq!(
        UnitBraces::model_value_type(),
        ValueType::Attributes(ident("UnitBraces"))
    );
}

// Single field structs
#[test]
#[allow(unused)]
fn single() {
    assert_eq!(
        OnePrim::model_attributes(),
        Attributes::new(
            ident("OnePrim"),
            vec![Field {
                name: ident("a"),
                ty: ValueType::U8,
            }],
            utils::rust_path!("source::attributes::OnePrim"),
        )
    );
    assert_eq!(
        OnePrim::model_value_type(),
        ValueType::Attributes(ident("OnePrim"))
    );

    assert_eq!(
        OneNested::model_attributes(),
        Attributes::new(
            ident("OneNested"),
            vec![Field {
                name: ident("a"),
                ty: ValueType::Attributes(ident("OnePrim")),
            }],
            utils::rust_path!("source::attributes::OneNested"),
        )
    );
    assert_eq!(
        OneNested::model_value_type(),
        ValueType::Attributes(ident("OneNested"))
    );

    assert_eq!(
        OneList::model_attributes(),
        Attributes::new(
            ident("OneList"),
            vec![Field {
                name: ident("a"),
                ty: ValueType::List(Box::new(ValueType::U8)),
            }],
            utils::rust_path!("source::attributes::OneList"),
        )
    );
    assert_eq!(
        OneList::model_value_type(),
        ValueType::Attributes(ident("OneList"))
    );

    assert_eq!(
        OneListNested::model_attributes(),
        Attributes::new(
            ident("OneListNested"),
            vec![Field {
                name: ident("a"),
                ty: ValueType::List(Box::new(ValueType::Attributes(ident("OnePrim")))),
            }],
            utils::rust_path!("source::attributes::OneListNested"),
        )
    );
    assert_eq!(
        OneListNested::model_value_type(),
        ValueType::Attributes(ident("OneListNested"))
    );

    assert_eq!(
        OneListListPrim::model_attributes(),
        Attributes::new(
            ident("OneListListPrim"),
            vec![Field {
                name: ident("a"),
                ty: ValueType::List(Box::new(ValueType::List(Box::new(ValueType::U8)))),
            }],
            utils::rust_path!("source::attributes::OneListListPrim"),
        )
    );
    assert_eq!(
        OneListListPrim::model_value_type(),
        ValueType::Attributes(ident("OneListListPrim"))
    );

    assert_eq!(
        OneListListNested::model_attributes(),
        Attributes::new(
            ident("OneListListNested"),
            vec![Field {
                name: ident("a"),
                ty: ValueType::List(Box::new(ValueType::List(Box::new(ValueType::Attributes(
                    ident("OnePrim")
                )))))
            }],
            utils::rust_path!("source::attributes::OneListListNested"),
        )
    );
    assert_eq!(
        OneListListNested::model_value_type(),
        ValueType::Attributes(ident("OneListListNested"))
    );
}

// Multiple field structs
#[test]
#[allow(unused)]
fn multi() {
    assert_eq!(
        MultiPrim::model_attributes(),
        Attributes::new(
            ident("MultiPrim"),
            vec![
                Field {
                    name: ident("a"),
                    ty: ValueType::U8,
                },
                Field {
                    name: ident("b"),
                    ty: ValueType::String,
                },
            ],
            utils::rust_path!("source::attributes::MultiPrim"),
        )
    );
    assert_eq!(
        MultiPrim::model_value_type(),
        ValueType::Attributes(ident("MultiPrim"))
    );

    assert_eq!(
        MultiNested::model_attributes(),
        Attributes::new(
            ident("MultiNested"),
            vec![
                Field {
                    name: ident("a"),
                    ty: ValueType::U8,
                },
                Field {
                    name: ident("b"),
                    ty: ValueType::Attributes(ident("MultiPrim")),
                },
                Field {
                    name: ident("c"),
                    ty: ValueType::List(Box::new(ValueType::U16)),
                },
                Field {
                    name: ident("d"),
                    ty: ValueType::String,
                },
            ],
            utils::rust_path!("source::attributes::MultiNested"),
        )
    );
    assert_eq!(
        MultiNested::model_value_type(),
        ValueType::Attributes(ident("MultiNested"))
    );

    assert_eq!(
        MultiOption::model_attributes(),
        Attributes::new(
            ident("MultiOption"),
            vec![
                Field {
                    name: ident("a"),
                    ty: ValueType::U8,
                },
                Field {
                    name: ident("b"),
                    ty: ValueType::Option(Box::new(ValueType::String)),
                },
            ],
            utils::rust_path!("source::attributes::MultiOption"),
        )
    );
    assert_eq!(
        MultiOption::model_value_type(),
        ValueType::Attributes(ident("MultiOption"))
    );
}

// All value types
#[test]
#[allow(unused)]
fn all_value_types() {
    assert_eq!(
        AllTypes::model_attributes(),
        Attributes::new(
            ident("AllTypes"),
            vec![
                Field {
                    name: ident("a_bool"),
                    ty: ValueType::Bool,
                },
                Field {
                    name: ident("a_uuid"),
                    ty: ValueType::Uuid,
                },
                Field {
                    name: ident("a_string"),
                    ty: ValueType::String,
                },
                Field {
                    name: ident("a_u8"),
                    ty: ValueType::U8,
                },
                Field {
                    name: ident("a_u16"),
                    ty: ValueType::U16,
                },
                Field {
                    name: ident("a_u32"),
                    ty: ValueType::U32,
                },
                Field {
                    name: ident("a_u64"),
                    ty: ValueType::U64,
                },
                Field {
                    name: ident("a_i8"),
                    ty: ValueType::I8,
                },
                Field {
                    name: ident("a_i16"),
                    ty: ValueType::I16,
                },
                Field {
                    name: ident("a_i32"),
                    ty: ValueType::I32,
                },
                Field {
                    name: ident("a_i64"),
                    ty: ValueType::I64,
                },
                Field {
                    name: ident("a_f32"),
                    ty: ValueType::F32,
                },
                Field {
                    name: ident("a_f64"),
                    ty: ValueType::F64,
                },
                Field {
                    name: ident("a_option"),
                    ty: ValueType::Option(Box::new(ValueType::U64)),
                },
                Field {
                    name: ident("a_list"),
                    ty: ValueType::List(Box::new(ValueType::U64)),
                },
                Field {
                    name: ident("a_nested"),
                    ty: ValueType::Attributes(ident("MultiNested")),
                },
                Field {
                    name: ident("a_custom"),
                    ty: ValueType::CustomAttributes,
                },
            ],
            utils::rust_path!("source::attributes::AllTypes"),
        )
    );
    assert_eq!(
        AllTypes::model_value_type(),
        ValueType::Attributes(ident("AllTypes"))
    );
}
