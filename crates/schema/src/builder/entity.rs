// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::builder::{BuilderError, insert_unique};
use crate::schema::Map;
use crate::{Annotations, Entity, Event, Identifier};

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
