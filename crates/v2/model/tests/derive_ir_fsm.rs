// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model::entity::Entity as _;
use quent_v2_model_ir::{
    data_type::DataType,
    entity::Entity,
    event::{Cardinality, EntityRefTarget, Event, EventField, EventFieldType},
    qualifications::{
        Qualification,
        fsm::{Fsm, State, Transition},
    },
};

use source::fsms::*;

use crate::utils::ident;

mod source;
mod utils;

#[test]
fn one_unit() {
    assert_eq!(
        OneUnit::ir(),
        Entity::new(
            ident("OneUnit"),
            vec![Event::new(ident("A"), Cardinality::Once, vec![])],
            vec![Qualification::Fsm(Fsm {
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
            })],
            utils::rust_path!("source::fsms::OneUnit"),
        )
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
        Entity::new(
            ident("MultiUnit"),
            vec![
                Event::new(ident("A"), Cardinality::Once, vec![]),
                Event::new(ident("B"), Cardinality::Once, vec![]),
                Event::new(ident("C"), Cardinality::Once, vec![]),
            ],
            vec![Qualification::Fsm(Fsm {
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
            })],
            utils::rust_path!("source::fsms::MultiUnit"),
        )
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
        Entity::new(
            ident("OneAttribs"),
            vec![Event::new(
                ident("A"),
                Cardinality::Once,
                vec![EventField::from_type(EventFieldType::Payload(
                    DataType::Record(ident("OnePrim")),
                ))],
            )],
            vec![Qualification::Fsm(Fsm {
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
            })],
            utils::rust_path!("source::fsms::OneAttribs"),
        )
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
        Entity::new(
            ident("MultiAttribs"),
            vec![
                Event::new(
                    ident("A"),
                    Cardinality::Once,
                    vec![EventField::from_type(EventFieldType::Payload(
                        DataType::Record(ident("OnePrim")),
                    ))],
                ),
                Event::new(
                    ident("B"),
                    Cardinality::Once,
                    vec![EventField::from_type(EventFieldType::Payload(
                        DataType::Record(ident("MultiNested")),
                    ))],
                ),
            ],
            vec![Qualification::Fsm(Fsm {
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
            })],
            utils::rust_path!("source::fsms::MultiAttribs"),
        )
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
        Entity::new(
            ident("SelfLoop"),
            vec![Event::new(ident("A"), Cardinality::Once, vec![])],
            vec![Qualification::Fsm(Fsm {
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
            })],
            utils::rust_path!("source::fsms::SelfLoop"),
        )
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
        Entity::new(
            ident("Loop"),
            vec![
                Event::new(ident("A"), Cardinality::Once, vec![]),
                Event::new(ident("B"), Cardinality::Once, vec![]),
                Event::new(ident("C"), Cardinality::Once, vec![]),
            ],
            vec![Qualification::Fsm(Fsm {
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
            })],
            utils::rust_path!("source::fsms::Loop"),
        )
    );
    assert_eq!(
        Loop::ir_ref_target(),
        EntityRefTarget::Specific(ident("Loop"))
    );
}
