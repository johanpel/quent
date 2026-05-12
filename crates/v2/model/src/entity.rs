// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::marker::PhantomData;

use quent_time::TimeUnixNanoSec;
use quent_v2_model_macros::Entity;
use std::sync::atomic::AtomicU8;
use uuid::Uuid;

// Trait to mark a type satisfies the requirements to be considered an entity.
pub trait EntityDeclaration {}

// User-facing types used for modeling

// Type to tag a ref can be to any type of entity.
pub struct AnyEntity;
impl EntityDeclaration for AnyEntity {} // special case.

// Tag type for reference kinds.
pub struct RegularRef; // a regular reference to another entity without additional semantics

// A type-safe reference to another entity.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(transparent)
)]
pub struct EntityRef<E: EntityDeclaration, R = RegularRef> {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub _entity: PhantomData<E>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub _ref_kind: PhantomData<R>,
    pub id: Uuid,
}

impl<E: EntityDeclaration, R> Clone for EntityRef<E, R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<E: EntityDeclaration, R> Copy for EntityRef<E, R> {}

// Trait to allow EntityRefs of certain kinds to be type erased, such that the
// events carry a UUID for which the type needs to be resolved.
pub trait IntoErased<T> {
    fn into_erased(self) -> T;
}

// TODO: use the analysis/event crate traits/structs.

// Every entity has a unique id.
// For instrumentation:
pub trait EntityHandle {
    type DeclarationType: EntityDeclaration;

    fn id(&self) -> Uuid;

    fn entity_ref(&self) -> EntityRef<Self::DeclarationType> {
        EntityRef {
            _entity: PhantomData,
            _ref_kind: PhantomData,
            id: self.id(),
        }
    }
}

// An event has an entity id, a timestamp, and a payload
pub struct Event<T> {
    pub id: Uuid,
    pub timestamp: TimeUnixNanoSec,
    pub payload: T,
}

// Mock error type
#[derive(Debug)]
pub struct ObserverError;
impl std::fmt::Display for ObserverError {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
impl std::error::Error for ObserverError {}
