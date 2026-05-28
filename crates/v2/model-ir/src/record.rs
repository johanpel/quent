// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{convention::Convention, data_type::DataType, identifier::Identifier};

/// Definition of a field in a record.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Field {
    /// The name of this field.
    pub name: Identifier,
    /// Potential documentation that can be added in code generation.
    pub docs: Option<String>,
    /// The type of this field.
    pub ty: DataType,
    /// Convention-specific metadata attached to this field.
    pub conventions: Vec<Convention>,
}

/// IR of a record.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Record {
    /// The name of the record.
    pub name: Identifier,
    /// Free-form documentation captured from the source declaration's `#[doc = "..."]`
    /// attributes (i.e. `///` lines). Multi-line docstrings are concatenated with
    /// `\n` separators. Surfaced by code generators (e.g. cxx, pyo3, schema export)
    /// as native target-language documentation.
    pub docs: Option<String>,
    /// The fields of the record.
    pub fields: Vec<Field>,
    /// Convention-specific metadata attached to this record.
    pub conventions: Vec<Convention>,
}
