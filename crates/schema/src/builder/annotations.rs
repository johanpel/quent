// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::builder::{BuilderError, insert_unique};
use crate::schema::Map;
use crate::{Annotations, Constraint, Metadata};

/// Builder for a map of named, optionally-valued string items.
#[derive(Default)]
struct OpaqueMapBuilder(Map<String, Option<String>>);

impl OpaqueMapBuilder {
    fn add(mut self, name: impl Into<String>, data: Option<String>) -> Result<Self, BuilderError> {
        let name = name.into();
        if name.is_empty() {
            return Err(BuilderError::EmptyName);
        }
        insert_unique(&mut self.0, name.clone(), data)?;
        Ok(self)
    }
}

/// Builder for [`Annotations`].
#[derive(Default)]
pub struct AnnotationsBuilder {
    docs: Option<String>,
    constraints: OpaqueMapBuilder,
    metadata: OpaqueMapBuilder,
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

    /// Add a constraint named `name`. Errors if `name` is empty or already declared.
    pub fn constraint(
        mut self,
        name: impl Into<String>,
        data: Option<String>,
    ) -> Result<Self, BuilderError> {
        self.constraints = self.constraints.add(name, data)?;
        Ok(self)
    }

    /// Add a metadata entry named `name`. Errors if `name` is empty or already declared.
    pub fn metadata(
        mut self,
        name: impl Into<String>,
        data: Option<String>,
    ) -> Result<Self, BuilderError> {
        self.metadata = self.metadata.add(name, data)?;
        Ok(self)
    }

    /// Finish building the annotations.
    pub fn build(self) -> Annotations {
        Annotations::from_parts(
            self.docs,
            self.constraints
                .0
                .into_iter()
                .map(|(k, v)| (k.clone(), Constraint::from_parts(k, v)))
                .collect(),
            self.metadata
                .0
                .into_iter()
                .map(|(k, v)| (k.clone(), Metadata::from_parts(k, v)))
                .collect(),
        )
    }
}
