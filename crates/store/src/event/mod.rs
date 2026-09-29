// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Typed access to fully materialized model events.

use quent_events::{CombinedEventModel, EntityMarker, Event};

#[cfg(any(feature = "io-ndjson", feature = "io-msgpack", feature = "io-postcard"))]
pub mod filesystem;

/// An iterator yielding owned [`Event<T>`](Event) values or read failures.
pub type EventIterator<T, E> = Box<dyn Iterator<Item = Result<Event<T>, E>>>;

/// The result of creating an [`EventIterator`].
pub type EventIteratorResult<T, E> = Result<EventIterator<T, E>, E>;

/// Loads owned entity event payloads selected by entity marker types in model `M`.
pub trait EventStore<M> {
    /// Error returned when events cannot be loaded.
    type Error;

    /// Loads `E::Payload` payloads for entity marker `E` from selected contexts without an ordering guarantee.
    fn entity_events<E>(&self) -> EventIteratorResult<E::Payload, <Self as EventStore<M>>::Error>
    where
        E: StoredEntity<M>,
        Self: EventLoader<E, Error = <Self as EventStore<M>>::Error>,
    {
        self.load_entity_events()
    }
}

/// Loads owned model-wide events with combined event payloads.
///
/// Generated models support this trait only when
/// `quent_store_build::Options::combined_event` is enabled.
pub trait CombinedEventStore<M: CombinedEventModel>: EventStore<M> {
    /// Loads every `Event<M::CombinedEvent>` stored in selected contexts without an ordering guarantee.
    fn events(&self) -> EventIteratorResult<M::CombinedEvent, <Self as EventStore<M>>::Error>
    where
        Self: CombinedEventLoader<M, Error = <Self as EventStore<M>>::Error>,
    {
        self.load_model_events()
    }
}

/// Loads the event payload type associated with entity marker `E`.
#[doc(hidden)]
pub trait EventLoader<E: EntityMarker> {
    /// Error returned when events cannot be loaded.
    type Error;

    /// Loads `E::Payload` payloads without an ordering guarantee.
    fn load_entity_events(&self) -> EventIteratorResult<E::Payload, Self::Error>;
}

/// Loads events carrying the model's combined event payload type.
#[doc(hidden)]
pub trait CombinedEventLoader<M: CombinedEventModel> {
    /// Error returned when events cannot be loaded.
    type Error;

    /// Loads `Event<M::CombinedEvent>` values without an ordering guarantee.
    fn load_model_events(&self) -> EventIteratorResult<M::CombinedEvent, Self::Error>;
}

/// Marks an entity marker as belonging to model `M`.
///
/// # Code generation
///
/// For entity marker `Task`, whose event payload type is `TaskEvent`,
/// `quent-store-build` emits `impl StoredEntity<Demo> for Task {}`.
#[doc(hidden)]
pub trait StoredEntity<M>: EntityMarker {}
