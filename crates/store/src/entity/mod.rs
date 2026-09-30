// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Typed access to stored entities.

use std::num::NonZeroUsize;

use quent_events::EntityMarker;
use quent_time::TimeUnixNanoSec;
use uuid::Uuid;

use self::sequence::EventSequence;

pub mod memory;
pub mod sequence;

/// Identifies an entity without owning its events.
pub trait EntityHandle {
    /// Entity marker addressed by this handle.
    type Entity: EntityMarker;

    /// Returns the entity UUID.
    fn id(&self) -> Uuid;

    /// Returns the entity type name.
    fn type_name(&self) -> &str;
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

/// Provides borrowed, timestamp-ordered raw event sequences for an entity marker.
pub trait BorrowedEventSequenceStore<E: EntityMarker>: EntityStore<E> {
    /// Borrows the selected sequence, or returns `None` if it is absent.
    fn sequence(&self, handle: &Self::Handle) -> Result<Option<&EventSequence<E>>, Self::Error>;
}

/// Provides owned, timestamp-ordered raw event sequences for an entity marker.
pub trait OwnedEventSequenceStore<E: EntityMarker>: EntityStore<E> {
    /// Returns an owned sequence without changing the store, or `None` if it is absent.
    fn owned_sequence(
        &self,
        handle: &Self::Handle,
    ) -> Result<Option<EventSequence<E>>, Self::Error>;
}
