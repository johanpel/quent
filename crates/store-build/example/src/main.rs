// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Runs instrumentation and loads its filesystem-exported events.

use std::convert::Infallible;

use demo::{Connection, ConnectionEvent, Demo, Uuid};
use quent_store::context::ContextSet;
use quent_store::entity::memory;
use quent_store::entity::sequence::EventSequence;
use quent_store::entity::{BorrowedEventSequenceStore, EntityHandle, EntityStore};
use quent_store::event::EventStore;
use quent_store::event::filesystem::{Result as StoreResult, Store};

#[allow(unused_imports, dead_code)]
mod demo {
    include!(concat!(env!("OUT_DIR"), "/demo.rs"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = tempfile::tempdir()?;
    let context_id = quent_instrumentation_build_example::run_with_ndjson(output.path())?;

    let store = Store::<Demo>::new(output.path(), ContextSet::one(context_id));

    print_raw_connection_events(&store)?;
    let events = store
        .entity_events::<Connection>()?
        .collect::<StoreResult<Vec<_>>>()?;
    let entities = memory::Store::<Connection>::new(events);
    print_connection_summaries(&entities)?;

    Ok(())
}

// The event store is the lowest layer. It returns individual recorded events.
fn print_raw_connection_events(store: &Store<Demo>) -> StoreResult<()> {
    println!("Raw Connection events:");
    for event in store.entity_events::<Connection>()?.take(2) {
        let event = event?;
        println!("  {event:?}");
    }
    println!("  ...");
    Ok(())
}

struct ConnectionSummary {
    id: Uuid,
    data_bytes: u64,
    closed: bool,
}

// Application-specific analysis reads a sequence without cloning its events.
fn summarize_connection(sequence: &EventSequence<Connection>) -> ConnectionSummary {
    let id = sequence.id();
    let mut data_bytes = 0;
    let mut closed = false;
    for event in sequence.events() {
        match &event.data {
            ConnectionEvent::Data { bytes, .. } => data_bytes += *bytes,
            ConnectionEvent::Closed => closed = true,
            _ => {}
        }
    }
    ConnectionSummary {
        id,
        data_bytes,
        closed,
    }
}

// The entity store supplies common properties and an ordered sequence for each entity.
fn print_connection_summaries(entities: &memory::Store<Connection>) -> Result<(), Infallible> {
    println!("\nConnection summaries:");
    for connection in entities.entities()? {
        let sequence = entities
            .sequence(&connection)?
            .expect("handle belongs to store");
        let summary = summarize_connection(sequence);
        println!(
            "  {} {}: {} events, timestamps {}..{}, {} data bytes, closed: {}",
            connection.type_name(),
            summary.id,
            entities
                .num_events(&connection)?
                .expect("handle belongs to store"),
            entities
                .earliest_timestamp(&connection)?
                .expect("handle belongs to store"),
            entities
                .latest_timestamp(&connection)?
                .expect("handle belongs to store"),
            summary.data_bytes,
            summary.closed
        );
    }
    Ok(())
}
