// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::marker::PhantomData;

use quent_v2_model_ir::{
    attributes::{EntityRefKind, EntityRefTarget},
    value_type::{ModelEntityRefKind, ModelEntityRefTarget},
};
use uuid::Uuid;

use crate::EntityDeclaration;

/// Trait allowing specific [`EntityRef`]s to be type-erased.
pub trait IntoErased<T> {
    fn into_erased(self) -> T;
}

/// A directed reference to another entity.
///
/// # The Entity Type `E`
///
/// Specifies the type of entity referring to.
///
/// If multiple of entities are allowed, the marker type [`AnyEntity`] can be
/// used.
///
/// # The Scope Type `S`
///
/// Specifies the scope in which this entity is referred to.
///
/// Entities of an application model form an entity-relation graph where
/// entities are vertices and [`EntityRef`]s are edges.
///
/// Scopes are sub-graphs of the entity-relation graph that can be reached from
/// the root entity.
///
/// Formally speaking, scopes include:
///
/// 1. the set of all [`EntityRef`]s of `S` and their source and target
///    entities
/// 2. any set of [`EntityRef`]s and entities that allow traversing the
///    entity-relation graph in reverse direction from the root to arrive
///    at the entities included in 1
///
/// ## The `Root` Scope
///
/// Every [`EntityRef`] is implicitly part of the pre-defined [`Root`] scope, even
/// if S is not [`Root`].
///
/// If the [`Root`] scope (i.e. the graph of all entities) is disconnected, then
/// the application model is invalid. Colloquially speaking, this would mean that
/// there would be no way to relate an entity's events back to the top-level
/// application instance ID, making them "orphans".
///
/// # The Reference Role Type `R`
///
/// `R` defines type of role of the [`EntityRef`]. By default, it is a plain
/// reference ([`Plain`]) which holds no particular meaning besides the
/// entities being somehow related.
///
/// `R` can also carry data in case a reference is richer than just a way of
/// pointing to another entity. For example, FSM states can use a resource, in
/// which case they must emit data about the amount used, also see the
/// [`crate::resource::Usage`] role type.
///
///
/// # Forming Tree-Like Scopes
///
/// Tree-like scopes can be formed by restricting some reference role `R` to be
/// applied within a certain scope `S`.
///
/// For example, to enforce an application model to have a "resource tree", define:
/// - A scope `S = ResourceScope`
/// - A role `R = ResourceParent`
///
/// Then ensure that if an `EntityRef` has `T = Resource`

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy)]
pub struct EntityRef<E: EntityDeclaration, R = PlainRef> {
    pub _target_type: PhantomData<E>,

    /// The ID of the entity being referred to.
    pub id: Uuid,
    /// Data (or a marker) specific to the role of this reference.
    pub role: R,
}

pub struct EntityRefs<E: EntityDeclaration, R = PlainRef>(pub Vec<EntityRef<E, R>>);

/// Type to mark an [`EntityRef`] can point to any type of entity, to be
/// determined at run-time.
pub struct AnyEntity;

/// Type to mark an [`EntityRef`] of being of no particular meaning.
pub struct PlainRef;

// Special case:
impl EntityDeclaration for AnyEntity {}

impl ModelEntityRefTarget for AnyEntity {
    fn model_entity_ref_target() -> EntityRefTarget {
        EntityRefTarget::Any
    }
}

impl ModelEntityRefKind for PlainRef {
    fn model_entity_ref_kind() -> EntityRefKind {
        EntityRefKind::Plain
    }
}
