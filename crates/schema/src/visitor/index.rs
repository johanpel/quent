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
    Missing(Identifier),
    /// The name is declared more than once in its scope, so the lookup is
    /// ambiguous.
    Duplicate(Identifier),
}

impl std::fmt::Display for LookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "unknown identifier \"{name}\""),
            Self::Duplicate(name) => write!(
                f,
                "schema element with identifier \"{name}\" is declared more than once"
            ),
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
}

impl<'s> IndexedSchema<'s> {
    /// Indexes every named element of `schema`.
    pub fn new(schema: &'s Schema) -> Self {
        Self {
            schema,
            annotations: IndexedAnnotations::new(&schema.annotations),
            entities: index_by(&schema.entities, |e| &e.name, IndexedEntity::new),
            records: index_by(&schema.records, |r| &r.name, IndexedRecord::new),
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
}

/// An indexed [`Entity`] and its named events.
pub struct IndexedEntity<'s> {
    entity: &'s Entity,
    annotations: IndexedAnnotations<'s>,
    events: HashMap<&'s Identifier, Result<IndexedEvent<'s>, LookupError>>,
}

impl<'s> IndexedEntity<'s> {
    fn new(entity: &'s Entity) -> Self {
        Self {
            entity,
            annotations: IndexedAnnotations::new(&entity.annotations),
            events: index_by(&entity.events, |e| &e.name, IndexedEvent::new),
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
    fn new(event: &'s Event) -> Self {
        Self {
            event,
            annotations: IndexedAnnotations::new(&event.annotations),
            fields: index_by(&event.payload, |f| &f.name, IndexedField::new),
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
    fn new(record: &'s Record) -> Self {
        Self {
            record,
            annotations: IndexedAnnotations::new(&record.annotations),
            fields: index_by(&record.fields, |f| &f.name, IndexedField::new),
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
    fn new(field: &'s Field) -> Self {
        Self {
            field,
            annotations: IndexedAnnotations::new(&field.annotations),
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
    constraints: HashMap<&'s str, &'s SchemaConstraint>,
    metadata: HashMap<&'s str, &'s Metadata>,
}

impl<'s> IndexedAnnotations<'s> {
    fn new(annotations: &'s Annotations) -> Self {
        Self {
            constraints: annotations
                .constraints
                .iter()
                .map(|c| (c.name.as_str(), c))
                .collect(),
            metadata: annotations
                .metadata
                .iter()
                .map(|m| (m.name.as_str(), m))
                .collect(),
        }
    }

    /// The constraint named `name`, if present.
    pub fn constraint(&self, name: &str) -> Option<&'s SchemaConstraint> {
        self.constraints.get(name).copied()
    }

    /// The metadata entry named `name`, if present.
    pub fn metadata(&self, name: &str) -> Option<&'s Metadata> {
        self.metadata.get(name).copied()
    }
}

fn index_get<'a, V>(
    map: &'a HashMap<&Identifier, Result<V, LookupError>>,
    name: &Identifier,
) -> Result<&'a V, LookupError> {
    match map.get(name) {
        Some(Ok(value)) => Ok(value),
        Some(Err(error)) => Err(error.clone()),
        None => Err(LookupError::Missing(name.clone())),
    }
}

fn index_by<'s, T, V>(
    items: &'s [T],
    key: impl Fn(&'s T) -> &'s Identifier,
    value: impl Fn(&'s T) -> V,
) -> HashMap<&'s Identifier, Result<V, LookupError>> {
    let mut map = HashMap::default();
    for item in items {
        let name = key(item);
        map.entry(name)
            .and_modify(|slot| *slot = Err(LookupError::Duplicate(name.clone())))
            .or_insert_with(|| Ok(value(item)));
    }
    map
}
