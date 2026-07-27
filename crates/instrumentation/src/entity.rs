// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Entity markers used by generated instrumentation libraries.

use std::sync::Arc;

use quent_events::EntityEvent;

use crate::ObserverInner;

/// Associates a generated entity marker with its event type and context.
pub trait Entity: Sized {
    /// Events emitted for this entity.
    type Event: EntityEvent;

    /// Instrumentation context containing this entity.
    type Context;

    /// Generated handle for this entity.
    type Handle: From<HandleInner<Self>>;
}

/// Provides handles for an entity type through its shared event observer.
pub struct Observer<E: Entity> {
    inner: Arc<ObserverInner<E::Event>>,
}

impl<E: Entity> Clone for Observer<E> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<E: Entity> Observer<E> {
    /// Creates an observer backed by `inner`.
    ///
    /// This method is hidden because generated model implementations construct
    /// observers while callers obtain them through their model context.
    #[doc(hidden)]
    pub fn new(inner: Arc<ObserverInner<E::Event>>) -> Self {
        Self { inner }
    }

    /// Creates a handle for a fresh entity instance.
    pub fn handle(&self) -> E::Handle {
        HandleInner::new(Arc::clone(&self.inner)).into()
    }

    /// Creates a handle for the entity instance identified by `id`.
    pub fn handle_with_id(&self, id: crate::Uuid) -> E::Handle {
        HandleInner::with_id(id, Arc::clone(&self.inner)).into()
    }
}

/// An error from emitting through a generated entity handle.
#[derive(Debug, thiserror::Error)]
pub enum HandleError {
    /// A once-cardinality event was emitted more than once for one entity
    /// instance.
    #[error("once-event `{event}` already emitted for this entity instance")]
    OnceAlreadyEmitted {
        /// Name of the event that was re-emitted.
        event: &'static str,
    },
}

/// Common operations for generated handles.
///
/// Generated local newtypes wrap this type so they can add inherent
/// entity-specific event methods.
///
/// This type is hidden because those newtypes are the application-facing
/// handle API.
#[doc(hidden)]
pub struct HandleInner<E: Entity> {
    id: crate::Uuid,
    /// One bit per once-cardinality event, set once that event is emitted.
    once_flags: u64,
    observer: Arc<ObserverInner<E::Event>>,
}

impl<E: Entity> HandleInner<E> {
    fn new(observer: Arc<ObserverInner<E::Event>>) -> Self {
        Self::with_id(crate::Uuid::now_v7(), observer)
    }

    fn with_id(id: crate::Uuid, observer: Arc<ObserverInner<E::Event>>) -> Self {
        Self {
            id,
            once_flags: 0,
            observer,
        }
    }

    /// Returns the entity instance ID.
    pub fn uuid(&self) -> crate::Uuid {
        self.id
    }

    /// Returns a typed reference to this instance carrying no data.
    pub fn as_entity_ref(&self) -> crate::EntityRef<E> {
        crate::EntityRef::new(self.uuid(), ())
    }

    /// Returns a typed reference to this instance carrying `data`.
    pub fn as_entity_ref_with<T>(&self, data: T) -> crate::EntityRef<E, T> {
        crate::EntityRef::new(self.uuid(), data)
    }

    /// Returns an untyped reference to this instance carrying no data.
    pub fn as_any_entity_ref(&self) -> crate::EntityRef<crate::AnyEntity> {
        crate::EntityRef::new(self.uuid(), ())
    }

    /// Returns an untyped reference to this instance carrying `data`.
    pub fn as_any_entity_ref_with<T>(&self, data: T) -> crate::EntityRef<crate::AnyEntity, T> {
        crate::EntityRef::new(self.uuid(), data)
    }

    /// Emits an event without cardinality tracking.
    ///
    /// This is hidden because generated event methods provide the typed API.
    #[doc(hidden)]
    pub fn emit(&self, event: E::Event) {
        self.observer.emit(self.id, event);
    }

    /// Emits an event unless the bit at `INDEX` was previously set.
    ///
    /// This is hidden because generated once-event methods provide the typed API.
    ///
    /// # Errors
    ///
    /// Returns [`HandleError`](crate::HandleError) when the event was already emitted.
    #[doc(hidden)]
    pub fn emit_once<const INDEX: u32>(
        &mut self,
        event_name: &'static str,
        event: E::Event,
    ) -> Result<(), HandleError> {
        const { assert!(INDEX < u64::BITS, "once-event bit index out of range") };
        let mask = 1u64 << INDEX;
        if self.once_flags & mask != 0 {
            return Err(HandleError::OnceAlreadyEmitted { event: event_name });
        }
        self.once_flags |= mask;
        self.observer.emit(self.id, event);
        Ok(())
    }

    /// Returns whether the bit at `INDEX` has been set.
    ///
    /// This is hidden because generated once-event methods expose named checks.
    #[doc(hidden)]
    pub fn is_emitted<const INDEX: u32>(&self) -> bool {
        const { assert!(INDEX < u64::BITS, "once-event bit index out of range") };
        self.once_flags & (1u64 << INDEX) != 0
    }
}
