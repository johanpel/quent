// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::marker::PhantomData;

use quent_v2_model_ir::value_type::{
    ModelEntityRefKind, ModelEntityRefTarget, ModelValueType, ValueType,
};
use uuid::Uuid;

use crate::entity_ref::EntityRef;

/// Trait to mark a type satisfies the requirements to be considered an entity.
///
/// The requirements are that it has a UUID and emits at least one event.
pub trait EntityDeclaration {}

/// Trait for handles to run-time instantiated entities.
pub trait EntityHandle {
    type DeclarationType: EntityDeclaration;

    fn id(&self) -> Uuid;

    fn entity_ref(&self) -> EntityRef<Self::DeclarationType> {
        EntityRef {
            _entity: PhantomData,
            _role: PhantomData,
            id: self.id(),
        }
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

impl<E, R> ModelValueType for EntityRef<E, R>
where
    E: EntityDeclaration + ModelEntityRefTarget,
    R: ModelEntityRefKind,
{
    fn model_value_type() -> ValueType {
        ValueType::EntityRef {
            entity_type: E::model_entity_ref_target(),
            role_type: R::model_entity_ref_kind(),
        }
    }
}
