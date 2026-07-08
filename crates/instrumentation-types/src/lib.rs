// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Types referenced by generated instrumentation libraries.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Reference from one entity instance to another by id, optionally carrying
/// payload data `T`.
///
/// This is the value backing schema `EntityRef` fields in generated
/// instrumentation libraries.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct EntityRef<T = ()> {
    /// Id of the referenced entity instance.
    pub target: Uuid,
    /// Payload carried alongside the reference.
    pub data: T,
}
