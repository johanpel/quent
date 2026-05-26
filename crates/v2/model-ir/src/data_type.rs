// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::identifier::Identifier;

/// Types of data values.
#[derive(Clone, Debug, PartialEq)]
pub enum DataType {
    Bool,
    Uuid,
    String,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Option(Box<DataType>),
    List(Box<DataType>),
    /// A reference to a record.
    Record(Identifier),
    /// A record whose fields are determined by the instrumentation client at run-time.
    DynamicRecord,
}
