// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model::entity::Entity as _;
use quent_v2_model_ir::{
    data_type::DataType,
    entity::Entity,
    event::{Cardinality, EntityRefTarget, Event, EventField, EventFieldType},
    fsm::{Fsm, State, Transition},
    identifier::Identifier,
};
use quent_v2_resource::{BoundednessData, CapacityData, CapacityKindData, ResourceData};

use source::resources::*;

use crate::utils::ident;

mod source;
mod utils;

fn linear_fsm() -> Fsm {
    Fsm {
        transitions: vec![
            Transition {
                source: State::Entry,
                target: State::State(ident("init")),
            },
            Transition {
                source: State::State(ident("init")),
                target: State::State(ident("operating")),
            },
            Transition {
                source: State::State(ident("operating")),
                target: State::State(ident("finalizing")),
            },
            Transition {
                source: State::State(ident("finalizing")),
                target: State::Exit,
            },
        ],
    }
}

fn resize_fsm() -> Fsm {
    let mut f = linear_fsm();
    f.transitions.push(Transition {
        source: State::State(ident("operating")),
        target: State::State(ident("resizing")),
    });
    f.transitions.push(Transition {
        source: State::State(ident("resizing")),
        target: State::State(ident("operating")),
    });
    f
}

fn payload_field(record_name: &str) -> EventField {
    EventField {
        name: Identifier::new_unchecked("payload"),
        docs: None,
        ty: EventFieldType::Payload(DataType::Record(ident(record_name))),
        conventions: Vec::new(),
    }
}

fn event(name: &str, cardinality: Cardinality, payload: Vec<EventField>) -> Event {
    Event {
        name: ident(name),
        docs: None,
        cardinality,
        payload,
        conventions: Vec::new(),
    }
}

fn linear_events(bound_record: &str) -> Vec<Event> {
    vec![
        event("init", Cardinality::Once, vec![payload_field(bound_record)]),
        event("operating", Cardinality::Once, vec![]),
        event("finalizing", Cardinality::Once, vec![]),
    ]
}

fn resize_events(bound_record: &str) -> Vec<Event> {
    vec![
        event("init", Cardinality::Once, vec![payload_field(bound_record)]),
        event("operating", Cardinality::Multi, vec![]),
        event("finalizing", Cardinality::Once, vec![]),
        event(
            "resizing",
            Cardinality::Multi,
            vec![payload_field(bound_record)],
        ),
    ]
}

fn single_capacity_data(kind: CapacityKindData, boundedness: BoundednessData) -> ResourceData {
    ResourceData {
        capacities: vec![CapacityData {
            name: "a".to_string(),
            kind,
            boundedness,
        }],
    }
}

/// Decode the `"Resource"` convention payload of `entity` back into a typed
/// `ResourceData` so the test can compare structurally regardless of JSON
/// whitespace.
fn extract_resource_data(entity: &Entity) -> ResourceData {
    let entry = entity
        .conventions
        .iter()
        .find(|c| c.name == "Resource")
        .expect("expected 'Resource' convention on entity");
    let raw = entry
        .data
        .as_deref()
        .expect("expected non-empty Resource convention payload");
    serde_json::from_str::<ResourceData>(raw).expect("valid Resource JSON")
}

fn assert_resource(
    actual: &Entity,
    expected_name: &str,
    expected_events: Vec<Event>,
    expected_fsm: Fsm,
    expected_data: ResourceData,
) {
    assert_eq!(actual.name, ident(expected_name));
    assert_eq!(actual.events, expected_events);
    assert_eq!(actual.fsm, Some(expected_fsm));
    assert_eq!(extract_resource_data(actual), expected_data);
    // The only convention is "Resource" — confirm no others snuck in.
    let names: Vec<&str> = actual.conventions.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["Resource"]);
}

#[test]
fn single_occupancy() {
    let ir = SingleOccupancy::ir();
    assert_resource(
        &ir,
        "SingleOccupancy",
        linear_events("OccupancyBound"),
        linear_fsm(),
        single_capacity_data(CapacityKindData::Occupancy, BoundednessData::Fixed),
    );
    assert_eq!(
        SingleOccupancy::ir_ref_target(),
        EntityRefTarget::Specific(ident("SingleOccupancy"))
    );
}

#[test]
fn single_occupancy_resize() {
    let ir = SingleOccupancyResize::ir();
    assert_resource(
        &ir,
        "SingleOccupancyResize",
        resize_events("OccupancyBound"),
        resize_fsm(),
        single_capacity_data(CapacityKindData::Occupancy, BoundednessData::Resizable),
    );
}

#[test]
fn single_occupancy_unbound() {
    let ir = SingleOccupancyUnbound::ir();
    assert_resource(
        &ir,
        "SingleOccupancyUnbound",
        linear_events("OccupancyBound"),
        linear_fsm(),
        single_capacity_data(CapacityKindData::Occupancy, BoundednessData::Unbounded),
    );
}

#[test]
fn single_rate() {
    let ir = SingleRate::ir();
    assert_resource(
        &ir,
        "SingleRate",
        linear_events("RateBound"),
        linear_fsm(),
        single_capacity_data(CapacityKindData::Rate, BoundednessData::Fixed),
    );
}

#[test]
fn single_rate_resize() {
    let ir = SingleRateResize::ir();
    assert_resource(
        &ir,
        "SingleRateResize",
        resize_events("RateBound"),
        resize_fsm(),
        single_capacity_data(CapacityKindData::Rate, BoundednessData::Resizable),
    );
}

#[test]
fn single_rate_unbound() {
    let ir = SingleRateUnbound::ir();
    assert_resource(
        &ir,
        "SingleRateUnbound",
        linear_events("RateBound"),
        linear_fsm(),
        single_capacity_data(CapacityKindData::Rate, BoundednessData::Unbounded),
    );
}

/// Verify that docstrings on a `resource!` declaration flow into the
/// synthesized entity's `docs` field.
#[test]
fn resource_docs_flow_through() {
    let ir = DocumentedResource::ir();
    assert_eq!(
        ir.docs.as_deref(),
        Some("A documented resource for testing docstring propagation.\nSecond line."),
    );
}
