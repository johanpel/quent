// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_instrumentation::{EventCallback, ExporterOptions};

use crate::demo::{ConnectionHandle, ConnectionObserver, DemoContext, Event, Uuid};

#[allow(unused)]
mod demo {
    include!(concat!(env!("OUT_DIR"), "/demo.rs"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context: DemoContext = demo::DemoContext::try_new(Some(debug_printing_exporter()))?;

    // Boot a server; its instance id is the target the connection references.
    let mut server = context.server_observer().handle();
    server.booted()?;

    let observer: ConnectionObserver = context.connection_observer();

    // The handle (may) hold per-instance state that enforces once-cardinality,
    // hence it is mut so it can update its state after producing a once-event.
    let mut conn: ConnectionHandle = observer.handle();

    conn.opened(
        demo::Endpoint {
            host: "localhost".to_owned(),
            port: 8080,
        },
        Uuid::nil(),
        server.as_entity_ref(),
    )?;
    conn.data(1234, None)?;

    // `extra` is the schema's `dynamic` field: a runtime-keyed bag of typed
    // attributes, so callers attach whatever key/values they have on hand.
    let mut extra = demo::CustomAttributes::new();
    extra.add_string("peer_agent", "curl/8.4");
    extra.add_u64("chunk_index", 3);
    extra.add_bool("compressed", true);
    conn.data(
        5678,
        Some(demo::Meta {
            tags: vec!["tls".to_string(), "keepalive".to_string()],
            extra,
        }),
    )?;

    // `ref` field carrying data: the server reference plus details of the edge.
    conn.routed(server.as_entity_ref_with(demo::Route { hops: 3 }))?;

    conn.closed()?;

    // Emitting a once-event a second time fails.
    assert!(conn.closed_emitted());
    assert!(conn.closed().is_err());

    Ok(())
}

/// Return an exporter that debug-prints each emitted event's payload.
fn debug_printing_exporter() -> ExporterOptions {
    ExporterOptions::Callback(EventCallback::new(|recorded| {
        if let Some(event) = recorded
            .event
            .downcast_ref::<Event<demo::ConnectionEvent>>()
        {
            println!("[{} @ {}] {:?}", event.id, event.timestamp, event.data);
        } else if let Some(event) = recorded.event.downcast_ref::<Event<demo::ServerEvent>>() {
            println!("[{} @ {}] {:?}", event.id, event.timestamp, event.data);
        } else {
            unreachable!()
        }
    }))
}
