// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use uuid::Uuid;

use crate::entity_ref::{EntityRef, Plain};

/// Trait to mark a type is considered an entity.
pub trait Entity {
    #[cfg(feature = "ir")]
    fn ir() -> quent_v2_model_ir::entity::Entity;
    #[cfg(feature = "ir")]
    fn ir_ref_target() -> quent_v2_model_ir::event::EntityRefTarget;
}

/// Trait for handles to run-time instantiated entities.
pub trait EntityHandle {
    type EntityType: Entity;

    /// Return the universally unique identifier of the entity.
    fn id(&self) -> Uuid;

    /// Return a [`Plain`] reference to the entity.
    fn entity_ref(&self) -> EntityRef<Plain, Self::EntityType> {
        EntityRef::new(self.id(), Plain)
    }
}

// Mock error type, todo
#[derive(Debug)]
pub struct ObserverError;
impl std::fmt::Display for ObserverError {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
impl std::error::Error for ObserverError {}
