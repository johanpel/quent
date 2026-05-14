// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::marker::PhantomData;

use uuid::Uuid;

/// Trait to mark a type satisfies the requirements to be considered an entity.
///
/// The requirements are that it has a UUID and emits at least one event.
pub trait EntityDeclaration {}

/// Type to tag an [`EntityRef`] can be to any type of entity, to be determined
/// at run-time.
pub struct AnyEntity;
// Special case:
impl EntityDeclaration for AnyEntity {}

/// Type to tag an [`EntityRef`] of being of no particular meaning.
pub struct PlainRef;

/// A reference to another entity.
///
/// `EntityType` defines the entity type to which this reference refers. This
/// can also be type-erased by using `EntityType = AnyEntity`, such that at
/// run-time, any entity's handle can be provided.
///
/// `RoleType` defines the role type of the reference. By default, it is a
/// regular reference (`RegularRef`) which holds no particular meaning. But, for
/// example, it can be set to [`super::resource_group::RgParentRef`] to specify
/// it carries a parent relation of a child resource group entity in the
/// resource hierarchy. The latter is a requirement of the ResourceGroup
/// qualification. One event MUST carry this field for an entity to qualify as a
/// ResourceGroup.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(transparent)
)]
pub struct EntityRef<EntityType: EntityDeclaration, RoleType = PlainRef> {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub _entity: PhantomData<EntityType>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub _role: PhantomData<RoleType>,

    pub id: Uuid,
}

impl<E: EntityDeclaration, R> Clone for EntityRef<E, R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<E: EntityDeclaration, R> Copy for EntityRef<E, R> {}

/// Trait allowing specific [`EntityRef`]s to be type-erased.
pub trait IntoErased<T> {
    fn into_erased(self) -> T;
}

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
