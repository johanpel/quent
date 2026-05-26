// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model::attributes::{Attributes, ValueType};
use quent_v2_model_ir::{
    attributes::{Attributes as IrAttr, Field},
    value_type::ValueType as IrValueType,
};

mod source;
mod utils;

use source::attributes::*;

use crate::utils::ident;

#[test]
fn unit() {
    assert_eq!(
        <Unit as Attributes>::ir(),
        IrAttr::new(
            ident("Unit"),
            vec![],
            utils::rust_path!("source::attributes::Unit")
        )
    );
    assert_eq!(
        <Unit as ValueType>::ir(),
        IrValueType::Attributes(ident("Unit"))
    );
}

#[test]
fn unit_braces() {
    assert_eq!(
        <UnitBraces as Attributes>::ir(),
        IrAttr::new(
            ident("UnitBraces"),
            vec![],
            utils::rust_path!("source::attributes::UnitBraces"),
        )
    );
    assert_eq!(
        <UnitBraces as ValueType>::ir(),
        IrValueType::Attributes(ident("UnitBraces"))
    );
}

#[test]
fn one_prim() {
    assert_eq!(
        <OnePrim as Attributes>::ir(),
        IrAttr::new(
            ident("OnePrim"),
            vec![Field {
                name: ident("a"),
                ty: IrValueType::U8,
            }],
            utils::rust_path!("source::attributes::OnePrim"),
        )
    );
    assert_eq!(
        <OnePrim as ValueType>::ir(),
        IrValueType::Attributes(ident("OnePrim"))
    );
}

#[test]
fn one_nested() {
    assert_eq!(
        <OneNested as Attributes>::ir(),
        IrAttr::new(
            ident("OneNested"),
            vec![Field {
                name: ident("a"),
                ty: IrValueType::Attributes(ident("OnePrim")),
            }],
            utils::rust_path!("source::attributes::OneNested"),
        )
    );
    assert_eq!(
        <OneNested as ValueType>::ir(),
        IrValueType::Attributes(ident("OneNested"))
    );
}

#[test]
fn one_list() {
    assert_eq!(
        <OneList as Attributes>::ir(),
        IrAttr::new(
            ident("OneList"),
            vec![Field {
                name: ident("a"),
                ty: IrValueType::List(Box::new(IrValueType::U8)),
            }],
            utils::rust_path!("source::attributes::OneList"),
        )
    );
    assert_eq!(
        <OneList as ValueType>::ir(),
        IrValueType::Attributes(ident("OneList"))
    );
}

#[test]
fn one_list_nested() {
    assert_eq!(
        <OneListNested as Attributes>::ir(),
        IrAttr::new(
            ident("OneListNested"),
            vec![Field {
                name: ident("a"),
                ty: IrValueType::List(Box::new(IrValueType::Attributes(ident("OnePrim")))),
            }],
            utils::rust_path!("source::attributes::OneListNested"),
        )
    );
    assert_eq!(
        <OneListNested as ValueType>::ir(),
        IrValueType::Attributes(ident("OneListNested"))
    );
}

#[test]
fn one_list_list_prim() {
    assert_eq!(
        <OneListListPrim as Attributes>::ir(),
        IrAttr::new(
            ident("OneListListPrim"),
            vec![Field {
                name: ident("a"),
                ty: IrValueType::List(Box::new(IrValueType::List(Box::new(IrValueType::U8)))),
            }],
            utils::rust_path!("source::attributes::OneListListPrim"),
        )
    );
    assert_eq!(
        <OneListListPrim as ValueType>::ir(),
        IrValueType::Attributes(ident("OneListListPrim"))
    );
}

#[test]
fn one_list_list_nested() {
    assert_eq!(
        <OneListListNested as Attributes>::ir(),
        IrAttr::new(
            ident("OneListListNested"),
            vec![Field {
                name: ident("a"),
                ty: IrValueType::List(Box::new(IrValueType::List(Box::new(
                    IrValueType::Attributes(ident("OnePrim"))
                ))))
            }],
            utils::rust_path!("source::attributes::OneListListNested"),
        )
    );
    assert_eq!(
        <OneListListNested as ValueType>::ir(),
        IrValueType::Attributes(ident("OneListListNested"))
    );
}

#[test]
fn multi_prim() {
    assert_eq!(
        <MultiPrim as Attributes>::ir(),
        IrAttr::new(
            ident("MultiPrim"),
            vec![
                Field {
                    name: ident("a"),
                    ty: IrValueType::U8,
                },
                Field {
                    name: ident("b"),
                    ty: IrValueType::String,
                },
            ],
            utils::rust_path!("source::attributes::MultiPrim"),
        )
    );
    assert_eq!(
        <MultiPrim as ValueType>::ir(),
        IrValueType::Attributes(ident("MultiPrim"))
    );
}

#[test]
fn multi_nested() {
    assert_eq!(
        <MultiNested as Attributes>::ir(),
        IrAttr::new(
            ident("MultiNested"),
            vec![
                Field {
                    name: ident("a"),
                    ty: IrValueType::U8,
                },
                Field {
                    name: ident("b"),
                    ty: IrValueType::Attributes(ident("MultiPrim")),
                },
                Field {
                    name: ident("c"),
                    ty: IrValueType::List(Box::new(IrValueType::U16)),
                },
                Field {
                    name: ident("d"),
                    ty: IrValueType::String,
                },
            ],
            utils::rust_path!("source::attributes::MultiNested"),
        )
    );
    assert_eq!(
        <MultiNested as ValueType>::ir(),
        IrValueType::Attributes(ident("MultiNested"))
    );
}

#[test]
fn multi_option() {
    assert_eq!(
        <MultiOption as Attributes>::ir(),
        IrAttr::new(
            ident("MultiOption"),
            vec![
                Field {
                    name: ident("a"),
                    ty: IrValueType::U8,
                },
                Field {
                    name: ident("b"),
                    ty: IrValueType::Option(Box::new(IrValueType::String)),
                },
            ],
            utils::rust_path!("source::attributes::MultiOption"),
        )
    );
    assert_eq!(
        <MultiOption as ValueType>::ir(),
        IrValueType::Attributes(ident("MultiOption"))
    );
}

#[test]
fn all_types() {
    assert_eq!(
        <AllTypes as Attributes>::ir(),
        IrAttr::new(
            ident("AllTypes"),
            vec![
                Field {
                    name: ident("a_bool"),
                    ty: IrValueType::Bool,
                },
                Field {
                    name: ident("a_uuid"),
                    ty: IrValueType::Uuid,
                },
                Field {
                    name: ident("a_string"),
                    ty: IrValueType::String,
                },
                Field {
                    name: ident("a_u8"),
                    ty: IrValueType::U8,
                },
                Field {
                    name: ident("a_u16"),
                    ty: IrValueType::U16,
                },
                Field {
                    name: ident("a_u32"),
                    ty: IrValueType::U32,
                },
                Field {
                    name: ident("a_u64"),
                    ty: IrValueType::U64,
                },
                Field {
                    name: ident("a_i8"),
                    ty: IrValueType::I8,
                },
                Field {
                    name: ident("a_i16"),
                    ty: IrValueType::I16,
                },
                Field {
                    name: ident("a_i32"),
                    ty: IrValueType::I32,
                },
                Field {
                    name: ident("a_i64"),
                    ty: IrValueType::I64,
                },
                Field {
                    name: ident("a_f32"),
                    ty: IrValueType::F32,
                },
                Field {
                    name: ident("a_f64"),
                    ty: IrValueType::F64,
                },
                Field {
                    name: ident("a_option"),
                    ty: IrValueType::Option(Box::new(IrValueType::U64)),
                },
                Field {
                    name: ident("a_list"),
                    ty: IrValueType::List(Box::new(IrValueType::U64)),
                },
                Field {
                    name: ident("a_nested"),
                    ty: IrValueType::Attributes(ident("MultiNested")),
                },
                Field {
                    name: ident("a_custom"),
                    ty: IrValueType::CustomAttributes,
                },
            ],
            utils::rust_path!("source::attributes::AllTypes"),
        )
    );
    assert_eq!(
        <AllTypes as ValueType>::ir(),
        IrValueType::Attributes(ident("AllTypes"))
    );
}
