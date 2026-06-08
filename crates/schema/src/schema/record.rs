// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::schema::{Map, annotations::Annotations, field::Field, identifier::Identifier};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Record {
    /// The name of the record.
    pub name: Identifier,
    /// The fields of the record.
    pub fields: Map<Identifier, Field>,
    /// Annotations of this record.
    pub annotations: Annotations,
}
