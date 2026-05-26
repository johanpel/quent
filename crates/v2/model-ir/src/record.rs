// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{data_type::DataType, identifier::Identifier};

/// Definition of a field in a record.
#[derive(Debug, PartialEq)]
pub struct Field {
    pub name: Identifier,
    pub ty: DataType,
}

/// IR of a record.
#[derive(Debug, PartialEq)]
pub struct Record {
    /// The name of the record.
    pub name: Identifier,
    /// The fields of the record.
    pub fields: Vec<Field>,

    /// The Rust path to the record.
    pub rust_path: String,
}

impl Record {
    pub fn new(name: Identifier, fields: Vec<Field>, rust_path: impl Into<String>) -> Self {
        Self {
            name,
            fields,
            rust_path: rust_path.into(),
        }
    }
}
