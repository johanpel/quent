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

use source::fsms::*;

use crate::utils::ident;

mod source;
mod utils;

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

fn entity_with_fsm(name: &str, events: Vec<Event>, fsm: Fsm) -> Entity {
    Entity {
        name: ident(name),
        docs: None,
        events,
        fsm: Some(fsm),
        conventions: Vec::new(),
    }
}

#[test]
fn one_unit() {
    assert_eq!(
        OneUnit::ir(),
        entity_with_fsm(
            "OneUnit",
            vec![event("A", Cardinality::Once, vec![])],
            Fsm {
                transitions: vec![
                    Transition {
                        source: State::Entry,
                        target: State::State(ident("A")),
                    },
                    Transition {
                        source: State::State(ident("A")),
                        target: State::Exit,
                    },
                ],
            },
        ),
    );
    assert_eq!(
        OneUnit::ir_ref_target(),
        EntityRefTarget::Specific(ident("OneUnit"))
    );
}

#[test]
fn multi_unit() {
    assert_eq!(
        MultiUnit::ir(),
        entity_with_fsm(
            "MultiUnit",
            vec![
                event("A", Cardinality::Once, vec![]),
                event("B", Cardinality::Once, vec![]),
                event("C", Cardinality::Once, vec![]),
            ],
            Fsm {
                transitions: vec![
                    Transition {
                        source: State::Entry,
                        target: State::State(ident("A")),
                    },
                    Transition {
                        source: State::State(ident("A")),
                        target: State::State(ident("B")),
                    },
                    Transition {
                        source: State::State(ident("B")),
                        target: State::State(ident("C")),
                    },
                    Transition {
                        source: State::State(ident("C")),
                        target: State::Exit,
                    },
                ],
            },
        ),
    );
    assert_eq!(
        MultiUnit::ir_ref_target(),
        EntityRefTarget::Specific(ident("MultiUnit"))
    );
}

#[test]
fn one_attribs() {
    assert_eq!(
        OneAttribs::ir(),
        entity_with_fsm(
            "OneAttribs",
            vec![event(
                "A",
                Cardinality::Once,
                vec![payload_field("OnePrim")],
            )],
            Fsm {
                transitions: vec![
                    Transition {
                        source: State::Entry,
                        target: State::State(ident("A")),
                    },
                    Transition {
                        source: State::State(ident("A")),
                        target: State::Exit,
                    },
                ],
            },
        ),
    );
    assert_eq!(
        OneAttribs::ir_ref_target(),
        EntityRefTarget::Specific(ident("OneAttribs"))
    );
}

#[test]
fn multi_attribs() {
    assert_eq!(
        MultiAttribs::ir(),
        entity_with_fsm(
            "MultiAttribs",
            vec![
                event("A", Cardinality::Once, vec![payload_field("OnePrim")],),
                event("B", Cardinality::Once, vec![payload_field("MultiNested")],),
            ],
            Fsm {
                transitions: vec![
                    Transition {
                        source: State::Entry,
                        target: State::State(ident("A")),
                    },
                    Transition {
                        source: State::State(ident("A")),
                        target: State::State(ident("B")),
                    },
                    Transition {
                        source: State::State(ident("B")),
                        target: State::Exit,
                    },
                ],
            },
        ),
    );
    assert_eq!(
        MultiAttribs::ir_ref_target(),
        EntityRefTarget::Specific(ident("MultiAttribs"))
    );
}

#[test]
fn self_loop() {
    assert_eq!(
        SelfLoop::ir(),
        entity_with_fsm(
            "SelfLoop",
            vec![event("A", Cardinality::Once, vec![])],
            Fsm {
                transitions: vec![
                    Transition {
                        source: State::Entry,
                        target: State::State(ident("A")),
                    },
                    Transition {
                        source: State::State(ident("A")),
                        target: State::State(ident("A")),
                    },
                    Transition {
                        source: State::State(ident("A")),
                        target: State::Exit,
                    },
                ],
            },
        ),
    );
    assert_eq!(
        SelfLoop::ir_ref_target(),
        EntityRefTarget::Specific(ident("SelfLoop"))
    );
}

#[test]
fn loop_() {
    assert_eq!(
        Loop::ir(),
        entity_with_fsm(
            "Loop",
            vec![
                event("A", Cardinality::Once, vec![]),
                event("B", Cardinality::Once, vec![]),
                event("C", Cardinality::Once, vec![]),
            ],
            Fsm {
                transitions: vec![
                    Transition {
                        source: State::Entry,
                        target: State::State(ident("A")),
                    },
                    Transition {
                        source: State::State(ident("A")),
                        target: State::State(ident("B")),
                    },
                    Transition {
                        source: State::State(ident("A")),
                        target: State::State(ident("C")),
                    },
                    Transition {
                        source: State::State(ident("B")),
                        target: State::State(ident("C")),
                    },
                    Transition {
                        source: State::State(ident("C")),
                        target: State::State(ident("A")),
                    },
                    Transition {
                        source: State::State(ident("A")),
                        target: State::Exit,
                    },
                ],
            },
        ),
    );
    assert_eq!(
        Loop::ir_ref_target(),
        EntityRefTarget::Specific(ident("Loop"))
    );
}
