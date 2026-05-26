// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{IrError, identifier::Identifier};

/// IR of an FSM state
#[derive(Debug, PartialEq, Eq)]
pub enum State {
    /// Special state from which FSMs transition to comes into existence.
    Entry,
    /// Special state to which FSMs transition to go out of existence.
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

/// IR of the FSM [`super::Qualification`].
#[derive(Debug, PartialEq, Eq)]
pub struct Fsm {
    /// The state transition topology of the FSM
    pub transitions: Vec<Transition>,
}

impl Fsm {
    /// Return the name of the initial state.
    pub fn initial_state(&self) -> Option<&Identifier> {
        self.transitions
            .iter()
            .find_map(|t| match (&t.source, &t.target) {
                (State::Entry, State::State(name)) => Some(name),
                _ => None,
            })
    }
}
