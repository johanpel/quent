// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Runs instrumentation and loads its filesystem-exported events.

use demo::entity_events::connection::{ConnectionEvents, ConnectionStore};
use demo::{Connection, Demo, Server, ServerEvent};
use quent_events::EventPayload;
use quent_store::{
    context::ContextSet,
    entity::{BorrowedEventSequenceStore, EntityStore, native},
    event::{EventLoader, filesystem},
};

#[allow(unused_imports, dead_code)]
mod demo {
    include!(concat!(env!("OUT_DIR"), "/demo.rs"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = tempfile::tempdir()?;
    let context_id = quent_instrumentation_build_example::run_with_ndjson(output.path())?;

    // Prepare the filesystem event loader.
    //
    // A loader loads raw events for one particular entity type.
    // Use [`quent_store::event::CombinedEventLoader`] to load events of any type.
    let loader = filesystem::Loader::<Demo>::new(output.path(), ContextSet::one(context_id));
    print_raw_connection_events(&loader)?;

    // Load all "Connection" and "Server" events into a Rust-native in-memory
    // "entity store".
    //
    // An entity store can list entities and look up their events by id.
    let connections = ConnectionStore::try_from_loader(&loader)?;
    let servers = native::Store::<Server>::try_from_loader(&loader)?;
    print_connection_events(&connections)?;

    // Take the ID of the first entity, then look it up to turn it into a handle
    // that can index into the event data using schema-defined accessors.
    if let Some(id) = connections.entities()?.next() {
        let entity = connections.entity(id)?.unwrap();
        // Note we unwrap because we just got the id from the store, so it
        // should exist there. If this ID is taken from data, it would be
        // prudent to properly handle a missing entity.

        // Access some properties common to all entities.
        println!(
            "Observed entity {} between {} and {}",
            entity.properties().id,
            entity.properties().earliest_timestamp,
            entity.properties().latest_timestamp
        );

        // Access some entity-specific properties:
        if let Some(opened) = entity.opened() {
            println!("Opened connection at {}", opened.timestamp());
            println!(
                "Connected to peer {}:{}",
                opened.peer().host,
                opened.peer().port
            );
            // Since the Connection refers to its parent Server, we can try to
            // look the parent up:
            println!(": {}", opened.host().target);
            // TODO
        }
    }

    Ok(())
}

// The loader returns individual recorded events.
fn print_raw_connection_events(loader: &filesystem::Loader<Demo>) -> quent_store::Result<()> {
    println!("Raw Connection events:");
    for event in EventLoader::<Connection>::events(loader)?.take(2) {
        let event = event?;
        println!("  {event:?}");
    }
    println!("  ...");
    Ok(())
}

fn print_connection_events<S>(connections: &S) -> Result<(), S::Error>
where
    S: BorrowedEventSequenceStore<Connection>,
{
    // Take the ID of the first Connection entity from the store.
    if let Some(connection_id) = connections.entities()?.next() {
        println!("\nFirst connection {} events:", connection_id.uuid());
        // Per the trait bound for S, we can borrow the underlying event
        // sequence of the entity:
        for event in connections.event_sequence(connection_id)?.events() {
            println!("  {event:?}");
        }
    }
    Ok(())
}

fn print_connection_fields(events: &impl ConnectionEvents) {
    println!("\nConnection {} fields:", events.id().uuid());

    // `opened()` returns an optional handle; `data()` iterates over handles.
    // Handles borrow fields from the original event and expose its timestamp.
    println!(
        "  opened: {:?}",
        events
            .opened()
            .map(|event| (event.timestamp(), event.peer()))
    );
    println!(
        "  data: {:?}",
        events
            .data()
            .map(|event| (event.timestamp(), *event.bytes()))
            .collect::<Vec<_>>()
    );
}
