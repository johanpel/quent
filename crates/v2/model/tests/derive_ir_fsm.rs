// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model_ir::{
    entity::{Entity, ModelEntity},
    event::{Cardinality, EntityRefTarget, Event, ModelEntityRefTarget},
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
        OneUnit::model_entity(),
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
        OneUnit::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("OneUnit"))
    );
}

#[test]
fn multi_unit() {
    assert_eq!(
        MultiUnit::model_entity(),
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
        MultiUnit::model_entity_ref_target(),
        EntityRefTarget::Specific(ident("MultiUnit"))
    );
}
