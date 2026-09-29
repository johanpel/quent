// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! In-memory entity storage.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::marker::PhantomData;

use quent_events::{EntityMarker, Event};
use uuid::Uuid;

use super::{EntityHandle, EntityStore};

/// An owned handle for one entity marker.
pub struct Handle<E> {
    id: Uuid,
    marker: PhantomData<fn() -> E>,
}

impl<E> EntityHandle for Handle<E> {
    fn id(&self) -> Uuid {
        self.id
    }
}

/// Stores one entity marker's events in memory, grouped by UUID.
pub struct Store<E: EntityMarker> {
    entities: BTreeMap<Uuid, Vec<Event<E::Payload>>>,
    marker: PhantomData<fn() -> E>,
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
        for history in entities.values_mut() {
            history.sort_by_key(|event| event.timestamp);
        }
        Self {
            entities,
            marker: PhantomData,
        }
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

    fn events<'a>(
        &'a self,
        handle: &Self::Handle,
    ) -> Result<impl Iterator<Item = &'a Event<E::Payload>>, Self::Error>
    where
        E::Payload: 'a,
    {
        Ok(self
            .entities
            .get(&handle.id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter())
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
        assert_eq!(
            store
                .events(&handle)
                .unwrap()
                .map(|event| &event.data)
                .collect::<Vec<_>>(),
            [&TaskEvent("early"), &TaskEvent("equal"), &TaskEvent("late")]
        );
        assert!(store.entity(Uuid::from_u128(3)).unwrap().is_none());
    }
}
