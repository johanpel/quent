// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{IrError, identifier::Identifier};

/// IR of an FSM state
#[derive(Debug, PartialEq, Eq)]
pub enum State {
    /// The special entry state for transitions where FSMs come into existence.
    Entry,
    /// The special exit state for transitions where FSMs go out of existence.
    Exit,
    /// A regular state.
    State(Identifier),
}

impl TryFrom<&str> for State {
    type Error = IrError;
    fn try_from(name: &str) -> Result<Self, Self::Error> {
        Ok(if name.eq_ignore_ascii_case("entry") {
            Self::Entry
        } else if name.eq_ignore_ascii_case("exit") {
            Self::Exit
        } else {
            Self::State(Identifier::try_from(name)?)
        })
    }
}

/// IR of an FSM transition
#[derive(Debug, PartialEq, Eq)]
pub struct Transition {
    /// The source state.
    pub source: State,
    /// The target state.
    pub target: State,
}

/// IR of an FSM qualification.
#[derive(Debug, PartialEq, Eq)]
pub struct Fsm {
    /// The possible transitions.
    pub transitions: Vec<Transition>,
}

impl Fsm {
    pub fn initial_state(&self) -> Option<&Identifier> {
        self.transitions
            .iter()
            .find_map(|t| match (&t.source, &t.target) {
                (State::Entry, State::State(name)) => Some(name),
                _ => None,
            })
    }
}
