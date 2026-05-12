// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::entity::{EntityDeclaration, EntityHandle, Event, ObserverError};
use quent_time::timestamp;
use quent_v2_model_macros::Fsm;
use std::marker::PhantomData;
use std::sync::atomic::AtomicU16;
use uuid::Uuid;

// Considerations:
//
// While theoretically we could first generate a #[derive(Entity)] from a
// #[derive(Fsm)], it would be harder to generate FSM entity instrumentation
// APIs with the type-state pattern from there, so #[derive(Fsm)] will not take
// that approach, but we should figure out what functionaltiy between those two
// derives overlaps and deduplicate any logic.
//
// To compile a set of states an Fsm can be in, I've considered declaring a
// struct where each field is the state name and the field type are the
// attribute types. However, I find the enum style more compelling since an FSM
// is always in exactly one state at any moment, which naturally translates to a
// sum type.
//
// Since all transitions are compile-time validated for correctness, as far as
// possible sequences defined by the FSMs topology is allowed, any errors the
// transition event calls return are going to be sender channel related. There
// is no recovery from these errors, so FSM handles are dropped. Future work can
// consider returning the handle in some erroneous state.

// Type representing the FSM transition event payload.
//
// Every FSM transition will be accompanied by a sequence number. The order of
// transitions can be determined by looking at the sequence number because most
// clock sources are guaranteed to be monotonically increasing, but there is no
// guarantee two subsequent events can never get same timestamp (although it is
// very unlikely).
pub struct Transition<T> {
    // If this ever wraps in case an FSM goes through over u16::MAX (65535)
    // state transitions, we should panic, so clients can let us know this needs
    // to be increased.
    pub sequence_number: u16,
    pub payload: T,
}
