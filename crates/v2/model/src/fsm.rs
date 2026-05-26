// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! # Finite-State-Machines (FSMs)
//!
//! An entitty can qualify as an FSM (Finite State Machine) if their order is
//! restricted by a certain topology of "states" and "transitions". This is
//! useful to trace a specific restricted lifecycle of the entity. These states
//! include the special "entry" and "exit" states, such that there is exactly
//! one transition from the "entry" state into some initial state, and from each
//! state, there exists a sequence of transitions to the "exit" state.
//!
//! Instrumentation client code emits the transition events, where both trigger
//! conditions and state outputs known at the time of transition can be
//! captured. In this sense, outputs are bound to transitions (Mealy-style). If
//! state outputs need to change while in a state, it can be modeled as a
//! self-transition that updates the state output attributes.
//!
//! Quent does not support more than one transition event per direction between
//! a source and destination state pair. Any variety of trigger conditions can
//! be captured as transition attributes.
//!
//! Quent does not support more than [`u16::MAX`] state transition events at
//! instrumentation run-time.
//!
//! ## Rust Modeling API for FSMs
//!
//! The FSM topology can be expressed by combining the `#[derive(Entity)]` with
//! the `#[quent(fsm(...))]` attribute.
//!
//! TODO(johanpel): examples
// TODO(johanpel): consider adding support for multiple transitions between
// pairs in a type-safe manner.

/// Type carrying the payload of an FSM event.
///
/// Every FSM transition will be accompanied by a sequence number. The order of
/// transitions can be determined by looking at the sequence number because most
/// clock sources are guaranteed to be monotonically increasing, but there is no
/// guarantee two subsequent events can never get same timestamp (although it is
/// very unlikely).
pub struct Transition<T> {
    /// The sequence number of this transition.
    pub sequence_number: u16,
    /// The payload of this transition.
    pub payload: T,
}

/// Trait implemented by entities that qualify as FSMs.
pub trait Fsm {
    #[cfg(feature = "ir")]
    fn ir() -> quent_v2_model_ir::qualifications::fsm::Fsm;
}
