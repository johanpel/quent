// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Typed access to stored entities.

use quent_events::{EntityMarker, Event};
use uuid::Uuid;

pub mod memory;

/// Identifies an entity without owning its events.
pub trait EntityHandle {
    /// Returns the entity UUID.
    fn id(&self) -> Uuid;
}

/// Provides typed access to stored entities of marker type `E`.
pub trait EntityStore<E: EntityMarker> {
    /// Error returned when entity access fails.
    type Error;
    /// Owned handle returned by this store.
    type Handle: EntityHandle;

    /// Iterates over entity handles in UUID order.
    fn entities(&self) -> Result<impl Iterator<Item = Self::Handle>, Self::Error>;

    /// Finds an entity handle by UUID.
    fn entity(&self, entity_id: Uuid) -> Result<Option<Self::Handle>, Self::Error>;

    /// Borrows the entity's events in timestamp order.
    fn events<'a>(
        &'a self,
        handle: &Self::Handle,
    ) -> Result<impl Iterator<Item = &'a Event<E::Payload>>, Self::Error>
    where
        E::Payload: 'a;
}
