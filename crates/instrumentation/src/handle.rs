// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Per-instance emit surface over a shared observer.

use std::sync::Arc;

use uuid::Uuid;

use crate::observer::Observer;

/// An error from emitting through a [`Handle`].
#[derive(Debug, thiserror::Error)]
pub enum ObserverError {
    /// A once-cardinality event was emitted more than once for one entity
    /// instance.
    #[error("once-event `{event}` already emitted for this entity instance")]
    OnceAlreadyEmitted {
        /// Name of the event that was re-emitted.
        event: &'static str,
    },
}

/// A handle to one entity instance: emits that instance's events through a
/// shared [`Observer`], enforcing once-cardinality events at most once.
///
/// Holds a shared reference to the observer's export pipeline, keeping it alive
/// while any handle does. Not `Clone`: the once-emit state is unique to one
/// instance, so a clone could re-emit a once-event.
#[doc(hidden)]
pub struct Handle<E> {
    id: Uuid,
    /// One bit per once-cardinality event, set once that event is emitted.
    once_flags: u64,
    observer: Arc<Observer<E>>,
}

impl<E> Handle<E> {
    /// Create a handle for a fresh entity instance, with a generated id.
    pub fn new(observer: Arc<Observer<E>>) -> Self {
        Self::with_id(Uuid::now_v7(), observer)
    }

    /// Create a handle for the entity instance identified by `id`.
    pub fn with_id(id: Uuid, observer: Arc<Observer<E>>) -> Self {
        Self {
            id,
            once_flags: 0,
            observer,
        }
    }

    /// The entity instance id this handle emits for.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Emit a multi-cardinality event for this instance.
    pub fn emit(&self, event: E) {
        self.observer.emit(self.id, event);
    }

    /// Emit a once-cardinality event, tracked by its `bit` index.
    ///
    /// Returns [`ObserverError::OnceAlreadyEmitted`] if this handle already
    /// emitted the event; otherwise records and emits it. `bit` must be below
    /// 64.
    pub fn emit_once(
        &mut self,
        bit: u32,
        event_name: &'static str,
        event: E,
    ) -> Result<(), ObserverError> {
        let mask = 1u64 << bit;
        if self.once_flags & mask != 0 {
            return Err(ObserverError::OnceAlreadyEmitted { event: event_name });
        }
        self.once_flags |= mask;
        self.observer.emit(self.id, event);
        Ok(())
    }
}
