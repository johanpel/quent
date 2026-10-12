// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Typed sequences of stored entity events.

use std::{marker::PhantomData, num::NonZeroUsize};

use quent_events::{EntityMarker, Event};
use quent_time::{OrderedCollector, TimeUnixNanoSec};
use uuid::Uuid;

use crate::{EntityHandle, Id};

// TODO(johanpel): Deduplicate with
//                 quent_analyzer::entity::native::AnalyzedEntity
//                 as part of rapidsai/quent#516.
/// A non-empty, timestamp-ordered event sequence for one entity instance.
pub struct EventSequence<E: EntityMarker> {
    /// The ID of entity that emitted these events.
    id: Uuid,
    /// The ordered events.
    events: Vec<Event<E::Payload>>,
    marker: PhantomData<fn() -> E>,
}

impl<E: EntityMarker> Clone for EventSequence<E>
where
    E::Payload: Clone,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            events: self.events.clone(),
            marker: PhantomData,
        }
    }
}

impl<E: EntityMarker> EventSequence<E> {
    pub(crate) fn from_ordered_events(
        id: Uuid,
        events: OrderedCollector<Event<E::Payload>>,
    ) -> Self {
        let events = events.into_inner();
        Self {
            id,
            events,
            marker: PhantomData,
        }
    }

    /// Returns the typed entity ID.
    pub fn id(&self) -> Id<E> {
        Id::new(self.id)
    }

    /// Returns events in timestamp order.
    pub fn events(&self) -> &[Event<E::Payload>] {
        &self.events
    }

    /// Returns the number of recorded events.
    pub fn event_count(&self) -> NonZeroUsize {
        NonZeroUsize::new(self.events.len()).expect("event sequence is non-empty")
    }

    /// Returns the first event timestamp.
    pub fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        // Self can only be constructed with at least one event so unwrap is fine.
        self.events.first().unwrap().timestamp
    }

    /// Returns the last event timestamp.
    pub fn latest_timestamp(&self) -> TimeUnixNanoSec {
        // Self can only be constructed with at least one event so unwrap is fine.
        self.events.last().unwrap().timestamp
    }

    /// Consumes the sequence and returns its events in timestamp order.
    pub fn into_events(self) -> Vec<Event<E::Payload>> {
        self.events
    }
}

impl<E: EntityMarker> EntityHandle for EventSequence<E> {
    type Entity = E;

    fn id(&self) -> Id<E> {
        Id::new(self.id)
    }

    fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        Self::earliest_timestamp(self)
    }

    fn latest_timestamp(&self) -> TimeUnixNanoSec {
        Self::latest_timestamp(self)
    }

    fn num_events(&self) -> NonZeroUsize {
        self.event_count()
    }
}
