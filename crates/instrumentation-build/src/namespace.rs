// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_schema::{Entity, Identifier, Record, Schema};

/// A tree of Rust namespaces that preserves each entity's schema-order index.
///
/// Entity indices select slots in the generated `Model::Observers` tuple and
/// must not be renumbered when entities are distributed across namespaces.
pub(crate) struct Namespace<'schema> {
    path: Vec<Identifier>,
    records: Vec<&'schema Record>,
    entities: Vec<(usize, &'schema Entity)>,
    children: Vec<Self>,
}

impl<'schema> Namespace<'schema> {
    pub(crate) fn root(schema: &'schema Schema) -> Self {
        let mut root = Self::new(Vec::new());
        for record in schema.records() {
            root.namespace_mut(record.path().namespace())
                .records
                .push(record);
        }
        for (index, entity) in schema.entities().enumerate() {
            root.namespace_mut(entity.path().namespace())
                .entities
                .push((index, entity));
        }
        root
    }

    pub(crate) fn path(&self) -> &[Identifier] {
        &self.path
    }

    pub(crate) fn records(&self) -> &[&'schema Record] {
        &self.records
    }

    pub(crate) fn entities(&self) -> &[(usize, &'schema Entity)] {
        &self.entities
    }

    pub(crate) fn children(&self) -> &[Self] {
        &self.children
    }

    pub(crate) fn has_entities(&self) -> bool {
        !self.entities.is_empty() || self.children.iter().any(Self::has_entities)
    }

    fn new(path: Vec<Identifier>) -> Self {
        Self {
            path,
            records: Vec::new(),
            entities: Vec::new(),
            children: Vec::new(),
        }
    }

    fn namespace_mut(&mut self, path: &[Identifier]) -> &mut Self {
        let mut namespace = self;
        for segment in path {
            let index = match namespace
                .children
                .iter()
                .position(|child| child.path.last() == Some(segment))
            {
                Some(index) => index,
                None => {
                    let mut child_path = namespace.path.clone();
                    child_path.push(segment.clone());
                    namespace.children.push(Self::new(child_path));
                    namespace.children.len() - 1
                }
            };
            namespace = &mut namespace.children[index];
        }
        namespace
    }
}
