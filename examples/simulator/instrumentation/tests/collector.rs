// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#![cfg(feature = "collector")]

use std::sync::{Arc, Mutex};

use quent_collector_client::{CollectorSink, serialize_event};
use quent_instrumentation::{EntityEvent, EventCallback};
use quent_simulator_instrumentation::{
    Context, DynamicAttributes, EngineEvent, EngineImplementationAttributes, Event, Simulator,
    SimulatorEvent, Uuid,
};

#[test]
fn collector_sink_routes_schema_events() {
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let callback = {
        let recorded = Arc::clone(&recorded);
        EventCallback::new(move |event| recorded.lock().unwrap().push(event))
    };
    let context = Context::<Simulator>::try_new(callback).unwrap();
    let engine_id = Uuid::now_v7();
    let event = Event::new(
        engine_id,
        42,
        EngineEvent::Init {
            implementation: EngineImplementationAttributes {
                name: Some("test".to_owned()),
                version: None,
                custom_attributes: DynamicAttributes::default(),
            },
            instance_name: Some("engine".to_owned()),
        },
    );

    context
        .ingest(EngineEvent::NAME, &serialize_event(&event).unwrap())
        .unwrap();
    drop(context);

    let mut recorded = recorded.lock().unwrap();
    let event = recorded.pop().unwrap();
    assert_eq!(event.id, engine_id);
    assert_eq!(event.timestamp, 42);
    assert!(matches!(
        event.data,
        SimulatorEvent::Engine(EngineEvent::Init {
            instance_name: Some(ref name),
            ..
        }) if name == "engine"
    ));
    assert!(recorded.is_empty());
}
