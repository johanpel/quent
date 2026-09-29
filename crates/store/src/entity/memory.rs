// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! In-memory entity storage.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::marker::PhantomData;
use std::num::NonZeroUsize;

use quent_events::{EntityMarker, Event, EventPayload};
use quent_time::TimeUnixNanoSec;
use uuid::Uuid;

use super::history::EntityHistory;
use super::{EntityHandle, EntityStore};

/// An owned handle for one entity marker.
pub struct Handle<E> {
    id: Uuid,
    marker: PhantomData<fn() -> E>,
}

impl<E: EntityMarker> EntityHandle for Handle<E> {
    type Entity = E;

    fn id(&self) -> Uuid {
        self.id
    }

    fn type_name(&self) -> &str {
        E::Payload::NAME
    }
}

/// Stores one entity marker's events in memory, grouped by UUID.
pub struct Store<E: EntityMarker> {
    entities: BTreeMap<Uuid, EntityHistory<E>>,
}

impl<E: EntityMarker> Store<E> {
    /// Groups events by UUID and orders each history by timestamp.
    ///
    /// Equal timestamps retain input order. Timestamp order is not causal order.
    pub fn new(events: impl IntoIterator<Item = Event<E::Payload>>) -> Self {
        let mut entities: BTreeMap<Uuid, Vec<Event<E::Payload>>> = BTreeMap::new();
        for event in events {
            entities.entry(event.id).or_default().push(event);
        }
        let entities = entities
            .into_iter()
            .map(|(id, mut events)| {
                events.sort_by_key(|event| event.timestamp);
                (id, EntityHistory::from_ordered_events(id, events))
            })
            .collect();
        Self { entities }
    }

    /// Consumes the store and yields histories in UUID order.
    pub fn into_histories(self) -> impl Iterator<Item = EntityHistory<E>> {
        self.entities.into_values()
    }

    /// Borrows the history selected by `handle`, if it is present.
    pub fn get(&self, handle: &Handle<E>) -> Option<&EntityHistory<E>> {
        self.entities.get(&handle.id)
    }
}

impl<E: EntityMarker> EntityStore<E> for Store<E> {
    type Error = Infallible;
    type Handle = Handle<E>;

    fn entities(&self) -> Result<impl Iterator<Item = Self::Handle>, Self::Error> {
        Ok(self.entities.keys().copied().map(|id| Handle {
            id,
            marker: PhantomData,
        }))
    }

    fn entity(&self, entity_id: Uuid) -> Result<Option<Self::Handle>, Self::Error> {
        Ok(self.entities.contains_key(&entity_id).then_some(Handle {
            id: entity_id,
            marker: PhantomData,
        }))
    }

    fn earliest_timestamp(
        &self,
        handle: &Self::Handle,
    ) -> Result<Option<TimeUnixNanoSec>, Self::Error> {
        Ok(self.get(handle).map(EntityHistory::earliest_timestamp))
    }

    fn latest_timestamp(
        &self,
        handle: &Self::Handle,
    ) -> Result<Option<TimeUnixNanoSec>, Self::Error> {
        Ok(self.get(handle).map(EntityHistory::latest_timestamp))
    }

    fn num_events(&self, handle: &Self::Handle) -> Result<Option<NonZeroUsize>, Self::Error> {
        Ok(self.get(handle).map(EntityHistory::event_count))
    }
}

#[cfg(test)]
mod tests {
    use quent_events::{EntityMarker, EventPayload};

    use super::*;

    struct Task;

    impl EntityMarker for Task {
        type Payload = TaskEvent;
    }

    #[derive(Debug, PartialEq, Eq)]
    struct TaskEvent(&'static str);

    impl EventPayload for TaskEvent {
        const NAME: &'static str = "Task";
    }

    #[test]
    fn groups_entities_and_preserves_equal_timestamp_input_order() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        let store = Store::<Task>::new([
            Event::new(second, 3, TaskEvent("other")),
            Event::new(first, 2, TaskEvent("late")),
            Event::new(first, 1, TaskEvent("early")),
            Event::new(first, 1, TaskEvent("equal")),
        ]);

        assert_eq!(
            store
                .entities()
                .unwrap()
                .map(|handle| handle.id())
                .collect::<Vec<_>>(),
            [first, second]
        );
        let handle = store.entity(first).unwrap().unwrap();
        assert_eq!(handle.type_name(), "Task");
        assert_eq!(handle.earliest_timestamp(&store).unwrap(), Some(1));
        assert_eq!(handle.latest_timestamp(&store).unwrap(), Some(2));
        assert_eq!(handle.num_events(&store).unwrap(), NonZeroUsize::new(3));
        let history = store.get(&handle).unwrap();
        assert_eq!(history.id(), first);
        assert_eq!(history.event_count(), NonZeroUsize::new(3).unwrap());
        assert_eq!(history.earliest_timestamp(), 1);
        assert_eq!(history.latest_timestamp(), 2);
        assert_eq!(
            history
                .events()
                .iter()
                .map(|event| &event.data)
                .collect::<Vec<_>>(),
            [&TaskEvent("early"), &TaskEvent("equal"), &TaskEvent("late")]
        );
        assert!(store.entity(Uuid::from_u128(3)).unwrap().is_none());
        let other_store =
            Store::<Task>::new([Event::new(Uuid::from_u128(3), 4, TaskEvent("foreign"))]);
        let foreign_handle = other_store.entity(Uuid::from_u128(3)).unwrap().unwrap();
        assert_eq!(foreign_handle.earliest_timestamp(&store).unwrap(), None);
        assert_eq!(foreign_handle.latest_timestamp(&store).unwrap(), None);
        assert_eq!(foreign_handle.num_events(&store).unwrap(), None);
        assert_eq!(
            store
                .into_histories()
                .map(|history| history.into_events().len())
                .collect::<Vec<_>>(),
            [3, 1]
        );
    }
}
