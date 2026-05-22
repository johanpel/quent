// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model_ir::value_type::{ModelValueType, ValueType};
use uuid::Uuid;

use crate::entity_ref::{EntityRef, Plain};

/// Trait to mark a type satisfies the requirements to be considered an entity.
///
/// The requirements are that it has a UUID and emits at least one event.
pub trait Entity {}

/// Trait for handles to run-time instantiated entities.
pub trait EntityHandle {
    type EntityType: Entity;

    fn id(&self) -> Uuid;

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
