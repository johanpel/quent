// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::schema::{annotations::Annotations, data_type::DataType, identifier::Identifier};

/// A named, typed field of a [`crate::record::Record`] or
/// [`crate::event::Event`].
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Field {
    /// The name of the field.
    name: Identifier,
    /// The type of the field.
    ty: DataType,
    /// Annotations of this field.
    annotations: Annotations,
}

impl Field {
    /// The name of the field.
    pub fn name(&self) -> &Identifier {
        &self.name
    }

    /// The type of the field.
    pub fn ty(&self) -> &DataType {
        &self.ty
    }

    /// The annotations of this field.
    pub fn annotations(&self) -> &Annotations {
        &self.annotations
    }
}
