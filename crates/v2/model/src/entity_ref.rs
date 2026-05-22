// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::marker::PhantomData;

use quent_v2_model_ir::event as ir;
use uuid::Uuid;

use crate::entity::Entity;

/// Trait to declare a type can be used to define the role of an entity
/// reference
pub trait EntityRefRole {}

/// Trait to convey T can be the target of a reference with the Self role.
pub trait EntityRefRoleTarget<T: Entity>: EntityRefRole {}

/// A directed reference to another entity with a particular role.
///
/// # Role Type `R`
///
/// `R` defines type of role of the [`EntityRef`]. The type `R` may carry
/// data detailing the relation. By default, `R = Plain`.
///
/// # Entity Type `E`
///
/// Specifies the type of entity referring to.
///
/// If multiple of entities are allowed, the marker type [`AnyEntity`] can be
/// used, which is the default.
///
/// ## Built-in Role Types
///
/// ### [`Scope`]
///
/// A reference role for "structural" references, used to form a hierarchical
/// tree of entities. An entity can have at most one reference with this role.
/// An entity with no references with this role is considered a root entity.
///
/// ### [`Plain`]
///
/// A reference role for references of no particular meaning, other than that
/// the entities are somehow related.
///
/// ### [`crate::resource::Usage<R>`]
///
/// A reference role for the usage of some resource type `R`.
///
/// ## Restricting which entities a role may target
///
/// Some roles only make sense with certain target entity types. For
/// example, a [`Scope`] reference must not point at a resource (so
/// resources are leaves of the scope tree), and a `Usage<R>` reference
/// must only point at the resource `R` it consumes.
///
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy)]
pub struct EntityRef<R = Plain, E = AnyEntity>
where
    E: Entity,
    R: EntityRefRole + EntityRefRoleTarget<E>,
{
    _target_type: PhantomData<E>,

    /// The ID of the entity being referred to.
    id: Uuid,
    /// Data (or a marker) specific to the role of this reference.
    role: R,
}

/// Marker type for a type-erased entity target of an [`EntityRef`].
///
/// In other words, use this if the reference can be made to any type of entity,
/// to be determined at instrumentatiton run-time.
pub struct AnyEntity;

/// [`EntityRefRole`] where the reference is of no particular meaning.
///
/// This reference role accepts any type of entity, including at run-time
/// through [`AnyEntity`].
///
/// Thus, the weakest possible form of referencing another entity is by using
/// `EntityRef<AnyEntity, Plain>`, which represents a relation to any other
/// entity without any particular meaning.
pub struct Plain;

/// [`EntityRefRole`] to mark the [`EntityRef`] refers to the parent in the
/// application's entity tree.
///
/// This reference role accepts any type of entity, including [`AnyEntity`].
///
/// Thus, the weakest possible form of referencing another entity is by using
/// `EntityRef<AnyEntity, Plain>`, which represents a relation to any other
/// entity without any particular meaning.
pub struct Scope;

/// In order for an `EntityRef` to be constructible:
///
/// `E` must be an entity.
/// `R` must be a role.
/// `E` must be an acceptible target for the role R.
impl<R, E> EntityRef<R, E>
where
    E: Entity,
    R: EntityRefRole + EntityRefRoleTarget<E>,
{
    pub fn new(id: Uuid, role: R) -> Self {
        Self {
            id,
            role,
            _target_type: PhantomData,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn role(&self) -> &R {
        &self.role
    }
}

impl EntityRefRole for Plain {}
/// Plain references can target anything that is an entity.
impl<E: Entity> EntityRefRoleTarget<E> for Plain {}

impl EntityRefRole for Scope {}
/// Scope references can target anything that is an entity. The difference from
/// the [`Plain`] role is that an entity can only have at most one reference with
/// the [`Scope`] role, enforced at compile-time.
impl<E: Entity> EntityRefRoleTarget<E> for Scope {}

/// AnyEntity is a special case of an entity.
impl Entity for AnyEntity {}

// The IR has a special case for Plain reference roles.
impl ir::ModelEntityRefRole for Plain {
    fn model_entity_ref_role() -> ir::EntityRefRole {
        ir::EntityRefRole::Plain
    }
}

// The IR has a special case for Scope reference roles.
impl ir::ModelEntityRefRole for Scope {
    fn model_entity_ref_role() -> ir::EntityRefRole {
        ir::EntityRefRole::Scope
    }
}

// The IR has a special case for Any entity reference targets.
impl ir::ModelEntityRefTarget for AnyEntity {
    fn model_entity_ref_target() -> ir::EntityRefTarget {
        ir::EntityRefTarget::Any
    }
}
