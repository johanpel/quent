// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{convention::Convention, data_type::DataType, identifier::Identifier};

/// Definition of a field in a record.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Field {
    /// The name of this field.
    pub name: Identifier,
    /// Potential documentation.
    pub docs: Option<String>,
    /// The type of this field.
    pub ty: DataType,
    /// Convention-specific metadata attached to this field.
    pub conventions: Vec<Convention>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Record {
    /// The name of the record.
    pub name: Identifier,
    /// Potential documentation.
    pub docs: Option<String>,
    /// The fields of the record.
    pub fields: Vec<Field>,
    /// Convention-specific metadata attached to this record.
    pub conventions: Vec<Convention>,
}
