// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Type definitions of entity events.

mod entity_ref;

use quent_time::{TimeUnixNanoSec, Timestamp, timestamp};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub use entity_ref::{AnyEntity, EntityRef};
pub use quent_build_info as build_info;
pub use quent_dynamic_attributes::DynamicAttributes;
pub use uuid::Uuid;

/// Trait for the event type of an entity.
pub trait EntityEvent {
    /// The name of the entity.
    const NAME: &'static str;
}

/// Associates an entity marker with the events emitted for that entity.
pub trait Entity: Sized {
    /// Events emitted for this entity.
    type Event: EntityEvent;
}

/// Associates a model marker with its umbrella event type and metadata.
pub trait Model: Sized + 'static {
    /// Events emitted by entities in this model.
    type Event: Send + 'static;

    /// Returns metadata describing this model.
    fn model_info() -> build_info::ModelInfo;
}

#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[derive(Debug)]
pub struct Event<T> {
    /// The ID of the entity producing this event.
    pub id: Uuid,
    /// The timestamp of the event.
    pub timestamp: TimeUnixNanoSec,
    /// The payload of the event.
    pub data: T,
}

impl<T> Event<T> {
    #[inline(always)]
    pub fn new_now(id: Uuid, data: T) -> Self {
        Self {
            id,
            timestamp: timestamp(),
            data,
        }
    }

    #[inline(always)]
    pub fn new(id: Uuid, timestamp: TimeUnixNanoSec, data: T) -> Self {
        Self {
            id,
            timestamp,
            data,
        }
    }
}

impl<T> Timestamp for Event<T> {
    fn timestamp(&self) -> TimeUnixNanoSec {
        self.timestamp
    }
}
