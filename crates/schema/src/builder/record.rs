// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::builder::{AnnotationsBuilder, BuilderError, collect_unique};
use crate::schema::identifier::IdentifierError;
use crate::{Annotations, Field, Identifier, Record};

/// Builder for a [`Record`].
pub struct RecordBuilder {
    name: Identifier,
    fields: Vec<Field>,
    annotations: AnnotationsBuilder,
}

impl RecordBuilder {
    /// Start a record named `name`.
    pub fn new(name: Identifier) -> Self {
        Self {
            name,
            fields: Vec::new(),
            annotations: AnnotationsBuilder::new(),
        }
    }

    /// Start a record named `name`, validating `name` as an [`Identifier`].
    ///
    /// # Errors
    ///
    /// Errors if `name` is not a valid identifier.
    pub fn try_new(
        name: impl TryInto<Identifier, Error = IdentifierError>,
    ) -> Result<Self, IdentifierError> {
        Ok(Self::new(name.try_into()?))
    }

    /// Add a field, returning the builder for chaining.
    pub fn with_field(mut self, field: Field) -> Self {
        self.fields.push(field);
        self
    }

    /// Add several fields, returning the builder for chaining.
    pub fn with_fields(mut self, fields: impl IntoIterator<Item = Field>) -> Self {
        self.fields.extend(fields);
        self
    }

    /// Set the record's annotations, replacing any added so far, and return
    /// the builder for chaining.
    pub fn with_annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = AnnotationsBuilder::from_annotations(&annotations);
        self
    }

    /// Finish building the record.
    ///
    /// # Errors
    ///
    /// Errors if a field name is repeated or the annotations are invalid.
    pub fn build(self) -> Result<Record, BuilderError> {
        let Self {
            name,
            fields,
            annotations,
        } = self;
        let fields = collect_unique(fields, |field| field.name().clone())?;
        let annotations = annotations.build()?;
        Ok(Record::from_parts(name, fields, annotations))
    }
}
