// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, Mutex};

use crate::demo::{
    Connection, Context, Demo, DemoEvent, Handle, Noop, Query, QueryEvent, Server, Uuid,
};
use quent_instrumentation::EventCallback;

#[test]
fn ordinary_handles_preserve_size_and_once_event_behavior() {
    assert_eq!(
        std::mem::size_of::<Handle<Connection>>(),
        std::mem::size_of::<quent_instrumentation::HandleInner<Connection>>()
    );

    let context = Context::<Demo>::try_new(Noop).unwrap();
    let mut server = context.observer::<Server>().handle();
    assert!(server.booted().is_ok());
    assert!(server.booted_emitted());
    assert!(server.booted().is_err());
}

#[test]
fn fsm_sequences_are_scoped_to_each_handle() {
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let callback = {
        let recorded = Arc::clone(&recorded);
        EventCallback::new(move |event| recorded.lock().unwrap().push(event))
    };
    let context = Context::<Demo>::try_new(callback).unwrap();
    let connection = context.observer::<Connection>().handle();

    let first = context.observer::<Query>().handle();
    let first_id = first.uuid();
    let _first = first
        .submitted("first".to_string(), connection.as_entity_ref())
        .running(10)
        .running(20)
        .ready(true);

    let second_id = Uuid::from_u128(42);
    let second = context.observer::<Query>().handle_with_id(second_id);
    assert_eq!(second.uuid(), second_id);
    let _second = second
        .submitted("second".to_string(), connection.as_entity_ref())
        .running(30)
        .ready(true);

    drop((_first, _second, connection));
    drop(context);

    let sequences = recorded
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| {
            let DemoEvent::Query(query) = &event.data else {
                return None;
            };
            let sequence = match query {
                QueryEvent::Submitted { seq, .. }
                | QueryEvent::Running { seq, .. }
                | QueryEvent::Ready { seq, .. } => *seq,
            };
            Some((event.id, sequence))
        })
        .collect::<Vec<_>>();

    assert_eq!(
        sequences,
        vec![
            (first_id, 0),
            (first_id, 1),
            (first_id, 2),
            (first_id, 3),
            (second_id, 0),
            (second_id, 1),
            (second_id, 2),
        ]
    );
}

#[test]
fn fsm_sequence_wraps_after_u16_max() {
    let sequences = Arc::new(Mutex::new(Vec::new()));
    let callback = {
        let sequences = Arc::clone(&sequences);
        EventCallback::new(move |event| {
            let DemoEvent::Query(query) = event.data else {
                return;
            };
            let sequence = match query {
                QueryEvent::Submitted { seq, .. }
                | QueryEvent::Running { seq, .. }
                | QueryEvent::Ready { seq, .. } => seq,
            };
            sequences.lock().unwrap().push(sequence);
        })
    };
    let context = Context::<Demo>::try_new(callback).unwrap();
    let connection = context.observer::<Connection>().handle();
    let mut query = context
        .observer::<Query>()
        .handle()
        .submitted("wrap".to_string(), connection.as_entity_ref())
        .running(0);

    for _ in 2..=u16::MAX {
        query = query.running(0);
    }
    let query = query.running(0);

    drop((query, connection));
    drop(context);

    let sequences = sequences.lock().unwrap();
    assert_eq!(sequences.len(), usize::from(u16::MAX) + 2);
    assert_eq!(sequences[0], 0);
    assert_eq!(sequences[usize::from(u16::MAX)], u16::MAX);
    assert_eq!(sequences[usize::from(u16::MAX) + 1], 0);
}
