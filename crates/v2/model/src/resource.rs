// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
use std::marker::PhantomData;

use quent_v2_model_ir::event as ir;

use crate::{
    Entity,
    entity_ref::{EntityRefRole, EntityRefRoleTarget},
};

// Notes:
//
// Resources should roughly be considered an attribute "convention" on top of
// the entity and FSM semantics. As such it should be possible to provide a
// sugaring syntax over those concepts, without requiring additional core things
// from the entity and FSM derives.

// TODO: seal traits below

/// Trait for markers defining how a capacity's bounds are to be perceived.
pub trait Boundedness {}

/// Trait for markers defining the kind of capacity.
pub trait CapacityKind {}

/// A trait for entities that are resources.
pub trait Resource: Entity {
    type UsageType;
    type BoundsType;
}

/// The resource capacity is fixed-size and bounded.
pub struct Fixed;

/// The resource capacity is resizeable and bounded.
///
/// It is physically always bounded, but the bounds may be unknown.
pub struct Resizeable;

/// The resource capacity is unbounded.
///
/// While in reality capacities are always subject to physical limits, the
/// bounds are unknown as the application model is concerned. This can be used
/// for abstractions of resources where it is non-trivial to obtain the bounds
/// (e.g. data transfer rates over an unknown physical network interface
/// abstracted as a rate capacity channel resource).
pub struct Unbounded;

impl Boundedness for Fixed {}
impl Boundedness for Resizeable {}
impl Boundedness for Unbounded {}

/// The resource capacity is fixed after initialization.
pub struct Occupancy;

/// The resource capacity is resizable after initialization.
pub struct Rate;

impl CapacityKind for Occupancy {}
impl CapacityKind for Rate {}

// User-facing types used during modeling While K, R, and B are two-valued
// properties, which would technically allow for the use of a const bool
// generic, it would make the declaration site less readable, hence we favor
// marker types.
//
// TODO: since not all combinations of R and B are allowed, consider making it a
// single three-valued generic.
//
// Would be nice if we could use plain enums as const generics, but we can't.
pub struct Capacity<T, K = Occupancy, B = Fixed>
where
    K: CapacityKind,
    B: Boundedness,
{
    _value_type: PhantomData<T>,
    _kind: PhantomData<K>,
    _bounded: PhantomData<B>,
}

// User-facing types used in the instrumentation API:

/// A bound of an [`Occupancy`]-type [`Resource`] [`Capacity`].
pub struct OccupancyBound<T> {
    pub value: T,
}

/// A bound of a [`Rate`]-type [`Resource`] [`Capacity`].
pub struct RateBound<T> {
    /// The number of items in the rate bound expressed as items/nanoseconds
    pub items: T,
    /// The amount of nanoseconds in the rate bound expressed as items/nanoseconds.
    pub nanoseconds: u64,
}

/// An [`crate::entity_ref::EntityRef`] role for FSMs to convey they are using a
/// resource for the duration of some state.
pub struct Usage<R>
where
    R: Resource,
{
    pub amounts: R::UsageType,
}

// /// The capacity bound of a resourtc
// pub struct Bounds<R>
// where
//     R: Resource,
// {
//     pub bounds: R::BoundsType,
// }

// Usage is a role of a reference
impl<R: Resource> EntityRefRole for Usage<R> {
    #[cfg(feature = "ir")]
    fn ir() -> ir::EntityRefRole {
        todo!()
    }
}
// A reference with a resource usage role can only target resource entities
impl<R: Resource> EntityRefRoleTarget<R> for Usage<R> {}
