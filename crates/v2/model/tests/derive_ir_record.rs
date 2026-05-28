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

fn field(name: &str, ty: IrDataType) -> Field {
    Field {
        name: ident(name),
        docs: None,
        ty,
        conventions: Vec::new(),
    }
}

fn record(name: &str, fields: Vec<Field>) -> IrRecord {
    IrRecord {
        name: ident(name),
        docs: None,
        fields,
        conventions: Vec::new(),
    }
}

#[test]
fn unit() {
    assert_eq!(<Unit as Record>::ir(), record("Unit", vec![]));
    assert_eq!(<Unit as DataType>::ir(), IrDataType::Record(ident("Unit")));
}

#[test]
fn unit_braces() {
    assert_eq!(<UnitBraces as Record>::ir(), record("UnitBraces", vec![]));
    assert_eq!(
        <UnitBraces as DataType>::ir(),
        IrDataType::Record(ident("UnitBraces"))
    );
}

#[test]
fn one_prim() {
    assert_eq!(
        <OnePrim as Record>::ir(),
        record("OnePrim", vec![field("a", IrDataType::U8)])
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
        record(
            "OneNested",
            vec![field("a", IrDataType::Record(ident("OnePrim")))],
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
        record(
            "OneList",
            vec![field("a", IrDataType::List(Box::new(IrDataType::U8)))],
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
        record(
            "OneListNested",
            vec![field(
                "a",
                IrDataType::List(Box::new(IrDataType::Record(ident("OnePrim"))))
            )],
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
        record(
            "OneListListPrim",
            vec![field(
                "a",
                IrDataType::List(Box::new(IrDataType::List(Box::new(IrDataType::U8))))
            )],
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
        record(
            "OneListListNested",
            vec![field(
                "a",
                IrDataType::List(Box::new(IrDataType::List(Box::new(IrDataType::Record(
                    ident("OnePrim")
                )))))
            )],
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
        record(
            "MultiPrim",
            vec![field("a", IrDataType::U8), field("b", IrDataType::String),],
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
        record(
            "MultiNested",
            vec![
                field("a", IrDataType::U8),
                field("b", IrDataType::Record(ident("MultiPrim"))),
                field("c", IrDataType::List(Box::new(IrDataType::U16))),
                field("d", IrDataType::String),
            ],
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
        record(
            "MultiOption",
            vec![
                field("a", IrDataType::U8),
                field("b", IrDataType::Option(Box::new(IrDataType::String))),
            ],
        )
    );
    assert_eq!(
        <MultiOption as DataType>::ir(),
        IrDataType::Record(ident("MultiOption"))
    );
}

#[test]
fn record_docstring_flows_to_ir() {
    let ir = <DocumentedRecord as Record>::ir();
    assert_eq!(
        ir.docs.as_deref(),
        Some("A documented record.\nCarries two lines."),
    );
    assert_eq!(ir.fields[0].docs.as_deref(), Some("Documented field."));
}

#[test]
fn all_types() {
    assert_eq!(
        <AllTypes as Record>::ir(),
        record(
            "AllTypes",
            vec![
                field("a_bool", IrDataType::Bool),
                field("a_uuid", IrDataType::Uuid),
                field("a_string", IrDataType::String),
                field("a_u8", IrDataType::U8),
                field("a_u16", IrDataType::U16),
                field("a_u32", IrDataType::U32),
                field("a_u64", IrDataType::U64),
                field("a_i8", IrDataType::I8),
                field("a_i16", IrDataType::I16),
                field("a_i32", IrDataType::I32),
                field("a_i64", IrDataType::I64),
                field("a_f32", IrDataType::F32),
                field("a_f64", IrDataType::F64),
                field("a_option", IrDataType::Option(Box::new(IrDataType::U64))),
                field("a_list", IrDataType::List(Box::new(IrDataType::U64))),
                field("a_nested", IrDataType::Record(ident("MultiNested"))),
                field("a_custom", IrDataType::DynamicRecord),
            ],
        )
    );
    assert_eq!(
        <AllTypes as DataType>::ir(),
        IrDataType::Record(ident("AllTypes"))
    );
}
