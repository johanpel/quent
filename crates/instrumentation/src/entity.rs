// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Entity markers used by generated instrumentation libraries.

use std::sync::Arc;

use quent_events::EntityEvent;

use crate::{EventHandle, EventPipeline};

/// Associates a generated entity marker with its event type and context.
pub trait Entity: Sized {
    /// Events emitted for this entity.
    type Event: EntityEvent;

    /// Instrumentation context containing this entity.
    type Context;

    /// Generated handle for this entity.
    type Handle: From<HandleInner<Self>>;

    /// Returns this entity's shared observer from `context`.
    ///
    /// Repeated calls for the same context must clone the same observer.
    ///
    /// This is hidden because callers obtain observers through
    /// [`Context::observer`](crate::Context::observer).
    #[doc(hidden)]
    fn observer(context: &Self::Context) -> Arc<EventPipeline<Self::Event>>;
}

/// Provides handles for an entity type through its shared event observer.
pub struct ObserverInner<E: Entity> {
    inner: Arc<EventPipeline<E::Event>>,
}

impl<E: Entity> Clone for ObserverInner<E> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<E: Entity> ObserverInner<E> {
    pub(crate) fn new(inner: Arc<EventPipeline<E::Event>>) -> Self {
        Self { inner }
    }

    /// Creates a handle for a fresh entity instance.
    pub fn handle(&self) -> E::Handle {
        HandleInner::new(EventHandle::new(Arc::clone(&self.inner))).into()
    }

    /// Creates a handle for the entity instance identified by `id`.
    pub fn handle_with_id(&self, id: crate::Uuid) -> E::Handle {
        HandleInner::new(EventHandle::with_id(id, Arc::clone(&self.inner))).into()
    }
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
    inner: EventHandle<E::Event>,
}

impl<E: Entity> HandleInner<E> {
    fn new(inner: EventHandle<E::Event>) -> Self {
        Self { inner }
    }

    /// Returns the entity instance ID.
    pub fn uuid(&self) -> crate::Uuid {
        self.inner.id()
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
        self.inner.emit(event);
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
    ) -> Result<(), crate::HandleError> {
        self.inner.emit_once::<INDEX>(event_name, event)
    }

    /// Returns whether the bit at `INDEX` has been set.
    ///
    /// This is hidden because generated once-event methods expose named checks.
    #[doc(hidden)]
    pub fn is_emitted<const INDEX: u32>(&self) -> bool {
        self.inner.is_emitted::<INDEX>()
    }
}
