// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Collections of entities participating in a scope tree.

use uuid::Uuid;

use crate::{AnalyzerResult, ScopedEntity};

/// Provides lookup and traversal of scoped entities.
pub trait ScopeCollection {
    /// Return all entities participating in the scope tree.
    fn scoped_entities(&self) -> impl Iterator<Item = &dyn ScopedEntity>;

    /// Return the scoped entity with the provided ID.
    ///
    /// # Errors
    ///
    /// Returns an error when the ID does not identify a scoped entity.
    fn scoped_entity(&self, entity_id: Uuid) -> AnalyzerResult<&dyn ScopedEntity>;

    /// Return the direct children of the provided entity.
    fn scope_children(&self, entity_id: Uuid) -> impl Iterator<Item = &dyn ScopedEntity> {
        self.scoped_entities()
            .filter(move |entity| entity.scope_id() == Some(entity_id))
    }
}

#[cfg(test)]
mod tests {
    use quent_time::TimeUnixNanoSec;

    use super::*;
    use crate::{AnalyzerError, Entity};

    struct TestEntity {
        id: Uuid,
        scope_id: Option<Uuid>,
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

    impl ScopedEntity for TestEntity {
        fn scope_id(&self) -> Option<Uuid> {
            self.scope_id
        }
    }

    struct TestCollection(Vec<TestEntity>);

    impl ScopeCollection for TestCollection {
        fn scoped_entities(&self) -> impl Iterator<Item = &dyn ScopedEntity> {
            self.0.iter().map(|entity| entity as &dyn ScopedEntity)
        }

        fn scoped_entity(&self, entity_id: Uuid) -> AnalyzerResult<&dyn ScopedEntity> {
            self.0
                .iter()
                .find(|entity| entity.id == entity_id)
                .map(|entity| entity as &dyn ScopedEntity)
                .ok_or(AnalyzerError::InvalidId(entity_id))
        }
    }

    #[test]
    fn traverses_direct_scope_children() {
        let root_id = Uuid::from_u128(1);
        let child_id = Uuid::from_u128(2);
        let grandchild_id = Uuid::from_u128(3);
        let collection = TestCollection(vec![
            TestEntity {
                id: root_id,
                scope_id: None,
            },
            TestEntity {
                id: child_id,
                scope_id: Some(root_id),
            },
            TestEntity {
                id: grandchild_id,
                scope_id: Some(child_id),
            },
        ]);

        assert_eq!(
            collection
                .scope_children(root_id)
                .map(Entity::id)
                .collect::<Vec<_>>(),
            [child_id]
        );
        assert_eq!(
            collection.scoped_entity(grandchild_id).unwrap().id(),
            grandchild_id
        );
    }
}
