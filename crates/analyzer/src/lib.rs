// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Telemetry analysis functionality based on modeling primitives.

pub use crate::error::AnalyzerError;
use crate::resource::{ResourceGroup, collection::ResourceCollection, tree::ResourceTreeNode};
pub use entity::{Entity, ScopedEntity};
use quent_time::{TimeUnixNanoSec, span::SpanUnixNanoSec};
use uuid::Uuid;

pub mod context;
pub mod entity;
pub mod error;
pub mod fsm;
pub mod resource;
pub mod timeline;

pub type AnalyzerResult<T> = std::result::Result<T, AnalyzerError>;

/// Trait for entities associated with a single moment in time.
pub trait Instant: Entity {
    /// Return the timestamp associated with this type.
    fn instant(&self) -> AnalyzerResult<TimeUnixNanoSec>;
}

/// Trait for things that are associated with a span of time.
///
/// Typically represents the entire lifetime of the entity.
pub trait Span {
    /// Return the span of time this type is associated with.
    ///
    /// # Errors
    ///
    /// This function can return an [`AnalyzerError`] in cases such as:
    /// - Events are missing to form a complete entity model.
    /// - The sequence of FSM transition events violates model specifications.
    fn span(&self) -> AnalyzerResult<SpanUnixNanoSec>;
}

/// Trait for type safety wrappers around entity IDs.
pub trait EntityId {
    fn is_resource(&self) -> bool;
    fn is_resource_group(&self) -> bool;
}

/// Trait for application models.
pub trait Model: ResourceCollection {
    /// Type-safety wrapper around an entity ID.
    type EntityIdType: EntityId;

    /// Given an [`AnalyzedEntity`] ID, resolve it into an [`Self::EntityIdType`].
    fn try_entity_ref(&self, entity_id: Uuid) -> AnalyzerResult<Self::EntityIdType>;

    /// Return the root resource group.
    fn root(&self) -> AnalyzerResult<&impl ResourceGroup>;

    /// Return the resource tree.
    fn resource_tree(&self) -> AnalyzerResult<ResourceTreeNode>
    where
        Self: Sized,
    {
        ResourceTreeNode::try_new(self, self.root()?.id())
    }
}
