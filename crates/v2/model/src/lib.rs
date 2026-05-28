// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! TODO(johanpel): maybe make this the top-level crate
//!
//! Rust's powerful type system is used to enforce as many constraints as
//! possible, as long as the model declaration stays compact and straightforward
//! for users. Some constraints could in principle be encoded in types too, but
//! they would be very awkward to encode (e.g. FSM state reachability). Since
//! the IR is necessary for cross-language code generation, the IR validator
//! checks these constraints instead, even in a pure Rust flow (see below).
//! This is an opinionated trade-off between Rust-purity, ease of encoding the
//! modeling constraints, and ease of declaring an application model.
//! Constraints may migrate to the type system over time, because in an ideal
//! world, the Rust compiler would validate everything.

pub mod data_type;
pub mod entity;
pub mod entity_ref;
pub mod event;
pub mod fsm;
pub mod record;

pub use entity::{Entity, EntityHandle, ObserverError};
pub use entity_ref::EntityRef;

pub use quent_exporter as exporter;
pub use quent_v2_model_macros::{Entity, Record};
