// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#![cfg(test)]

use quent_store::{
    Error,
    entity::{BorrowedEventSequenceStore, EntityHandle, EntityStore, Id, native::Store},
};

#[allow(unused_imports, dead_code)]
mod model {
    include!("model.rs");
}

#[allow(unused_imports, dead_code)]
mod events_only {
    include!("events_only.rs");
}

use crate::model::{
    EntityRef, Event, Uuid,
    nested::r#type::{Worker, WorkerEvent, WorkerEvents, WorkerStore},
};

#[test]
fn indexed_store_returns_borrowed_views_and_retains_raw_sequences() {
    let id = Id::<Worker>::new(Uuid::from_u128(8));
    let missing = Id::<Worker>::new(Uuid::from_u128(9));
    let sequences = Store::<Worker>::new([
        Event::new(id.uuid(), 20, WorkerEvent::Update { string: "later".to_owned() }),
        Event::new(id.uuid(), 10, WorkerEvent::Renamed { string: "name".to_owned() }),
    ]);
    let original_buffer = sequences.event_sequence(id).unwrap().events().as_ptr();
    let store = WorkerStore::try_from(sequences).unwrap();
    assert_eq!(store.entities().unwrap().collect::<Vec<_>>(), [id]);
    assert!(store.entity(missing).unwrap().is_none());
    assert!(matches!(store.event_sequence(missing), Err(Error::MissingEntity(uuid)) if uuid == missing.uuid()));
    let entity = store.entity(id).unwrap().unwrap();
    assert_eq!(entity.renamed().unwrap().string(), "name");
    assert_eq!(entity.update().next().unwrap().timestamp(), 20);
    assert!(std::ptr::eq(entity, store.entity(id).unwrap().unwrap()));
    let reference = EntityRef::<Worker, _>::new(id.uuid(), "reference data".to_owned());
    assert!(std::ptr::eq(entity, store.entity(&reference).unwrap().unwrap()));
    assert_eq!(reference.data, "reference data");
    assert!(std::ptr::eq(entity, store.entity(reference).unwrap().unwrap()));
    assert!(std::ptr::eq(entity, store.entity(id.uuid()).unwrap().unwrap()));
    assert_eq!(store.event_sequence(id).unwrap().events().as_ptr(), original_buffer);
    assert_eq!(entity.earliest_timestamp(), 10);
    assert_eq!(entity.latest_timestamp(), 20);
    assert_eq!(entity.num_events().get(), 2);

    fn name<S: EntityStore<Worker>>(store: &S, id: Id<Worker>) -> String
    where
        for<'a> S::Handle<'a>: WorkerEvents,
    {
        store.entity(id).ok().unwrap().unwrap().renamed().unwrap().string().clone()
    }
    assert_eq!(name(&store, id), "name");
}

#[test]
fn indexed_store_rejects_duplicate_once_events() {
    use crate::model::{Parent, ParentEvent, ParentStore};
    let id = Uuid::from_u128(10);
    let sequences = Store::<Parent>::new([
        Event::new(id, 1, ParentEvent::Created),
        Event::new(id, 2, ParentEvent::Created),
    ]);
    assert!(matches!(ParentStore::try_from(sequences), Err(Error::DuplicateOnceEvent {
        entity_id, event_name: "created"
    }) if entity_id == id));
}

#[test]
fn borrows_nested_fields_without_clone_derives() {
    let id = Uuid::from_u128(1);
    let parent_id = Uuid::from_u128(2);
    let name = "worker".to_owned();
    let name_pointer = name.as_ptr();
    let store = Store::<Worker>::new([
        Event::new(
            id,
            10,
            WorkerEvent::Type {
                string: name,
                option: None,
                vec: vec!["label".to_owned()],
                some: true,
                r#type: "kind".to_owned(),
                http2_code: 200,
                uuid: parent_id,
                attributes: model::DynamicAttributes::default(),
                shared: model::Shared {
                    label: "shared".to_owned(),
                },
                local: model::nested::Record { count: 3 },
                parent: EntityRef::new(parent_id, ()),
                peers: vec![EntityRef::new(
                    parent_id,
                    model::Shared {
                        label: "peer".to_owned(),
                    },
                )],
            },
        ),
        Event::new(
            id,
            20,
            WorkerEvent::Renamed {
                string: "renamed".to_owned(),
            },
        ),
    ]);

    let sequence_pointer = store.event_sequence(id.into()).unwrap().events().as_ptr();
    let store = WorkerStore::try_from(store).unwrap();
    let events = store.entity(id).unwrap().unwrap();
    assert_eq!(events.event_sequence().events().as_ptr(), sequence_pointer);
    assert_eq!(events.id().uuid(), id);
    assert_eq!(events.earliest_timestamp(), 10);
    assert_eq!(events.latest_timestamp(), 20);
    assert_eq!(events.num_events().get(), 2);
    let _debug = format!("{events:?}");
    let event = events.r#type().unwrap();
    assert_eq!(event.timestamp(), 10);
    assert_eq!(event.string().as_str(), "worker");
    assert_eq!(event.string().as_ptr(), name_pointer);
    assert_eq!(event.option(), &None);
    assert_eq!(event.vec().as_slice(), ["label"]);
    assert!(*event.some());
    assert_eq!(event.r#type().as_str(), "kind");
    assert_eq!(*event.http2_code(), 200);
    let uuid: &model::Uuid = event.uuid();
    assert_eq!(*uuid, parent_id);
    let attributes: &model::DynamicAttributes = event.attributes();
    assert!(attributes.0.is_empty());
    let record: &model::Shared = event.shared();
    assert_eq!(record.label, "shared");
    let local: &model::nested::Record = event.local();
    assert_eq!(local.count, 3);
    let parent: &EntityRef<model::Parent> = event.parent();
    assert_eq!(parent.target, parent_id);
    let peers: &Vec<EntityRef<model::Parent, model::Shared>> = event.peers();
    assert_eq!(peers[0].data.label, "peer");
    assert_eq!(events.renamed().unwrap().string().as_str(), "renamed");
    assert_eq!(events.update().count(), 0);
}

#[test]
fn groups_multi_events_stably_and_leaves_once_events_absent() {
    let id = Uuid::from_u128(1);
    let store = Store::<Worker>::new([(20, "later"), (10, "first"), (10, "second")].map(
        |(timestamp, string)| {
            Event::new(
                id,
                timestamp,
                WorkerEvent::Update {
                    string: string.to_owned(),
                },
            )
        },
    ).into_iter().chain([Event::new(id, 15, WorkerEvent::Renamed { string: "renamed".to_owned() })]));
    let store = WorkerStore::try_from(store).unwrap();
    let events = store.entity(id).unwrap().unwrap();
    assert_eq!(events.earliest_timestamp(), 10);
    assert_eq!(events.latest_timestamp(), 20);
    assert_eq!(events.num_events().get(), 4);
    assert!(events.r#type().is_none());
    assert_eq!(events.renamed().unwrap().timestamp(), 15);
    assert_eq!(events.event_sequence().events().len(), 4);
    assert_eq!(
        events
            .update()
            .map(|event| (event.timestamp(), event.string().as_str()))
            .collect::<Vec<_>>(),
        [(10, "first"), (10, "second"), (20, "later")]
    );
}

#[test]
fn rejects_duplicates_including_equal_timestamps_and_empty_payloads() {
    use crate::model::{
        Parent, ParentEvent, ParentEvents, ParentStore,
    };
    let id = Uuid::from_u128(1);
    let single = Store::<Parent>::new([Event::new(id, 10, ParentEvent::Created)]);
    let single = ParentStore::try_from(single).unwrap();
    let grouped = single.entity(id).unwrap().unwrap();
    assert_eq!(grouped.created().timestamp(), 10);
    for timestamp in [10, 20] {
        let store = Store::<Parent>::new([
            Event::new(id, 10, ParentEvent::Created),
            Event::new(id, timestamp, ParentEvent::Created),
        ]);
        assert!(matches!(
            ParentStore::try_from(store).err(),
            Some(Error::DuplicateOnceEvent {
                entity_id,
                event_name: "created"
            }) if entity_id == id
        ));
    }
}

#[test]
fn a_sole_multi_event_still_returns_all_occurrences() {
    use crate::model::{
        Stream, StreamEvent, StreamEvents, StreamStore,
    };
    let id = Uuid::from_u128(4);
    let store = Store::<Stream>::new([20, 10, 10].map(|timestamp| {
        Event::new(id, timestamp, StreamEvent::Tick)
    }));
    let store = StreamStore::try_from(store).unwrap();
    let events = store.entity(id).unwrap().unwrap();
    assert_eq!(events.tick().map(|event| event.timestamp()).collect::<Vec<_>>(), [10, 10, 20]);
}

#[test]
fn payload_fields_do_not_shadow_the_recorded_timestamp() {
    let id = Uuid::from_u128(3);
    let store = Store::<Worker>::new([Event::new(
        id,
        42,
        WorkerEvent::Metadata {
            id: 7,
            timestamp: 8,
            data: "data".to_owned(),
            event_id: "event_id".to_owned(),
            event_timestamp: "event_timestamp".to_owned(),
            field_0: "field_0".to_owned(),
        },
    )]);
    let store = WorkerStore::try_from(store).unwrap();
    let grouped = store.entity(id).unwrap().unwrap();
    let event = grouped.metadata().unwrap();
    assert_eq!(event.timestamp(), 42);
    assert_eq!(*event.id(), 7);
    assert_eq!(*event.field_timestamp(), 8);
    assert_eq!(event.data().as_str(), "data");
    assert_eq!(event.event_id().as_str(), "event_id");
    assert_eq!(event.event_timestamp().as_str(), "event_timestamp");
    assert_eq!(event.field_0().as_str(), "field_0");
}

#[test]
fn checked_handles_reject_other_variants_and_borrow_valid_events() {
    use crate::model::entity_events::nested::r#type::worker::RenamedHandle;

    let id = Uuid::from_u128(5);
    let event = Event::new(id, 30, WorkerEvent::Update { string: "update".to_owned() });
    let rejected = RenamedHandle::try_from(&event).err().unwrap();
    assert!(std::ptr::eq(rejected, &event));

    let event = Event::new(id, 40, WorkerEvent::Renamed { string: "renamed".to_owned() });
    let handle = RenamedHandle::try_from(&event).unwrap();
    assert_eq!(handle.timestamp(), 40);
    let copy = handle;
    let WorkerEvent::Renamed { string } = &event.data else { unreachable!() };
    assert!(std::ptr::eq(copy.string(), string));
}
