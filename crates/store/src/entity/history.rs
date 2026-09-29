// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Typed histories of stored entity events.

use std::marker::PhantomData;
use std::num::NonZeroUsize;

use quent_events::{EntityMarker, Event};
use quent_time::TimeUnixNanoSec;
use uuid::Uuid;

/// A non-empty, timestamp-ordered history for one entity marker.
/// Equal timestamps retain input order; timestamp order is not causal order across contexts.
pub struct EntityHistory<E: EntityMarker> {
    id: Uuid,
    events: Vec<Event<E::Payload>>,
    earliest_timestamp: TimeUnixNanoSec,
    latest_timestamp: TimeUnixNanoSec,
    marker: PhantomData<fn() -> E>,
}

impl<E: EntityMarker> EntityHistory<E> {
    pub(crate) fn from_ordered_events(id: Uuid, events: Vec<Event<E::Payload>>) -> Self {
        debug_assert!(!events.is_empty());
        debug_assert!(events.iter().all(|event| event.id == id));
        debug_assert!(
            events
                .windows(2)
                .all(|pair| pair[0].timestamp <= pair[1].timestamp)
        );
        let earliest_timestamp = events.first().unwrap().timestamp;
        let latest_timestamp = events.last().unwrap().timestamp;
        Self {
            id,
            events,
            earliest_timestamp,
            latest_timestamp,
            marker: PhantomData,
        }
    }

    /// Returns the entity UUID.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Returns events in timestamp order.
    pub fn events(&self) -> &[Event<E::Payload>] {
        &self.events
    }

    /// Returns the number of recorded events.
    pub fn event_count(&self) -> NonZeroUsize {
        NonZeroUsize::new(self.events.len()).expect("entity history is non-empty")
    }

    /// Returns the first event timestamp.
    pub fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        self.earliest_timestamp
    }

    /// Returns the last event timestamp.
    pub fn latest_timestamp(&self) -> TimeUnixNanoSec {
        self.latest_timestamp
    }

    /// Consumes the history and returns its events in timestamp order.
    pub fn into_events(self) -> Vec<Event<E::Payload>> {
        self.events
    }
}
