// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::builder::{AnnotationsBuilder, BuilderError, collect_unique};
use crate::schema::identifier::IdentifierError;
use crate::{Annotations, Entity, Event, Identifier};

/// Builder for an [`Entity`].
pub struct EntityBuilder {
    name: Identifier,
    events: Vec<Event>,
    annotations: AnnotationsBuilder,
}

impl EntityBuilder {
    /// Start an entity named `name`.
    pub fn new(name: Identifier) -> Self {
        Self {
            name,
            events: Vec::new(),
            annotations: AnnotationsBuilder::new(),
        }
    }

    /// Start an entity named `name`, validating `name` as an [`Identifier`].
    ///
    /// # Errors
    ///
    /// Errors if `name` is not a valid identifier.
    pub fn try_new(
        name: impl TryInto<Identifier, Error = IdentifierError>,
    ) -> Result<Self, IdentifierError> {
        Ok(Self::new(name.try_into()?))
    }

    /// Add an event, returning the builder for chaining.
    pub fn with_event(mut self, event: Event) -> Self {
        self.events.push(event);
        self
    }

    /// Add several events, returning the builder for chaining.
    pub fn with_events(mut self, events: impl IntoIterator<Item = Event>) -> Self {
        self.events.extend(events);
        self
    }

    /// Set the entity's annotations, replacing any added so far, and return
    /// the builder for chaining.
    pub fn with_annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = AnnotationsBuilder::from_annotations(&annotations);
        self
    }

    /// Finish building the entity.
    ///
    /// # Errors
    ///
    /// Errors if the entity declares no events, an event name is repeated, or
    /// the annotations are invalid.
    pub fn build(self) -> Result<Entity, BuilderError> {
        let Self {
            name,
            events,
            annotations,
        } = self;
        if events.is_empty() {
            return Err(BuilderError::NoEvents);
        }
        let events = collect_unique(events, |event| event.name().clone())?;
        let annotations = annotations.build()?;
        Ok(Entity::from_parts(name, events, annotations))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_entity_without_events() {
        let error = EntityBuilder::try_new("E").unwrap().build().unwrap_err();
        assert_eq!(error, BuilderError::NoEvents);
    }
}
