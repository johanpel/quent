// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Rust-native in-memory entity storage implementation support.

use std::{marker::PhantomData, num::NonZeroUsize};

use quent_events::{EntityMarker, Event};
use quent_time::{OrderedCollector, TimeUnixNanoSec};
use rustc_hash::FxHashMap as HashMap;
use uuid::Uuid;

use crate::{
    Error,
    entity::{
        BorrowedEventSequenceStore, EntityHandle, EntityStore, Id, OwnedEventSequenceStore,
        sequence::EventSequence,
    },
    event::EventLoader,
};

/// Stores one entity marker's events in memory, grouped by UUID.
///
/// The default representation holds raw sequences. Generated store aliases
/// select indexed entities that provide event-type-specific access.
pub struct Store<E: EntityMarker, T = EventSequence<E>> {
    entities: HashMap<Uuid, T>,
    marker: PhantomData<fn() -> E>,
}

impl<E: EntityMarker> Store<E> {
    /// Groups events by UUID and orders each sequence by timestamp.
    ///
    /// # Performance notes
    ///
    /// For n input events, construction is O(n) when each entity's events
    /// arrive in timestamp order. Inserting m out-of-order events for one
    /// entity can take O(m^2).
    ///
    /// Practically speaking, when leveraging Quent's default channels and
    /// exporters in instrumentation and importers, out-of-order events will
    /// only occur when entities emit events from multiple contexts.
    pub fn new(events: impl IntoIterator<Item = Event<E::Payload>>) -> Self {
        // OrderedCollector appends the common in-order case and inserts late arrivals in order.
        let mut entities: HashMap<Uuid, OrderedCollector<Event<E::Payload>>> = HashMap::default();
        for event in events {
            entities.entry(event.id).or_default().push(event);
        }
        Self::from_time_ordered_collectors(entities)
    }

    /// Loads events, grouping them by UUID and ordering each sequence by timestamp.
    ///
    /// # Errors
    ///
    /// Returns the first loader error and stops reading events.
    pub fn try_from_loader<L: EventLoader<E>>(loader: &L) -> Result<Self, L::Error> {
        let mut entities: HashMap<Uuid, OrderedCollector<Event<E::Payload>>> = HashMap::default();
        for event in loader.events()? {
            let event = event?;
            entities.entry(event.id).or_default().push(event);
        }
        Ok(Self::from_time_ordered_collectors(entities))
    }

    fn from_time_ordered_collectors(
        entities: HashMap<Uuid, OrderedCollector<Event<E::Payload>>>,
    ) -> Self {
        let entities = entities
            .into_iter()
            .map(|(id, events)| (id, EventSequence::from_ordered_events(id, events)))
            .collect();
        Self {
            entities,
            marker: PhantomData,
        }
    }
}

impl<E: EntityMarker, T: StoredEntity<E>> Store<E, T> {
    /// Consumes the store and yields sequences in unspecified order.
    pub fn into_sequences(self) -> impl Iterator<Item = EventSequence<E>> {
        self.entities.into_values().map(T::into_sequence)
    }
}

impl<E: EntityMarker, I: EventIndex<E>> Store<E, NativeEntity<E, I>> {
    /// Loads and indexes entity event sequences.
    ///
    /// # Errors
    ///
    /// Returns the first loader or indexing error. Indexing rejects duplicate
    /// once-events.
    pub fn try_from_loader<L: EventLoader<E>>(loader: &L) -> Result<Self, L::Error>
    where
        L::Error: From<Error>,
    {
        Self::try_from(Store::<E>::try_from_loader(loader)?).map_err(L::Error::from)
    }
}

/// # Errors
///
/// Returns the first error from indexing an entity's sequence.
impl<E: EntityMarker, I: EventIndex<E>> TryFrom<Store<E>> for Store<E, NativeEntity<E, I>> {
    type Error = Error;

    fn try_from(store: Store<E>) -> Result<Self, Self::Error> {
        let entities = store
            .entities
            .into_iter()
            .map(|(id, sequence)| NativeEntity::try_from(sequence).map(|entity| (id, entity)))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            entities,
            marker: PhantomData,
        })
    }
}

impl<E: EntityMarker, T: StoredEntity<E>> EntityStore<E> for Store<E, T> {
    type Error = Error;
    type Handle<'a>
        = &'a T
    where
        Self: 'a;

    fn entities(&self) -> Result<impl Iterator<Item = Id<E>>, Self::Error> {
        Ok(self.entities.keys().copied().map(Id::new))
    }

    fn entity(&self, entity_id: impl Into<Id<E>>) -> Result<Option<Self::Handle<'_>>, Self::Error> {
        Ok(self.entities.get(&entity_id.into().uuid()))
    }
}

impl<E: EntityMarker, T: StoredEntity<E>> BorrowedEventSequenceStore<E> for Store<E, T> {
    fn event_sequence(&self, entity_id: Id<E>) -> Result<&EventSequence<E>, Self::Error> {
        self.entities
            .get(&entity_id.uuid())
            .map(T::sequence)
            .ok_or(Error::MissingEntity(entity_id.uuid()))
    }
}

impl<E: EntityMarker, T: StoredEntity<E>> OwnedEventSequenceStore<E> for Store<E, T>
where
    E::Payload: Clone,
{
    fn event_sequence_owned(&self, entity_id: Id<E>) -> Result<EventSequence<E>, Self::Error> {
        self.event_sequence(entity_id).cloned()
    }
}

/// Provides raw sequence access for values held by a native store.
#[doc(hidden)]
pub trait StoredEntity<E: EntityMarker>: EntityHandle<Entity = E> {
    fn sequence(&self) -> &EventSequence<E>;
    fn into_sequence(self) -> EventSequence<E>;
}

impl<E: EntityMarker> StoredEntity<E> for EventSequence<E> {
    fn sequence(&self) -> &EventSequence<E> {
        self
    }
    fn into_sequence(self) -> EventSequence<E> {
        self
    }
}

impl<E: EntityMarker, I: EventIndex<E>> StoredEntity<E> for NativeEntity<E, I> {
    fn sequence(&self) -> &EventSequence<E> {
        &self.sequence
    }
    fn into_sequence(self) -> EventSequence<E> {
        self.sequence
    }
}

/// Allows generated indices to record positions in an entity event sequence.
///
/// [`Default`] must produce empty indices.
#[doc(hidden)]
pub trait EventIndex<E: EntityMarker>: Default {
    /// Records the position of an event in its matching group.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DuplicateOnceEvent`] if the corresponding once-event
    /// group is occupied, leaving the existing index unchanged.
    fn push(&mut self, event: &Event<E::Payload>, position: usize) -> Result<(), Error>;
}

/// Storage for one entity's event sequence and event-type indices.
#[doc(hidden)]
pub struct NativeEntity<E: EntityMarker, I: EventIndex<E>> {
    /// The sequence of events this entity emitted.
    sequence: EventSequence<E>,
    /// Positions in the sequence grouped by entity event type.
    event_index: I,
}

impl<E: EntityMarker, I: EventIndex<E>> NativeEntity<E, I> {
    /// Borrows the event-type indices.
    #[doc(hidden)]
    pub fn event_index(&self) -> &I {
        &self.event_index
    }

    /// Borrows the raw event sequence.
    pub fn event_sequence(&self) -> &EventSequence<E> {
        &self.sequence
    }
}

impl<E: EntityMarker, I: EventIndex<E>> EntityHandle for NativeEntity<E, I> {
    type Entity = E;

    fn id(&self) -> Id<E> {
        self.sequence.id()
    }

    fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        self.sequence.earliest_timestamp()
    }

    fn latest_timestamp(&self) -> TimeUnixNanoSec {
        self.sequence.latest_timestamp()
    }

    fn num_events(&self) -> NonZeroUsize {
        self.sequence.event_count()
    }
}

/// # Errors
///
/// Returns the first error from [`EventIndex::push`].
impl<E: EntityMarker, I: EventIndex<E>> TryFrom<EventSequence<E>> for NativeEntity<E, I> {
    type Error = Error;

    fn try_from(sequence: EventSequence<E>) -> Result<Self, Self::Error> {
        let mut entity = Self {
            sequence,
            event_index: I::default(),
        };
        for (position, event) in entity.sequence.events().iter().enumerate() {
            entity.event_index.push(event, position)?;
        }
        Ok(entity)
    }
}

impl<E: EntityMarker, I: EventIndex<E>> std::fmt::Debug for NativeEntity<E, I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeEntity")
            .field("id", &self.id().uuid())
            .field("earliest_timestamp", &self.earliest_timestamp())
            .field("latest_timestamp", &self.latest_timestamp())
            .field("event_count", &self.num_events())
            .finish_non_exhaustive()
    }
}

/// Inserts a value into an empty once-event slot.
///
/// # Errors
///
/// Returns the incoming value when occupied, leaving the existing value unchanged.
#[doc(hidden)]
pub fn insert_once<T>(slot: &mut Option<T>, value: T) -> Result<(), T> {
    if slot.is_some() {
        return Err(value);
    }
    *slot = Some(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use quent_events::{EntityMarker, EventPayload};

    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct Task;

    impl EntityMarker for Task {
        type Payload = TaskEvent;
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TaskEvent(&'static str);

    impl EventPayload for TaskEvent {
        const NAME: &'static str = "Task";
    }

    struct TestLoader {
        fail_open: bool,
        fail_read: bool,
        reads: Rc<Cell<usize>>,
    }

    impl EventLoader<Task> for TestLoader {
        type Error = &'static str;

        fn events(
            &self,
        ) -> Result<crate::event::EventIterator<TaskEvent, Self::Error>, Self::Error> {
            if self.fail_open {
                return Err("open failed");
            }
            let id = Uuid::from_u128(1);
            let events = [
                Ok(Event::new(id, 2, TaskEvent("late"))),
                if self.fail_read {
                    Err("read failed")
                } else {
                    Ok(Event::new(id, 1, TaskEvent("early")))
                },
                Ok(Event::new(id, 1, TaskEvent("equal"))),
                Ok(Event::new(Uuid::from_u128(2), 3, TaskEvent("other"))),
            ];
            let reads = self.reads.clone();
            Ok(Box::new(events.into_iter().inspect(move |_| {
                reads.set(reads.get() + 1);
            })))
        }
    }

    #[test]
    fn loads_and_groups_events_without_losing_equal_timestamp_order() {
        let loader = TestLoader {
            fail_open: false,
            fail_read: false,
            reads: Rc::default(),
        };
        let store = Store::<Task>::try_from_loader(&loader).unwrap();
        assert_eq!(loader.reads.get(), 4);
        assert_eq!(store.entities().unwrap().count(), 2);
        let handle = store.entity(Uuid::from_u128(1)).unwrap().unwrap();
        let sequence = store.event_sequence(handle.id()).unwrap();
        assert_eq!(
            sequence
                .events()
                .iter()
                .map(|event| event.data.0)
                .collect::<Vec<_>>(),
            ["early", "equal", "late"]
        );
    }

    #[test]
    fn propagates_loader_errors_and_stops_at_the_first_read_failure() {
        for (fail_open, expected, reads) in [(true, "open failed", 0), (false, "read failed", 2)] {
            let loader = TestLoader {
                fail_open,
                fail_read: true,
                reads: Rc::default(),
            };
            assert!(
                matches!(Store::<Task>::try_from_loader(&loader), Err(error) if error == expected)
            );
            assert_eq!(loader.reads.get(), reads);
        }
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

        let mut ids = store
            .entities()
            .unwrap()
            .map(|id| id.uuid())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, [first, second]);
        let handle = store.entity(first).unwrap().unwrap();
        assert_eq!(EventSequence::<Task>::type_name(), "Task");
        assert_eq!(handle.earliest_timestamp(), 1);
        assert_eq!(handle.latest_timestamp(), 2);
        assert_eq!(handle.num_events(), NonZeroUsize::new(3).unwrap());
        let sequence = store.event_sequence(handle.id()).unwrap();
        let owned = store.event_sequence_owned(handle.id()).unwrap();
        assert_eq!(owned.id().uuid(), first);
        assert_eq!(owned.event_count(), sequence.event_count());
        assert_ne!(owned.events().as_ptr(), sequence.events().as_ptr());
        let mut owned_events = owned.into_events();
        assert_eq!(owned_events[0].data, TaskEvent("early"));
        owned_events[0].data = TaskEvent("changed");
        assert_eq!(sequence.events()[0].data, TaskEvent("early"));
        assert_eq!(sequence.id().uuid(), first);
        assert_eq!(sequence.event_count(), NonZeroUsize::new(3).unwrap());
        assert_eq!(sequence.earliest_timestamp(), 1);
        assert_eq!(sequence.latest_timestamp(), 2);
        assert_eq!(
            sequence
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
        assert!(
            matches!(store.event_sequence(foreign_handle.id()), Err(Error::MissingEntity(id)) if id == foreign_handle.id().uuid())
        );
        assert!(
            matches!(store.event_sequence_owned(foreign_handle.id()), Err(Error::MissingEntity(id)) if id == foreign_handle.id().uuid())
        );
        let mut lengths = store
            .into_sequences()
            .map(|sequence| sequence.into_events().len())
            .collect::<Vec<_>>();
        lengths.sort_unstable();
        assert_eq!(lengths, [1, 3]);
    }

    #[test]
    fn inserting_once_returns_the_rejected_event_and_preserves_the_existing_event() {
        struct Payload(&'static str);

        let id = Uuid::from_u128(1);
        let mut slot = None;
        assert!(insert_once(&mut slot, Event::new(id, 10, Payload("first"))).is_ok());

        let rejected = insert_once(&mut slot, Event::new(id, 10, Payload("second")))
            .err()
            .unwrap();
        assert_eq!(rejected.id, id);
        assert_eq!(rejected.timestamp, 10);
        assert_eq!(rejected.data.0, "second");

        let event = slot.unwrap();
        assert_eq!(event.id, id);
        assert_eq!(event.timestamp, 10);
        assert_eq!(event.data.0, "first");
    }
}
