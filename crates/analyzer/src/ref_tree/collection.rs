// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Collections of entities participating in a Reference Tree.

use uuid::Uuid;

use crate::{AnalyzerResult, RefTreeEntity};

/// Provides lookup and traversal of entities participating in a Reference Tree.
pub trait RefTreeCollection {
    /// Return all entities participating in the Reference Tree.
    fn ref_tree_entities(&self) -> impl Iterator<Item = &dyn RefTreeEntity>;

    /// Return the Reference Tree entity with the provided ID.
    ///
    /// # Errors
    ///
    /// Returns an error when the ID does not identify a Reference Tree entity.
    fn ref_tree_entity(&self, entity_id: Uuid) -> AnalyzerResult<&dyn RefTreeEntity>;

    /// Return the direct children of the provided entity.
    fn children(&self, entity_id: Uuid) -> impl Iterator<Item = &dyn RefTreeEntity> {
        self.ref_tree_entities()
            .filter(move |entity| entity.parent_id() == Some(entity_id))
    }
}

#[cfg(test)]
mod tests {
    use quent_time::TimeUnixNanoSec;

    use super::*;
    use crate::{AnalyzerError, Entity};

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
    fn traverses_direct_children() {
        let root_id = Uuid::from_u128(1);
        let child_id = Uuid::from_u128(2);
        let grandchild_id = Uuid::from_u128(3);
        let collection = TestCollection(vec![
            TestEntity {
                id: root_id,
                parent_id: None,
            },
            TestEntity {
                id: child_id,
                parent_id: Some(root_id),
            },
            TestEntity {
                id: grandchild_id,
                parent_id: Some(child_id),
            },
        ]);

        assert_eq!(
            collection
                .children(root_id)
                .map(Entity::id)
                .collect::<Vec<_>>(),
            [child_id]
        );
        assert_eq!(
            collection.ref_tree_entity(grandchild_id).unwrap().id(),
            grandchild_id
        );
    }
}
