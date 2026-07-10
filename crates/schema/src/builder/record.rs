// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::builder::{BuilderError, insert_unique};
use crate::schema::Map;
use crate::{Annotations, Field, Identifier, Record};

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
