// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Resource hierarchy construction and traversal for schemas using both the Reference Tree and resource constraints.

use std::collections::VecDeque;

use uuid::Uuid;

use crate::{
    AnalyzerResult,
    resource::{Resource, collection::ResourceCollection},
};

/// An entity in a resource hierarchy, optionally marked as a resource.
pub struct ResourceTreeNode {
    pub entity_id: Uuid,
    pub is_resource: bool,
    pub children: Vec<ResourceTreeNode>,
}

impl ResourceTreeNode {
    pub fn try_new(
        resources: &impl ResourceCollection,
        root_group_id: Uuid,
    ) -> AnalyzerResult<Self> {
        let group_children = resources
            .resource_group_child_groups(root_group_id)?
            .map(|child_group| Self::try_new(resources, child_group));
        let resource_children = resources
            .resource_group_child_resources(root_group_id)?
            .map(|entity_id| {
                Ok(ResourceTreeNode {
                    entity_id,
                    is_resource: true,
                    children: Vec::new(),
                })
            });
        Ok(ResourceTreeNode {
            entity_id: root_group_id,
            is_resource: false,
            children: group_children
                .chain(resource_children)
                .collect::<AnalyzerResult<_>>()?,
        })
    }

    /// Return the IDs of all resources in this hierarchy.
    pub fn iter_resource_ids(&self) -> ResourceTreeResourceIter<'_> {
        ResourceTreeResourceIter { stack: vec![self] }
    }

    /// Return references to all resources in this hierarchy.
    pub fn iter_resource_refs<'a>(
        &self,
        resources: &'a impl ResourceCollection,
    ) -> impl Iterator<Item = AnalyzerResult<&'a dyn Resource>> {
        self.iter_resource_ids().map(|id| resources.resource(id))
    }

    /// Breadth-first search for a specific entity ID.
    pub fn find(&self, target_id: Uuid) -> Option<&Self> {
        let mut queue = VecDeque::new();
        queue.push_back(self);

        while let Some(node) = queue.pop_front() {
            if node.entity_id == target_id {
                return Some(node);
            }
            queue.extend(node.children.iter());
        }

        None
    }
}

/// Iterator over all resources in a [`ResourceTreeNode`].
pub struct ResourceTreeResourceIter<'a> {
    stack: Vec<&'a ResourceTreeNode>,
}

impl Iterator for ResourceTreeResourceIter<'_> {
    type Item = Uuid;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.stack.pop() {
            self.stack.extend(node.children.iter().rev());
            if node.is_resource {
                return Some(node.entity_id);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(
        entity_id: Uuid,
        is_resource: bool,
        children: Vec<ResourceTreeNode>,
    ) -> ResourceTreeNode {
        ResourceTreeNode {
            entity_id,
            is_resource,
            children,
        }
    }

    #[test]
    fn iterates_resources() {
        let first_resource_id = Uuid::from_u128(1);
        let second_resource_id = Uuid::from_u128(2);
        let group_id = Uuid::from_u128(3);
        let root_id = Uuid::from_u128(4);
        let tree = node(
            root_id,
            false,
            vec![
                node(first_resource_id, true, vec![]),
                node(
                    group_id,
                    false,
                    vec![node(second_resource_id, true, vec![])],
                ),
            ],
        );

        assert_eq!(
            tree.iter_resource_ids().collect::<Vec<_>>(),
            [first_resource_id, second_resource_id]
        );
    }

    #[test]
    fn resource_may_have_resource_children() {
        let parent_resource_id = Uuid::from_u128(1);
        let child_resource_id = Uuid::from_u128(2);
        let tree = node(
            parent_resource_id,
            true,
            vec![node(child_resource_id, true, vec![])],
        );

        assert_eq!(
            tree.iter_resource_ids().collect::<Vec<_>>(),
            [parent_resource_id, child_resource_id]
        );
    }

    #[test]
    fn finds_entities() {
        let root_id = Uuid::from_u128(1);
        let child_id = Uuid::from_u128(2);
        let tree = node(root_id, false, vec![node(child_id, true, vec![])]);

        assert_eq!(tree.find(child_id).unwrap().entity_id, child_id);
        assert!(tree.find(Uuid::from_u128(3)).is_none());
    }
}
