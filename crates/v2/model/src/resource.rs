// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
use std::marker::PhantomData;

use uuid::Uuid;

// Notes:
//
// Resources should roughly be considered an attribute "convention" on top of
// the entity and FSM semantics. As such it should be possible to provide a
// sugaring syntax over those concepts, without requiring additional core things
// from the entity and FSM derives.

// TODO: seal traits below

// Trait + markers for whether capacities are bounded or unbounded
pub trait Boundedness {}
/// The resource capacity is bounded.
pub struct Bounded;
impl Boundedness for Bounded {}
/// The resource capacity is unbounded.
///
/// It is physically always bounded, but the bounds may be unknown.
pub struct Unbounded;
impl Boundedness for Unbounded {}

// Trait + markers for capacities that after resource init are either fixed or dynamically resizable.
pub trait Resizeability {}
/// The resource capacity is fixed after initialization.
pub struct Fixed;
impl Resizeability for Fixed {}
/// The resource capacity is resizable after initialization.
pub struct Resizable;
impl Resizeability for Resizable {}

// Trait + markers for the kind of capacity.
pub trait CapacityKind {}
/// The resource capacity is fixed after initialization.
pub struct Occupancy;
impl CapacityKind for Occupancy {}
/// The resource capacity is resizable after initialization.
pub struct Rate;
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
pub struct Capacity<T, K = Occupancy, R = Fixed, B = Bounded>
where
    K: CapacityKind,
    R: Resizeability,
    B: Boundedness,
{
    _value_type: PhantomData<T>,
    _kind: PhantomData<K>,
    _bounded: PhantomData<B>,
    _resizable: PhantomData<R>,
}

// User-facing types used in the instrumentation API:
pub struct OccupancyBound<T> {
    pub value: T,
}

pub struct RateBound<T> {
    pub items: T,
    pub nanoseconds: u64,
}

/// To convey a new capacity value.
pub struct CapacityValue<ValueType> {
    pub value: ValueType,
}

/// A trait for resources that allows setting the usage amounts of the
/// capacities during instrumentation run time.
pub trait Resource {
    type UsageValueType; // this must be serde/narrow/etc. compatible
}

/// A type serving as an [`crate::entity_ref::EntityRef`] role for FSMs to
/// convey they are using a resource for the duration of some state.
pub struct Usage<R>
where
    R: Resource,
{
    pub amounts: R::UsageValueType,
}
