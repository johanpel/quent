// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES.
// All rights reserved. SPDX-License-Identifier: Apache-2.0

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
