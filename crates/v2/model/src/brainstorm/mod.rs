// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Nesting the examples / exploration the following way:
//
// mod entity_name {                // name of the entity
//   mod model { ... }              // the model declaration, the only thing the user writes, the things below are derived/generated
//   mod desuagered { ... }         // things that are desugared into using core / primitive concepts
//   mod events { ... }             // the generated event types for the model component
//   mod instrumentation { ... }    // the generated instrumentation api for the model component
//   mod usage { ... }              // instrumentation usage example
//   mod analyzer { ... }           // the generated analyzer api for the model component. Future work typically so commented out
// }
//
// Generated code does not need to follow this pattern, this is just for sanity.
mod entity;
mod fsm;
mod fsm_using_resource;
mod qe_rewrite;
mod resource;
mod resource_group;

use super::*;
use crate::ObserverError;
use quent_events::Event;
use quent_time::timestamp;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU8, AtomicU16};
use uuid::Uuid;
