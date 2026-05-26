// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::identifier::Identifier;

/// Types of attribute values.
#[derive(Clone, Debug, PartialEq)]
pub enum ValueType {
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
    Option(Box<ValueType>),
    List(Box<ValueType>),
    /// A reference to an attributes set.
    Attributes(Identifier),
    /// A set of attributes determined by the instrumentation client at run-time.
    CustomAttributes,
}
