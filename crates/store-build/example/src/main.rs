// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Runs instrumentation and loads its filesystem-exported events.

use std::convert::Infallible;

use demo::{Connection, Demo};
use quent_store::context::ContextSet;
use quent_store::entity::memory;
use quent_store::entity::{EntityHandle, EntityStore};
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
    print_connection_entities(&entities)?;

    Ok(())
}

// The event store is the lowest layer. It returns individual recorded events.
fn print_raw_connection_events(store: &Store<Demo>) -> StoreResult<()> {
    println!("Raw Connection events:");
    for event in store.entity_events::<Connection>()? {
        let event = event?;
        println!("  {event:?}");
    }
    Ok(())
}

// The entity store groups the raw events and returns one handle per entity UUID.
fn print_connection_entities(entities: &memory::Store<Connection>) -> Result<(), Infallible> {
    println!("Connection entities:");
    for connection in EntityStore::<Connection>::entities(entities)? {
        println!("  Connection {}:", connection.id());
        for event in EntityStore::<Connection>::events(entities, &connection)? {
            println!("    {event:?}");
        }
    }
    Ok(())
}
