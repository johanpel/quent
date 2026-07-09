// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Reference from one entity instance to another.

use uuid::Uuid;

/// Reference from one entity instance to another by id, optionally carrying
/// payload data `T`.
///
/// Placeholder backing the schema generator's `DataType::EntityRef` fields.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct EntityRef<T = ()> {
    /// Identifier of the referenced entity instance.
    pub target: Uuid,
    /// Payload carried alongside the reference.
    pub data: T,
}
