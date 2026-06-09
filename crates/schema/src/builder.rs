// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Builders for [`Schema`] and its elements.

use std::fmt::Display;
use std::hash::Hash;

use thiserror::Error;

use crate::schema::Map;
use crate::{
    Annotations, Cardinality, Constraint, Entity, Event, Field, Identifier, Metadata, Record,
    Schema,
};

/// Error returned while assembling a schema element.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BuilderError {
    /// A name was added more than once within the same collection.
    #[error("duplicate name \"{0}\"")]
    DuplicateName(String),
}

fn insert_unique<K, V>(map: &mut Map<K, V>, key: K, value: V) -> Result<(), BuilderError>
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

/// Builder for an [`Entity`].
pub struct EntityBuilder {
    name: Identifier,
    events: Map<Identifier, Event>,
    annotations: Annotations,
}

impl EntityBuilder {
    /// Start an entity named `name`.
    pub fn new(name: Identifier) -> Self {
        Self {
            name,
            events: Map::default(),
            annotations: Annotations::default(),
        }
    }

    /// Add an event. Errors if its name is already declared.
    pub fn event(mut self, event: Event) -> Result<Self, BuilderError> {
        insert_unique(&mut self.events, event.name().clone(), event)?;
        Ok(self)
    }

    /// Add several events. Errors on the first duplicate name.
    pub fn events(mut self, events: impl IntoIterator<Item = Event>) -> Result<Self, BuilderError> {
        for event in events {
            self = self.event(event)?;
        }
        Ok(self)
    }

    /// Set the entity's annotations.
    pub fn annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = annotations;
        self
    }

    /// Finish building the entity.
    pub fn build(self) -> Entity {
        Entity::from_parts(self.name, self.events, self.annotations)
    }
}

/// Builder for an [`Event`].
pub struct EventBuilder {
    name: Identifier,
    cardinality: Cardinality,
    payload: Map<Identifier, Field>,
    annotations: Annotations,
}

impl EventBuilder {
    /// Start an event named `name` with the given `cardinality`.
    pub fn new(name: Identifier, cardinality: Cardinality) -> Self {
        Self {
            name,
            cardinality,
            payload: Map::default(),
            annotations: Annotations::default(),
        }
    }

    /// Add a payload field. Errors if its name is already declared.
    pub fn field(mut self, field: Field) -> Result<Self, BuilderError> {
        insert_unique(&mut self.payload, field.name().clone(), field)?;
        Ok(self)
    }

    /// Add several payload fields. Errors on the first duplicate name.
    pub fn fields(mut self, fields: impl IntoIterator<Item = Field>) -> Result<Self, BuilderError> {
        for field in fields {
            self = self.field(field)?;
        }
        Ok(self)
    }

    /// Set the event's annotations.
    pub fn annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = annotations;
        self
    }

    /// Finish building the event.
    pub fn build(self) -> Event {
        Event::from_parts(self.name, self.cardinality, self.payload, self.annotations)
    }
}

/// Builder for a [`Record`].
pub struct RecordBuilder {
    name: Identifier,
    fields: Map<Identifier, Field>,
    annotations: Annotations,
}

impl RecordBuilder {
    /// Start a record named `name`.
    pub fn new(name: Identifier) -> Self {
        Self {
            name,
            fields: Map::default(),
            annotations: Annotations::default(),
        }
    }

    /// Add a field. Errors if its name is already declared.
    pub fn field(mut self, field: Field) -> Result<Self, BuilderError> {
        insert_unique(&mut self.fields, field.name().clone(), field)?;
        Ok(self)
    }

    /// Add several fields. Errors on the first duplicate name.
    pub fn fields(mut self, fields: impl IntoIterator<Item = Field>) -> Result<Self, BuilderError> {
        for field in fields {
            self = self.field(field)?;
        }
        Ok(self)
    }

    /// Set the record's annotations.
    pub fn annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = annotations;
        self
    }

    /// Finish building the record.
    pub fn build(self) -> Record {
        Record::from_parts(self.name, self.fields, self.annotations)
    }
}

/// Builder for [`Annotations`].
#[derive(Default)]
pub struct AnnotationsBuilder {
    docs: Option<String>,
    constraints: Map<String, Constraint>,
    metadata: Map<String, Metadata>,
}

impl AnnotationsBuilder {
    /// Start with empty annotations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the documentation string.
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }

    /// Add a constraint. Errors if its name is already declared.
    pub fn constraint(mut self, constraint: Constraint) -> Result<Self, BuilderError> {
        insert_unique(
            &mut self.constraints,
            constraint.name().to_string(),
            constraint,
        )?;
        Ok(self)
    }

    /// Add a metadata entry. Errors if its name is already declared.
    pub fn metadata(mut self, metadata: Metadata) -> Result<Self, BuilderError> {
        insert_unique(&mut self.metadata, metadata.name().to_string(), metadata)?;
        Ok(self)
    }

    /// Finish building the annotations.
    pub fn build(self) -> Annotations {
        Annotations::from_parts(self.docs, self.constraints, self.metadata)
    }
}
