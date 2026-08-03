// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_events::{Entity, EntityEvent, Event, Model};
use uuid::Uuid;

pub type StoreResult<T> = Result<T, StoreError>;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("context `{0}` was not found")]
    ContextNotFound(Uuid),
    #[error("context model `{actual}` does not match expected model `{expected}`")]
    ModelMismatch { expected: String, actual: String },
    #[error("context contains no non-empty recognized event streams")]
    EmptyContext,
    #[error("context contains an unsupported event format `{0}`")]
    UnsupportedFormat(String),
    #[error("context mixes event formats `{first}` and `{second}`")]
    MixedFormats { first: String, second: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Importer(#[from] quent_io_types::ImporterError),
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextQuery {
    All,
    Emitted { entity: &'static str },
}

pub trait EventStore<M: Model> {
    fn contexts(&self, query: ContextQuery) -> StoreResult<Vec<Uuid>>;

    fn import_events(
        &self,
        context_id: Uuid,
    ) -> StoreResult<Box<dyn Iterator<Item = Event<M::Event>>>>;
}

pub trait EventStoreExt<M: Model>: EventStore<M> {
    fn context_ids(&self) -> StoreResult<Vec<Uuid>> {
        self.contexts(ContextQuery::All)
    }

    fn contexts_with_events<E>(&self) -> StoreResult<Vec<Uuid>>
    where
        E: Entity,
        E::Event: Into<M::Event>,
    {
        self.contexts(ContextQuery::Emitted {
            entity: E::Event::NAME,
        })
    }
}

impl<M: Model, S: EventStore<M> + ?Sized> EventStoreExt<M> for S {}
