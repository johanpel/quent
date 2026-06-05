// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use rustc_hash::FxHashMap as HashMap;

use crate::{
    Annotations, Constraint as SchemaConstraint, Entity, Event, Field, Identifier, Metadata,
    Record, Schema,
};

/// Reason a [`IndexedSchema`] lookup failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupError {
    /// No element is declared under the queried name.
    Missing(String),
    /// The name is declared more than once in its scope, so the lookup is
    /// ambiguous.
    Duplicate(String),
}

impl std::fmt::Display for LookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "unknown name \"{name}\""),
            Self::Duplicate(name) => write!(f, "name \"{name}\" is declared more than once"),
        }
    }
}

impl std::error::Error for LookupError {}

/// An index of a [`Schema`] to help look things up by name.
pub struct IndexedSchema<'s> {
    schema: &'s Schema,
    annotations: IndexedAnnotations<'s>,
    entities: HashMap<&'s Identifier, Result<IndexedEntity<'s>, LookupError>>,
    records: HashMap<&'s Identifier, Result<IndexedRecord<'s>, LookupError>>,
    duplicates: Vec<String>,
}

impl<'s> IndexedSchema<'s> {
    /// Indexes every named element of `schema`, recording the path to every
    /// duplicated key along the way.
    pub fn new(schema: &'s Schema) -> Self {
        let path = schema.name.to_string();
        let mut duplicates = Vec::new();
        let annotations = IndexedAnnotations::new(&schema.annotations, &path, &mut duplicates);
        let entities = index_by(
            &schema.entities,
            |e| &e.name,
            &path,
            &mut duplicates,
            IndexedEntity::new,
        );
        let records = index_by(
            &schema.records,
            |r| &r.name,
            &path,
            &mut duplicates,
            IndexedRecord::new,
        );
        duplicates.sort_unstable();
        Self {
            schema,
            annotations,
            entities,
            records,
            duplicates,
        }
    }

    /// The indexed schema.
    pub fn schema(&self) -> &'s Schema {
        self.schema
    }

    /// The schema's own annotations.
    pub fn annotations(&self) -> &IndexedAnnotations<'s> {
        &self.annotations
    }

    /// The entity declared under `name`.
    pub fn entity(&self, name: &Identifier) -> Result<&IndexedEntity<'s>, LookupError> {
        index_get(&self.entities, name)
    }

    /// The record declared under `name`.
    pub fn record(&self, name: &Identifier) -> Result<&IndexedRecord<'s>, LookupError> {
        index_get(&self.records, name)
    }

    /// The path to every name declared more than once in its scope — across
    /// entities, records, an entity's events, an event's/record's fields, and
    /// the constraint and metadata names on any element's annotations.
    ///
    /// Computed while indexing; this is just the result, sorted.
    pub fn duplicate_paths(&self) -> &[String] {
        &self.duplicates
    }
}

/// An indexed [`Entity`] and its named events.
pub struct IndexedEntity<'s> {
    entity: &'s Entity,
    annotations: IndexedAnnotations<'s>,
    events: HashMap<&'s Identifier, Result<IndexedEvent<'s>, LookupError>>,
}

impl<'s> IndexedEntity<'s> {
    fn new(entity: &'s Entity, path: &str, duplicates: &mut Vec<String>) -> Self {
        Self {
            entity,
            annotations: IndexedAnnotations::new(&entity.annotations, path, duplicates),
            events: index_by(
                &entity.events,
                |e| &e.name,
                path,
                duplicates,
                IndexedEvent::new,
            ),
        }
    }

    /// The underlying entity.
    pub fn entity(&self) -> &'s Entity {
        self.entity
    }

    /// The entity's annotations.
    pub fn annotations(&self) -> &IndexedAnnotations<'s> {
        &self.annotations
    }

    /// The event declared under `name`.
    pub fn event(&self, name: &Identifier) -> Result<&IndexedEvent<'s>, LookupError> {
        index_get(&self.events, name)
    }
}

/// An indexed [`Event`] and its named fields.
pub struct IndexedEvent<'s> {
    event: &'s Event,
    annotations: IndexedAnnotations<'s>,
    fields: HashMap<&'s Identifier, Result<IndexedField<'s>, LookupError>>,
}

impl<'s> IndexedEvent<'s> {
    fn new(event: &'s Event, path: &str, duplicates: &mut Vec<String>) -> Self {
        Self {
            event,
            annotations: IndexedAnnotations::new(&event.annotations, path, duplicates),
            fields: index_by(
                &event.payload,
                |f| &f.name,
                path,
                duplicates,
                IndexedField::new,
            ),
        }
    }

    /// The underlying event.
    pub fn event(&self) -> &'s Event {
        self.event
    }

    /// The event's annotations.
    pub fn annotations(&self) -> &IndexedAnnotations<'s> {
        &self.annotations
    }

    /// The field declared under `name`.
    pub fn field(&self, name: &Identifier) -> Result<&IndexedField<'s>, LookupError> {
        index_get(&self.fields, name)
    }
}

/// An indexed [`Record`] and its named fields.
pub struct IndexedRecord<'s> {
    record: &'s Record,
    annotations: IndexedAnnotations<'s>,
    fields: HashMap<&'s Identifier, Result<IndexedField<'s>, LookupError>>,
}

impl<'s> IndexedRecord<'s> {
    fn new(record: &'s Record, path: &str, duplicates: &mut Vec<String>) -> Self {
        Self {
            record,
            annotations: IndexedAnnotations::new(&record.annotations, path, duplicates),
            fields: index_by(
                &record.fields,
                |f| &f.name,
                path,
                duplicates,
                IndexedField::new,
            ),
        }
    }

    /// The underlying record.
    pub fn record(&self) -> &'s Record {
        self.record
    }

    /// The record's annotations.
    pub fn annotations(&self) -> &IndexedAnnotations<'s> {
        &self.annotations
    }

    /// The field declared under `name`.
    pub fn field(&self, name: &Identifier) -> Result<&IndexedField<'s>, LookupError> {
        index_get(&self.fields, name)
    }
}

/// An indexed [`Field`].
pub struct IndexedField<'s> {
    field: &'s Field,
    annotations: IndexedAnnotations<'s>,
}

impl<'s> IndexedField<'s> {
    fn new(field: &'s Field, path: &str, duplicates: &mut Vec<String>) -> Self {
        Self {
            field,
            annotations: IndexedAnnotations::new(&field.annotations, path, duplicates),
        }
    }

    /// The underlying field.
    pub fn field(&self) -> &'s Field {
        self.field
    }

    /// The field's annotations.
    pub fn annotations(&self) -> &IndexedAnnotations<'s> {
        &self.annotations
    }
}

/// Annotations indexed by their name.
pub struct IndexedAnnotations<'s> {
    constraints: HashMap<&'s str, Result<&'s SchemaConstraint, LookupError>>,
    metadata: HashMap<&'s str, Result<&'s Metadata, LookupError>>,
}

impl<'s> IndexedAnnotations<'s> {
    fn new(annotations: &'s Annotations, path: &str, duplicates: &mut Vec<String>) -> Self {
        Self {
            constraints: index_named(
                annotations.constraints.iter().map(|c| (c.name.as_str(), c)),
                path,
                duplicates,
            ),
            metadata: index_named(
                annotations.metadata.iter().map(|m| (m.name.as_str(), m)),
                path,
                duplicates,
            ),
        }
    }

    /// The constraint named `name`.
    pub fn constraint(&self, name: &str) -> Result<&'s SchemaConstraint, LookupError> {
        index_get_str(&self.constraints, name)
    }

    /// The metadata entry named `name`.
    pub fn metadata(&self, name: &str) -> Result<&'s Metadata, LookupError> {
        index_get_str(&self.metadata, name)
    }
}

fn index_get<'a, V>(
    map: &'a HashMap<&Identifier, Result<V, LookupError>>,
    name: &Identifier,
) -> Result<&'a V, LookupError> {
    match map.get(name) {
        Some(Ok(value)) => Ok(value),
        Some(Err(error)) => Err(error.clone()),
        None => Err(LookupError::Missing(name.to_string())),
    }
}

fn index_get_str<V: Copy>(
    map: &HashMap<&str, Result<V, LookupError>>,
    name: &str,
) -> Result<V, LookupError> {
    match map.get(name) {
        Some(Ok(value)) => Ok(*value),
        Some(Err(error)) => Err(error.clone()),
        None => Err(LookupError::Missing(name.to_string())),
    }
}

/// Index `items` by an [`Identifier`] key, building each value through `value`
/// and recording the path to every duplicated key under `parent`. The first
/// occurrence of a name keeps its value; a repeat collapses the slot to
/// [`LookupError::Duplicate`].
fn index_by<'s, T, V>(
    items: &'s [T],
    key: impl Fn(&'s T) -> &'s Identifier,
    parent: &str,
    duplicates: &mut Vec<String>,
    value: impl Fn(&'s T, &str, &mut Vec<String>) -> V,
) -> HashMap<&'s Identifier, Result<V, LookupError>> {
    let mut map: HashMap<&'s Identifier, Result<V, LookupError>> = HashMap::default();
    for item in items {
        let name = key(item);
        let path = format!("{parent}.{name}");
        if let Some(slot) = map.get_mut(name) {
            if slot.is_ok() {
                *slot = Err(LookupError::Duplicate(name.to_string()));
                duplicates.push(path);
            }
        } else {
            let value = value(item, &path, duplicates);
            map.insert(name, Ok(value));
        }
    }
    map
}

/// Index `items` by a string key, recording the path to every duplicated name
/// under `parent` as an annotation key. Mirrors [`index_by`] for the flat
/// constraint and metadata name-spaces.
fn index_named<'s, V>(
    items: impl Iterator<Item = (&'s str, V)>,
    parent: &str,
    duplicates: &mut Vec<String>,
) -> HashMap<&'s str, Result<V, LookupError>> {
    let mut map: HashMap<&'s str, Result<V, LookupError>> = HashMap::default();
    for (name, value) in items {
        if let Some(slot) = map.get_mut(name) {
            if slot.is_ok() {
                *slot = Err(LookupError::Duplicate(name.to_string()));
                duplicates.push(format!("{parent}.Annotations.{name}"));
            }
        } else {
            map.insert(name, Ok(value));
        }
    }
    map
}
