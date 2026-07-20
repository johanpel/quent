// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_schema::{
    Annotations, DataType, Field, Identifier, Record,
    builder::{AnnotationsBuilder, BuilderError, RecordBuilder},
    schema::identifier::IdentifierError,
};
use thiserror::Error;

use crate::{Capacities, Capacity, CapacityKind, Resource};

/// The artifacts a [`ResourceBuilder`] delivers for a resource.
pub struct ResourceParts {
    /// The resource constraint definition to place on the resource entity's
    /// constraints.
    pub definition: Resource,
    /// The record type a usage of the resource is carried as.
    pub usage: Record,
    /// The record type the resource's bounds are carried as, present iff a
    /// capacity is bounded.
    pub bounds: Option<Record>,
}

/// Builds a resource's usage and bounds record types and its definition, for the
/// caller to compose the resource entity from and add to a schema.
///
/// ```
/// # use quent_resource::{CapacityKind, Resource, ResourceBuilder};
/// # use quent_schema::{
/// #     Annotations, Cardinality, DataType, Field, Identifier,
/// #     builder::{AnnotationsBuilder, EntityBuilder, EventBuilder, SchemaBuilder},
/// # };
/// // A bounded capacity, so `build` also yields a bounds record.
/// let parts = ResourceBuilder::new(Identifier::try_new("Memory")?)
///     .capacity(Identifier::try_new("bytes")?, CapacityKind::Occupancy, true)
///     .build()?;
/// let bounds = parts.bounds.ok_or("expected a bounds record")?;
///
/// // Assemble the resource entity: its `operating` event declares the bounds record.
/// let operating = EventBuilder::new(Identifier::try_new("operating")?, Cardinality::Once)
///     .try_with_field(Field::new(
///         Identifier::try_new("bounds")?,
///         DataType::Record(bounds.name().clone()),
///         Annotations::default(),
///     ))?
///     .build();
/// let definition = AnnotationsBuilder::new()
///     .try_with_constraint(Resource::NAME, Some(serde_json::to_string(&parts.definition)?))?
///     .build();
/// let entity = EntityBuilder::new(Identifier::try_new("Memory")?)
///     .try_with_event(operating)?
///     .with_annotations(definition)
///     .build();
///
/// // Add the entity and its record types to the schema.
/// let schema = SchemaBuilder::new(Identifier::try_new("App")?)
///     .try_with_entity(entity)?
///     .try_with_record(parts.usage)?
///     .try_with_record(bounds)?
///     .build();
/// # let _ = schema;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct ResourceBuilder {
    name: Identifier,
    capacities: Capacities,
}

impl ResourceBuilder {
    /// Start a resource named `name`.
    pub fn new(name: Identifier) -> Self {
        Self {
            name,
            capacities: Capacities::default(),
        }
    }

    /// Add a capacity. A later capacity with the same name replaces it.
    pub fn capacity(mut self, name: Identifier, kind: CapacityKind, bounded: bool) -> Self {
        self.capacities
            .insert(name.clone(), Capacity::new(name, kind, bounded));
        self
    }

    /// Build the definition and the usage and bounds record types.
    ///
    /// # Errors
    ///
    /// Errors if no capacity was added or generating the records or constraint data fails.
    pub fn build(self) -> Result<ResourceParts, BuildError> {
        let ResourceBuilder { name, capacities } = self;
        if capacities.is_empty() {
            return Err(BuildError::NoCapacities);
        }

        // The usage record carries a claim field for each capacity.
        let usage = role_record(
            suffixed(&name, "Usage")?,
            Resource::Usage {
                resource: name.clone(),
            },
            capacities.values().map(Capacity::name),
        )?;

        // The bounds record carries a field for each bounded capacity, if any.
        let bounds = if capacities.values().any(Capacity::bounded) {
            Some(role_record(
                suffixed(&name, "Bounds")?,
                Resource::Bounds {
                    resource: name.clone(),
                },
                capacities
                    .values()
                    .filter(|capacity| capacity.bounded())
                    .map(Capacity::name),
            )?)
        } else {
            None
        };

        Ok(ResourceParts {
            definition: Resource::Definition(capacities),
            usage,
            bounds,
        })
    }
}

/// A record carrying resource `role`, with a `U64` field for each name in
/// `fields`.
fn role_record<'a>(
    name: Identifier,
    role: Resource,
    fields: impl Iterator<Item = &'a Identifier>,
) -> Result<Record, BuildError> {
    let annotations = AnnotationsBuilder::new()
        .try_with_constraint(Resource::NAME, Some(serde_json::to_string(&role)?))?
        .build();
    let mut builder = RecordBuilder::new(name).with_annotations(annotations);
    for field in fields {
        builder = builder.try_with_field(Field::new(
            field.clone(),
            DataType::U64,
            Annotations::default(),
        ))?;
    }
    Ok(builder.build())
}

fn suffixed(resource: &Identifier, suffix: &str) -> Result<Identifier, BuildError> {
    Ok(Identifier::try_new(format!("{resource}{suffix}"))?)
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("resource must declare at least one capacity")]
    NoCapacities,
    #[error(transparent)]
    Schema(#[from] BuilderError),
    #[error(transparent)]
    Identifier(#[from] IdentifierError),
    #[error("serializing resource data: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_definition_and_records() -> Result<(), BuildError> {
        let bytes = Identifier::try_new("bytes")?;
        let usage_name = Identifier::try_new("MemoryUsage")?;
        let bounds_name = Identifier::try_new("MemoryBounds")?;

        let parts = ResourceBuilder::new(Identifier::try_new("Memory")?)
            .capacity(bytes.clone(), CapacityKind::Occupancy, true)
            .build()?;

        assert!(matches!(parts.definition, Resource::Definition(_)));
        assert_eq!(parts.usage.name(), &usage_name);
        assert!(parts.usage.field(&bytes).is_some());
        assert!(
            parts
                .bounds
                .is_some_and(|bounds| bounds.name() == &bounds_name)
        );
        Ok(())
    }

    #[test]
    fn rejects_empty_resource() {
        let result = ResourceBuilder::new(Identifier::try_new("Memory").unwrap()).build();
        assert!(matches!(result, Err(BuildError::NoCapacities)));
    }
}
