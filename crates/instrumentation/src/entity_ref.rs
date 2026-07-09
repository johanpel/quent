// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Reference from one entity instance to another.

use uuid::Uuid;

/// Reference from one entity instance to another by id, optionally carrying
/// payload data `T`.
///
/// Placeholder backing the schema generator's `DataType::EntityRef` fields.
// TODO(johanpel): flesh out ref-target semantics (see `quent.ref-target.v1`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EntityRef<T = ()> {
    /// Id of the referenced entity instance.
    pub target: Uuid,
    /// Payload carried alongside the reference.
    pub data: T,
}
