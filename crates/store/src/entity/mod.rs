// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Typed access to stored entities.

use std::num::NonZeroUsize;

use quent_events::EntityMarker;
use quent_time::TimeUnixNanoSec;
use uuid::Uuid;

pub mod history;
pub mod memory;

/// Identifies an entity without owning its events.
pub trait EntityHandle {
    /// Entity marker addressed by this handle.
    type Entity: EntityMarker;

    /// Returns the entity UUID.
    fn id(&self) -> Uuid;

    /// Returns the entity type name.
    fn type_name(&self) -> &str;

    /// Returns the earliest recorded timestamp, or `None` if this handle has no entity in `store`.
    fn earliest_timestamp<S>(&self, store: &S) -> Result<Option<TimeUnixNanoSec>, S::Error>
    where
        Self: Sized,
        S: EntityStore<Self::Entity, Handle = Self>,
    {
        store.earliest_timestamp(self)
    }

    /// Returns the latest recorded timestamp, or `None` if this handle has no entity in `store`.
    fn latest_timestamp<S>(&self, store: &S) -> Result<Option<TimeUnixNanoSec>, S::Error>
    where
        Self: Sized,
        S: EntityStore<Self::Entity, Handle = Self>,
    {
        store.latest_timestamp(self)
    }

    /// Returns the event count, or `None` if this handle has no entity in `store`.
    fn num_events<S>(&self, store: &S) -> Result<Option<NonZeroUsize>, S::Error>
    where
        Self: Sized,
        S: EntityStore<Self::Entity, Handle = Self>,
    {
        store.num_events(self)
    }
}

/// Provides typed access to stored entities of marker type `E`.
pub trait EntityStore<E: EntityMarker> {
    /// Error returned when entity access fails.
    type Error;
    /// Owned handle returned by this store.
    type Handle: EntityHandle<Entity = E>;

    /// Iterates over entity handles in UUID order.
    fn entities(&self) -> Result<impl Iterator<Item = Self::Handle>, Self::Error>;

    /// Finds an entity handle by UUID.
    fn entity(&self, entity_id: Uuid) -> Result<Option<Self::Handle>, Self::Error>;

    /// Returns the earliest recorded timestamp, or `None` if the handle has no entity in this store.
    fn earliest_timestamp(
        &self,
        handle: &Self::Handle,
    ) -> Result<Option<TimeUnixNanoSec>, Self::Error>;

    /// Returns the latest recorded timestamp, or `None` if the handle has no entity in this store.
    fn latest_timestamp(
        &self,
        handle: &Self::Handle,
    ) -> Result<Option<TimeUnixNanoSec>, Self::Error>;

    /// Returns the event count, or `None` if the handle has no entity in this store.
    fn num_events(&self, handle: &Self::Handle) -> Result<Option<NonZeroUsize>, Self::Error>;
}
