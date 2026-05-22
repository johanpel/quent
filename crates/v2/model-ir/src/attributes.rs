// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{identifier::Identifier, value_type::ValueType};

/// Trait to obtain the IR of a type representing an attribute set.
pub trait ModelAttributes {
    fn model_attributes() -> Attributes;
}

/// Definition of a field in an attribute set.
#[derive(Debug, PartialEq)]
pub struct Field {
    pub name: Identifier,
    pub ty: ValueType,
}

/// IR of a set of attributes.
// TODO(johanpel): consider naming this Record or something else
#[derive(Debug, PartialEq)]
pub struct Attributes {
    /// The name of the attributes.
    pub name: Identifier,
    /// The fields of the attributes.
    pub fields: Vec<Field>,

    /// The Rust path to the attributes.
    pub rust_path: String,
}

impl Attributes {
    pub fn new(name: Identifier, fields: Vec<Field>, rust_path: impl Into<String>) -> Self {
        Self {
            name,
            fields,
            rust_path: rust_path.into(),
        }
    }
}
