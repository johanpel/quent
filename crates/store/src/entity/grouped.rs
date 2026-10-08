// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Native entities with event-type-scoped storage.

use std::marker::PhantomData;

use quent_events::{EntityMarker, Event};
use uuid::Uuid;

use super::{DuplicateOnceEvent, EntityHandle, sequence::EventSequence};

/// Groups an entity's events by event type.
///
/// [`Default`] must produce empty groups. Successful insertions preserve event
/// metadata and insertion order within each group.
pub trait EventGroups<E: EntityMarker>: Default {
    /// Moves an event into its matching group.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateOnceEvent`] if the corresponding once-event group is
    /// occupied, leaving the existing event unchanged.
    fn push(&mut self, event: Event<E::Payload>) -> Result<(), DuplicateOnceEvent>;
}

/// Owns an entity's identity and event groups.
///
/// Conversion from [`EventSequence`] inserts events in timestamp order without
/// requiring [`Clone`] and stops at the first grouping error.
pub struct NativeEntity<E: EntityMarker, G: EventGroups<E>> {
    id: Uuid,
    event_groups: G,
    marker: PhantomData<fn() -> E>,
}

impl<E: EntityMarker, G: EventGroups<E>> NativeEntity<E, G> {
    /// Borrows the event groups.
    pub fn event_groups(&self) -> &G {
        &self.event_groups
    }
}

impl<E: EntityMarker, G: EventGroups<E>> EntityHandle for NativeEntity<E, G> {
    type Entity = E;

    fn id(&self) -> Uuid {
        self.id
    }
}

impl<E: EntityMarker, G: EventGroups<E>> TryFrom<EventSequence<E>> for NativeEntity<E, G> {
    type Error = DuplicateOnceEvent;

    fn try_from(sequence: EventSequence<E>) -> Result<Self, Self::Error> {
        let mut entity = Self {
            id: sequence.id(),
            event_groups: G::default(),
            marker: PhantomData,
        };
        for event in sequence.into_events() {
            entity.event_groups.push(event)?;
        }
        Ok(entity)
    }
}
