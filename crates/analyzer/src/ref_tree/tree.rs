// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Runtime Reference Tree construction and validation.

use rustc_hash::FxHashMap as HashMap;
use uuid::Uuid;

use crate::{AnalyzerError, AnalyzerResult, ref_tree::collection::RefTreeCollection};

/// A node in a validated runtime Reference Tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefTreeNode {
    pub entity_id: Uuid,
    pub children: Vec<RefTreeNode>,
}

impl RefTreeNode {
    /// Construct and validate the Reference Tree represented by `collection`.
    ///
    /// Construction takes `O(n)` time and storage for `n` entities.
    ///
    /// # Errors
    ///
    /// Returns [`AnalyzerError::Validation`] if IDs are duplicated, the root is
    /// not unique, a parent is missing, or the relationships contain a cycle.
    pub fn try_new(collection: &impl RefTreeCollection) -> AnalyzerResult<Self> {
        let mut parents = HashMap::default();
        let mut children: HashMap<Uuid, Vec<Uuid>> = HashMap::default();
        let mut roots = Vec::new();

        for entity in collection.ref_tree_entities() {
            let id = entity.id();
            let parent_id = entity.parent_id();
            if parents.insert(id, parent_id).is_some() {
                return Err(AnalyzerError::Validation(format!(
                    "duplicate Reference Tree entity id {id}"
                )));
            }
            match parent_id {
                Some(parent_id) => children.entry(parent_id).or_default().push(id),
                None => roots.push(id),
            }
        }

        if roots.len() != 1 {
            return Err(AnalyzerError::Validation(format!(
                "Reference Tree must have exactly one root, found {}",
                roots.len()
            )));
        }

        for (id, parent_id) in &parents {
            if let Some(parent_id) = parent_id
                && !parents.contains_key(parent_id)
            {
                return Err(AnalyzerError::Validation(format!(
                    "Reference Tree entity {id} references missing parent {parent_id}"
                )));
            }
        }

        let mut visits = HashMap::default();
        for id in parents.keys().copied() {
            validate_acyclic(id, &parents, &mut visits)?;
        }

        Ok(build_tree(roots[0], &mut children))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Visit {
    Active,
    Complete,
}

fn validate_acyclic(
    id: Uuid,
    parents: &HashMap<Uuid, Option<Uuid>>,
    visits: &mut HashMap<Uuid, Visit>,
) -> AnalyzerResult<()> {
    match visits.get(&id) {
        Some(Visit::Active) => {
            return Err(AnalyzerError::Validation(format!(
                "Reference Tree contains a cycle at entity {id}"
            )));
        }
        Some(Visit::Complete) => return Ok(()),
        None => {}
    }

    visits.insert(id, Visit::Active);
    if let Some(parent_id) = parents[&id] {
        validate_acyclic(parent_id, parents, visits)?;
    }
    visits.insert(id, Visit::Complete);
    Ok(())
}

fn build_tree(id: Uuid, children: &mut HashMap<Uuid, Vec<Uuid>>) -> RefTreeNode {
    RefTreeNode {
        entity_id: id,
        children: children
            .remove(&id)
            .unwrap_or_default()
            .into_iter()
            .map(|child_id| build_tree(child_id, children))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use quent_time::TimeUnixNanoSec;

    use super::*;
    use crate::{Entity, RefTreeEntity};

    struct TestEntity {
        id: Uuid,
        parent_id: Option<Uuid>,
    }

    impl Entity for TestEntity {
        fn id(&self) -> Uuid {
            self.id
        }

        fn type_name(&self) -> &str {
            "test"
        }

        fn earliest_timestamp(&self) -> TimeUnixNanoSec {
            0
        }

        fn latest_timestamp(&self) -> TimeUnixNanoSec {
            0
        }
    }

    impl RefTreeEntity for TestEntity {
        fn parent_id(&self) -> Option<Uuid> {
            self.parent_id
        }
    }

    struct TestCollection(Vec<TestEntity>);

    impl RefTreeCollection for TestCollection {
        fn ref_tree_entities(&self) -> impl Iterator<Item = &dyn RefTreeEntity> {
            self.0.iter().map(|entity| entity as &dyn RefTreeEntity)
        }

        fn ref_tree_entity(&self, entity_id: Uuid) -> AnalyzerResult<&dyn RefTreeEntity> {
            self.0
                .iter()
                .find(|entity| entity.id == entity_id)
                .map(|entity| entity as &dyn RefTreeEntity)
                .ok_or(AnalyzerError::InvalidId(entity_id))
        }
    }

    #[test]
    fn constructs_reference_tree() {
        let root_id = Uuid::from_u128(1);
        let child_id = Uuid::from_u128(2);
        let grandchild_id = Uuid::from_u128(3);
        let tree = RefTreeNode::try_new(&TestCollection(vec![
            TestEntity {
                id: grandchild_id,
                parent_id: Some(child_id),
            },
            TestEntity {
                id: root_id,
                parent_id: None,
            },
            TestEntity {
                id: child_id,
                parent_id: Some(root_id),
            },
        ]))
        .unwrap();

        assert_eq!(tree.entity_id, root_id);
        assert_eq!(tree.children[0].entity_id, child_id);
        assert_eq!(tree.children[0].children[0].entity_id, grandchild_id);
    }

    #[test]
    fn rejects_duplicate_ids() {
        let id = Uuid::from_u128(1);
        assert!(matches!(
            RefTreeNode::try_new(&TestCollection(vec![
                TestEntity {
                    id,
                    parent_id: None,
                },
                TestEntity {
                    id,
                    parent_id: None,
                },
            ])),
            Err(AnalyzerError::Validation(_))
        ));
    }

    #[test]
    fn rejects_missing_parent() {
        assert!(matches!(
            RefTreeNode::try_new(&TestCollection(vec![
                TestEntity {
                    id: Uuid::from_u128(1),
                    parent_id: None,
                },
                TestEntity {
                    id: Uuid::from_u128(2),
                    parent_id: Some(Uuid::from_u128(3)),
                },
            ])),
            Err(AnalyzerError::Validation(_))
        ));
    }

    #[test]
    fn rejects_disconnected_cycle() {
        let first = Uuid::from_u128(2);
        let second = Uuid::from_u128(3);
        assert!(matches!(
            RefTreeNode::try_new(&TestCollection(vec![
                TestEntity {
                    id: Uuid::from_u128(1),
                    parent_id: None,
                },
                TestEntity {
                    id: first,
                    parent_id: Some(second),
                },
                TestEntity {
                    id: second,
                    parent_id: Some(first),
                },
            ])),
            Err(AnalyzerError::Validation(_))
        ));
    }
}
