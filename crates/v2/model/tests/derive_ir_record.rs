// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model::{data_type::DataType, record::Record};
use quent_v2_model_ir::{
    data_type::DataType as IrDataType,
    record::{Field, Record as IrRecord},
};

mod source;
mod utils;

use source::records::*;

use crate::utils::ident;

#[test]
fn unit() {
    assert_eq!(
        <Unit as Record>::ir(),
        IrRecord::new(
            ident("Unit"),
            vec![],
            utils::rust_path!("source::records::Unit")
        )
    );
    assert_eq!(<Unit as DataType>::ir(), IrDataType::Record(ident("Unit")));
}

#[test]
fn unit_braces() {
    assert_eq!(
        <UnitBraces as Record>::ir(),
        IrRecord::new(
            ident("UnitBraces"),
            vec![],
            utils::rust_path!("source::records::UnitBraces"),
        )
    );
    assert_eq!(
        <UnitBraces as DataType>::ir(),
        IrDataType::Record(ident("UnitBraces"))
    );
}

#[test]
fn one_prim() {
    assert_eq!(
        <OnePrim as Record>::ir(),
        IrRecord::new(
            ident("OnePrim"),
            vec![Field {
                name: ident("a"),
                ty: IrDataType::U8,
            }],
            utils::rust_path!("source::records::OnePrim"),
        )
    );
    assert_eq!(
        <OnePrim as DataType>::ir(),
        IrDataType::Record(ident("OnePrim"))
    );
}

#[test]
fn one_nested() {
    assert_eq!(
        <OneNested as Record>::ir(),
        IrRecord::new(
            ident("OneNested"),
            vec![Field {
                name: ident("a"),
                ty: IrDataType::Record(ident("OnePrim")),
            }],
            utils::rust_path!("source::records::OneNested"),
        )
    );
    assert_eq!(
        <OneNested as DataType>::ir(),
        IrDataType::Record(ident("OneNested"))
    );
}

#[test]
fn one_list() {
    assert_eq!(
        <OneList as Record>::ir(),
        IrRecord::new(
            ident("OneList"),
            vec![Field {
                name: ident("a"),
                ty: IrDataType::List(Box::new(IrDataType::U8)),
            }],
            utils::rust_path!("source::records::OneList"),
        )
    );
    assert_eq!(
        <OneList as DataType>::ir(),
        IrDataType::Record(ident("OneList"))
    );
}

#[test]
fn one_list_nested() {
    assert_eq!(
        <OneListNested as Record>::ir(),
        IrRecord::new(
            ident("OneListNested"),
            vec![Field {
                name: ident("a"),
                ty: IrDataType::List(Box::new(IrDataType::Record(ident("OnePrim")))),
            }],
            utils::rust_path!("source::records::OneListNested"),
        )
    );
    assert_eq!(
        <OneListNested as DataType>::ir(),
        IrDataType::Record(ident("OneListNested"))
    );
}

#[test]
fn one_list_list_prim() {
    assert_eq!(
        <OneListListPrim as Record>::ir(),
        IrRecord::new(
            ident("OneListListPrim"),
            vec![Field {
                name: ident("a"),
                ty: IrDataType::List(Box::new(IrDataType::List(Box::new(IrDataType::U8)))),
            }],
            utils::rust_path!("source::records::OneListListPrim"),
        )
    );
    assert_eq!(
        <OneListListPrim as DataType>::ir(),
        IrDataType::Record(ident("OneListListPrim"))
    );
}

#[test]
fn one_list_list_nested() {
    assert_eq!(
        <OneListListNested as Record>::ir(),
        IrRecord::new(
            ident("OneListListNested"),
            vec![Field {
                name: ident("a"),
                ty: IrDataType::List(Box::new(IrDataType::List(Box::new(IrDataType::Record(
                    ident("OnePrim")
                )))))
            }],
            utils::rust_path!("source::records::OneListListNested"),
        )
    );
    assert_eq!(
        <OneListListNested as DataType>::ir(),
        IrDataType::Record(ident("OneListListNested"))
    );
}

#[test]
fn multi_prim() {
    assert_eq!(
        <MultiPrim as Record>::ir(),
        IrRecord::new(
            ident("MultiPrim"),
            vec![
                Field {
                    name: ident("a"),
                    ty: IrDataType::U8,
                },
                Field {
                    name: ident("b"),
                    ty: IrDataType::String,
                },
            ],
            utils::rust_path!("source::records::MultiPrim"),
        )
    );
    assert_eq!(
        <MultiPrim as DataType>::ir(),
        IrDataType::Record(ident("MultiPrim"))
    );
}

#[test]
fn multi_nested() {
    assert_eq!(
        <MultiNested as Record>::ir(),
        IrRecord::new(
            ident("MultiNested"),
            vec![
                Field {
                    name: ident("a"),
                    ty: IrDataType::U8,
                },
                Field {
                    name: ident("b"),
                    ty: IrDataType::Record(ident("MultiPrim")),
                },
                Field {
                    name: ident("c"),
                    ty: IrDataType::List(Box::new(IrDataType::U16)),
                },
                Field {
                    name: ident("d"),
                    ty: IrDataType::String,
                },
            ],
            utils::rust_path!("source::records::MultiNested"),
        )
    );
    assert_eq!(
        <MultiNested as DataType>::ir(),
        IrDataType::Record(ident("MultiNested"))
    );
}

#[test]
fn multi_option() {
    assert_eq!(
        <MultiOption as Record>::ir(),
        IrRecord::new(
            ident("MultiOption"),
            vec![
                Field {
                    name: ident("a"),
                    ty: IrDataType::U8,
                },
                Field {
                    name: ident("b"),
                    ty: IrDataType::Option(Box::new(IrDataType::String)),
                },
            ],
            utils::rust_path!("source::records::MultiOption"),
        )
    );
    assert_eq!(
        <MultiOption as DataType>::ir(),
        IrDataType::Record(ident("MultiOption"))
    );
}

#[test]
fn all_types() {
    assert_eq!(
        <AllTypes as Record>::ir(),
        IrRecord::new(
            ident("AllTypes"),
            vec![
                Field {
                    name: ident("a_bool"),
                    ty: IrDataType::Bool,
                },
                Field {
                    name: ident("a_uuid"),
                    ty: IrDataType::Uuid,
                },
                Field {
                    name: ident("a_string"),
                    ty: IrDataType::String,
                },
                Field {
                    name: ident("a_u8"),
                    ty: IrDataType::U8,
                },
                Field {
                    name: ident("a_u16"),
                    ty: IrDataType::U16,
                },
                Field {
                    name: ident("a_u32"),
                    ty: IrDataType::U32,
                },
                Field {
                    name: ident("a_u64"),
                    ty: IrDataType::U64,
                },
                Field {
                    name: ident("a_i8"),
                    ty: IrDataType::I8,
                },
                Field {
                    name: ident("a_i16"),
                    ty: IrDataType::I16,
                },
                Field {
                    name: ident("a_i32"),
                    ty: IrDataType::I32,
                },
                Field {
                    name: ident("a_i64"),
                    ty: IrDataType::I64,
                },
                Field {
                    name: ident("a_f32"),
                    ty: IrDataType::F32,
                },
                Field {
                    name: ident("a_f64"),
                    ty: IrDataType::F64,
                },
                Field {
                    name: ident("a_option"),
                    ty: IrDataType::Option(Box::new(IrDataType::U64)),
                },
                Field {
                    name: ident("a_list"),
                    ty: IrDataType::List(Box::new(IrDataType::U64)),
                },
                Field {
                    name: ident("a_nested"),
                    ty: IrDataType::Record(ident("MultiNested")),
                },
                Field {
                    name: ident("a_custom"),
                    ty: IrDataType::DynamicRecord,
                },
            ],
            utils::rust_path!("source::records::AllTypes"),
        )
    );
    assert_eq!(
        <AllTypes as DataType>::ir(),
        IrDataType::Record(ident("AllTypes"))
    );
}
