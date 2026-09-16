// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Runtime analysis interfaces for the schema Reference Tree.

use uuid::Uuid;

use crate::Entity;

pub mod collection;
pub mod tree;

/// Trait for entities participating in the runtime Reference Tree.
pub trait RefTreeEntity: Entity {
    /// Return the parent entity ID, or `None` for the root entity.
    fn parent_id(&self) -> Option<Uuid>;
}
