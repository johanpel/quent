// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Indexing of runtime contexts by the entities that contribute telemetry.

use std::collections::{BTreeMap, BTreeSet};

use rustc_hash::FxHashMap;
use uuid::Uuid;

/// Identifies a runtime context rather than an instrumented entity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContextId(Uuid);

impl ContextId {
    /// Returns the underlying UUID for storage or transport APIs.
    pub const fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl From<Uuid> for ContextId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

/// Entities discovered by cheaply scanning one runtime context.
///
/// For example, in a distributed application with one driver and multiple
/// workers, the driver's context identifies the driver as both a contributing
/// entity and the analysis target. Each worker context identifies its worker as
/// a contributing entity and the driver as the analysis target. Only events
/// carrying these identities need to be read.
#[derive(Debug, Default)]
pub struct ContextInventory {
    /// The analysis targets represented in this context.
    ///
    /// Current analyzers produce one entry per relevant context; the vector also
    /// permits a context to contain entities for more than one target.
    pub analysis_targets: Vec<ContextAnalysisTarget>,
}

/// Entities in one context whose telemetry contributes to the same analysis
/// target.
///
/// For example, the entry for a driver context contains the driver ID as both
/// `analysis_target_id` and an `entity_ids` member. The entry for a worker
/// context contains the driver ID as `analysis_target_id` and the worker ID as
/// an `entity_ids` member.
#[derive(Debug)]
pub struct ContextAnalysisTarget {
    /// The entity being analyzed, such as the driver of a distributed application.
    pub analysis_target_id: Uuid,
    /// The contributing entities found in this context, such as a driver or worker.
    pub entity_ids: Vec<Uuid>,
}

/// Finds the contexts needed to analyze an entity.
///
/// For example, analysis of a distributed application's driver uses the
/// driver's context and every worker context. Independent targets may use the
/// same or different contexts, such as two implementations running the same
/// benchmark workload whose results will be compared.
#[derive(Debug, Default)]
pub struct ContextIndex {
    contexts_by_analysis_target: FxHashMap<Uuid, BTreeSet<ContextId>>,
    entities_by_analysis_target_context: FxHashMap<Uuid, BTreeMap<ContextId, BTreeSet<Uuid>>>,
}

impl ContextIndex {
    /// Adds the associations discovered in one runtime context.
    pub fn add_inventory(&mut self, context_id: ContextId, inventory: ContextInventory) {
        for analysis_target in inventory.analysis_targets {
            self.contexts_by_analysis_target
                .entry(analysis_target.analysis_target_id)
                .or_default()
                .insert(context_id);
            for entity_id in analysis_target.entity_ids {
                self.entities_by_analysis_target_context
                    .entry(analysis_target.analysis_target_id)
                    .or_default()
                    .entry(context_id)
                    .or_default()
                    .insert(entity_id);
            }
        }
    }

    /// Returns the entities for which aggregate analysis can be requested.
    pub fn analysis_target_ids(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.contexts_by_analysis_target.keys().copied()
    }

    /// Returns all contexts whose telemetry contributes to `analysis_target_id`.
    pub fn contexts_of_analysis_target(&self, analysis_target_id: Uuid) -> Vec<ContextId> {
        self.contexts_by_analysis_target
            .get(&analysis_target_id)
            .map(|contexts| contexts.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Returns each contributing context and its entities for `analysis_target_id`.
    pub fn entities_by_context_of_analysis_target(
        &self,
        analysis_target_id: Uuid,
    ) -> BTreeMap<ContextId, Vec<Uuid>> {
        let mut entities: BTreeMap<ContextId, Vec<Uuid>> = self
            .contexts_of_analysis_target(analysis_target_id)
            .into_iter()
            .map(|context_id| (context_id, Vec::new()))
            .collect();
        if let Some(by_context) = self
            .entities_by_analysis_target_context
            .get(&analysis_target_id)
        {
            for (&context_id, entity_ids) in by_context {
                entities.insert(context_id, entity_ids.iter().copied().collect());
            }
        }
        entities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_contexts_and_entities_by_analysis_target() {
        let analysis_target_id = Uuid::from_u128(1);
        let target_context = Uuid::from_u128(2);
        let child_context = Uuid::from_u128(3);
        let child_id = Uuid::from_u128(4);
        let mut index = ContextIndex::default();

        index.add_inventory(
            target_context.into(),
            ContextInventory {
                analysis_targets: vec![ContextAnalysisTarget {
                    analysis_target_id,
                    entity_ids: vec![analysis_target_id],
                }],
            },
        );
        index.add_inventory(
            child_context.into(),
            ContextInventory {
                analysis_targets: vec![ContextAnalysisTarget {
                    analysis_target_id,
                    entity_ids: vec![child_id],
                }],
            },
        );

        assert_eq!(
            index.contexts_of_analysis_target(analysis_target_id),
            vec![target_context.into(), child_context.into()]
        );
        assert_eq!(
            index.entities_by_context_of_analysis_target(analysis_target_id),
            BTreeMap::from([
                (target_context.into(), vec![analysis_target_id]),
                (child_context.into(), vec![child_id]),
            ])
        );
    }
}
