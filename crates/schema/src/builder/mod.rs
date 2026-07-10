// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Builders for [`Schema`] and its elements.

use std::fmt::Display;
use std::hash::Hash;

use thiserror::Error;

use crate::schema::Map;
use crate::{Annotations, Entity, Identifier, Record, Schema};

pub mod annotations;
pub mod entity;
pub mod event;
pub mod record;

pub use annotations::AnnotationsBuilder;
pub use entity::EntityBuilder;
pub use event::EventBuilder;
pub use record::RecordBuilder;

/// Error returned while assembling a schema element.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BuilderError {
    /// A name was added more than once within the same collection.
    #[error("duplicate name \"{0}\"")]
    DuplicateName(String),
    /// A name was empty.
    #[error("name must not be empty")]
    EmptyName,
}

pub(crate) fn insert_unique<K, V>(map: &mut Map<K, V>, key: K, value: V) -> Result<(), BuilderError>
where
    K: Eq + Hash + Display,
{
    match map.entry(key) {
        indexmap::map::Entry::Occupied(entry) => {
            Err(BuilderError::DuplicateName(entry.key().to_string()))
        }
        indexmap::map::Entry::Vacant(entry) => {
            entry.insert(value);
            Ok(())
        }
    }
}

/// Builder for a [`Schema`].
pub struct SchemaBuilder {
    name: Identifier,
    entities: Map<Identifier, Entity>,
    records: Map<Identifier, Record>,
    annotations: Annotations,
}

impl SchemaBuilder {
    /// Start a schema named `name`.
    pub fn new(name: Identifier) -> Self {
        Self {
            name,
            entities: Map::default(),
            records: Map::default(),
            annotations: Annotations::default(),
        }
    }

    /// Add an entity. Errors if its name is already declared.
    pub fn entity(mut self, entity: Entity) -> Result<Self, BuilderError> {
        insert_unique(&mut self.entities, entity.name().clone(), entity)?;
        Ok(self)
    }

    /// Add several entities. Errors on the first duplicate name.
    pub fn entities(
        mut self,
        entities: impl IntoIterator<Item = Entity>,
    ) -> Result<Self, BuilderError> {
        for entity in entities {
            self = self.entity(entity)?;
        }
        Ok(self)
    }

    /// Add a record. Errors if its name is already declared.
    pub fn record(mut self, record: Record) -> Result<Self, BuilderError> {
        insert_unique(&mut self.records, record.name().clone(), record)?;
        Ok(self)
    }

    /// Add several records. Errors on the first duplicate name.
    pub fn records(
        mut self,
        records: impl IntoIterator<Item = Record>,
    ) -> Result<Self, BuilderError> {
        for record in records {
            self = self.record(record)?;
        }
        Ok(self)
    }

    /// Set the schema's annotations.
    pub fn annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = annotations;
        self
    }

    /// Finish building the schema.
    pub fn build(self) -> Schema {
        Schema::from_parts(self.name, self.entities, self.records, self.annotations)
    }
}
