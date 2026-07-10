// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::builder::{BuilderError, insert_unique};
use crate::schema::Map;
use crate::{Annotations, Cardinality, Event, Field, Identifier};

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
