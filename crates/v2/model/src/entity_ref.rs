// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::marker::PhantomData;

use quent_v2_model_ir::event as ir;
use uuid::Uuid;

use crate::entity::Entity;

/// Trait to declare a type represents an entity reference role.
pub trait EntityRefRole {
    #[cfg(feature = "ir")]
    fn ir() -> quent_v2_model_ir::event::EntityRefRole;
}

/// Trait to convey an entity of type `T` is allowed as the target of an
/// [`EntityRef`] with the role that `Self` represents.
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
/// used. This is the default.
///
/// ## Built-in Role Types
///
/// ### [`Plain`]
///
/// A reference role for references of no particular meaning, other than that
/// the entities are somehow related.
///
/// ### [`Scope`]
///
/// A reference role for "structural" references, used to form a hierarchical
/// tree of entities. An entity can have at most one reference with this role.
/// An entity with no references with this role is considered a root entity.
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
// TODO(johanpel): what if the same relation captures multiple roles, should it
// be a new role or could it be a tuple of role types?

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

/// [`EntityRefRole`] used in entities that desire to declare they exist within
/// the scope of *exactly one* other entity.
///
/// The purpose of this role is to construct a tree of entities.
///
/// This reference role accepts any type of reference to an entity, including
/// [`AnyEntity`]. This tree typically provides an application-specific
/// preferred way of traversing a potentially complicated graph of entities and
/// their relations.
///
/// For example, it can be used to capture (among other potential usages):
/// - a hierarchy of resources (what manages X)
/// - the structure of the run-time arcitecture of program (what is part of X)
/// - the lineage of "objects" in a program (what created X)
/// - the lineage of operations executed by a program (what ran under X)
/// - unqiue ownership relations (what owns X)
///
/// # Example
///
/// An example of how `Scope`-roled reference could form a tree of entities at
/// run-time is shown below:
///
/// ```text
/// - Cluster (root)
///     - Worker A
///         - SchedulingThread
///             - Task X
///             - Task Y
///             - ...
///         - MemoryPool
///         - ThreadPool
///             - Thread 0
///             - Thread 1
///         - TaskQueue
///         - ...
///     - Worker B
///         - ...
/// ```
///
/// This tree captures the layout of entities at the higher levels capturing a
/// hierarchy of resources in a distributed application, starting with `Cluster`
/// as root, until they arrive at the level of `Task`.
///
/// One can imagine the `Task` entity to be an FSM which may refer to various
/// entities over the course of its lifecycle of states. For example, the
/// `SchedulingThread` when it is just created, the `Thread` it is scheduled to
/// run on when it is executing, and the `MemoryPool` entity when it is done
/// processing. While the `Task` refers to many other entities, it only refers
/// to the `SchedulingThread` with the `Scope`-role reference, as a means of
/// proclaiming its lineage ("i was created by the scheduling thread"). This
/// way, the entity tree that is formed represents a hierarchy of run-time
/// objects.
///
/// # Note on single parent enforcement and conditional events
///
/// Application logic may exist where there is a single entity that
/// conditionally emits certain events. Therefore, no restrictions can be placed
/// on the number of events that may carry this role at compile-time, while
/// logic dictates 🖖 exactly one such event should be emitted at run-time. Only
/// the amount of event fields that carry a reference with this role is limited
/// to one at compile-time.
// TODO(johanpel): implement this validation
///
/// While emitting a correct pattern of events w.r.t. some entity event model is
/// the responsibility of the application, it is in this case recommended to add
/// an event that unconditionally fires to convey this relation instead, such
/// that there is a slightly stronger guarantee no entity is left "dangling" by
/// not depending on conditionally fired events to convey this information to
/// downstream event consumers.
///
/// Emitting multiple events with an unequal entity reference of this role has
/// conceptually undefined semantics. Any possible interpretation is left to the
/// implementation of the consumer of the events. For example, it may decide to
/// only keep the reference in the latest event.
// TODO(johanpel): a run-time warning could be introduced if the parent is
// emitted multiple times and not equal to the last event emitting it
pub struct Scope;

// In order for an `EntityRef` to be constructible:
//
// `E` must be an entity.
// `R` must be a role.
// `E` must be an acceptible target for the role R.
impl<R, E> EntityRef<R, E>
where
    E: Entity,
    R: EntityRefRole + EntityRefRoleTarget<E>,
{
    /// Construct a new entity reference.
    pub fn new(id: Uuid, role: R) -> Self {
        Self {
            id,
            role,
            _target_type: PhantomData,
        }
    }

    /// Return the raw UUID of the entity that this reference refers to.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Return the raw role-specific data.
    pub fn role(&self) -> &R {
        &self.role
    }
}

impl EntityRefRole for Plain {
    fn ir() -> ir::EntityRefRole {
        ir::EntityRefRole::Plain
    }
}
/// Plain references can target anything that is an entity.
impl<E: Entity> EntityRefRoleTarget<E> for Plain {}

impl EntityRefRole for Scope {
    fn ir() -> ir::EntityRefRole {
        ir::EntityRefRole::Scope
    }
}
/// Scope references can target anything that is an entity. The difference from
/// the [`Plain`] role is that an entity can only have at most one reference with
/// the [`Scope`] role, enforced at compile-time.
impl<E: Entity> EntityRefRoleTarget<E> for Scope {}

/// AnyEntity is a special case of an entity.
impl Entity for AnyEntity {
    #[cfg(feature = "ir")]
    fn ir() -> quent_v2_model_ir::entity::Entity {
        panic!(
            "AnyEntity is a marker type for reference targets only, it has no IR representation as Entity"
        )
    }
    #[cfg(feature = "ir")]
    fn ir_ref_target() -> quent_v2_model_ir::event::EntityRefTarget {
        ir::EntityRefTarget::Any
    }
}
