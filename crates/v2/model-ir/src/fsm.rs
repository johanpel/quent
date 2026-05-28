// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES.
// All rights reserved. SPDX-License-Identifier: Apache-2.0

use crate::identifier::Identifier;

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum State {
    /// Special state from which FSMs transition to come into existence.
    Entry,
    /// Special state to which FSMs transition to no longer exist.
    Exit,
    /// A regular state with the name of the event representing the transition
    /// into that state.
    State(Identifier),
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Transition {
    /// The source state.
    pub source: State,
    /// The target state.
    pub target: State,
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fsm {
    /// The state transition topology.
    pub transitions: Vec<Transition>,
}
