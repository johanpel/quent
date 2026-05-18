// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_attributes::CustomAttributes;
use quent_v2_model::{
    Capacity, Entity, EntityRef, Fixed, Fsm, Occupancy, Resizable, Resource, ResourceGroup,
    RgParentRef, RootResourceGroup, Unbounded, Usage,
};
use serde::{Deserialize, Serialize};

// A "unit" resource.
//
// Only one entity may use this at a time.
//
// Inline doc strings are kept.
/// A thread running tasks.
#[derive(Entity)]
#[quent(resource)]
pub struct Thread;

// A resource with a capacity.
//
// Multiple entities may use this at a time.
//
// A Resource has a pre-defined FSM:
//
// initializing -> operating -> finalizing -> exit
//
// The maximum capacity is set in the transition to operating.
/// A cache holding on to recent things.
#[derive(Entity)]
#[quent(resource)]
pub struct Cache {
    pub slots: Capacity<u64>,
}

// A resource with capacities that are resizable at run-time.
//
// To this end, it will have an additional state compared to fixed-capacity
// resources, for when it's resizing:
//
// initializing -> operating <-> resizing -> finalizing -> exit
/// A memory pool providing space to do things.
#[derive(Entity)]
#[quent(resource)]
pub struct MemoryPool {
    pub bytes: Capacity<u64, Occupancy, Resizable>,
}

// A resource with a potentially unbounded capacity.
//
// In the instrumentation API, in the operating state, no bound is supplied.
/// A queue to enqueue stuff.
#[derive(Entity)]
#[quent(resource)]
pub struct Queue {
    pub entries: Capacity<u64, Occupancy, Fixed, Unbounded>,
}

// A trivial single-event entity.
//
// Note that this is a demonstration of how Quent can even be used to sink
// structured logs.
/// An info message.
#[derive(Entity)]
pub struct Info {
    pub message: String,
    pub source: Option<String>,
}

// Attributes for a multi-event entity.
/// Details of the applied checksum.
#[derive(Serialize, Deserialize)]
pub struct Checksum {
    pub algorithm: String,
    pub value: String,
}

/// Details of the decompression stage.
#[derive(Serialize, Deserialize)]
pub struct Decompressed {
    pub algorithm: String,
    pub ratio: f64,
}

// A multi-event entity.
//
// Events are considered unordered. This is useful for grouping events where
// their timestamps don't have a clear relation (like in FSM state transitions).
// For example, when recording the outcome of two pieces of asynchronous work
// without having to necessarily synchronize within the application (as far as
// emitting these events is concerned).
//
// Each variant is a kind of event. Variants are Once (at most one emission)
// by default; annotate with #[quent(multi)] for zero-or-more semantics.
#[derive(Entity, Serialize, Deserialize)]
pub enum FileStats {
    Checksum(Checksum),
    Decompressed(Decompressed),
}

// Structs with key-value attributes
#[derive(Serialize, Deserialize)]
pub struct Details {
    pub version: String,          // key known at compile-time
    pub custom: CustomAttributes, // for keys known at run-time only
}

// An entity can be marked as a Resource Group.
//
// If it can only have one type of parent T, this is expressed by carrying a
// field of type EntityRef<T, RgParentRef>.
#[derive(Entity, Serialize, Deserialize)]
#[quent(resource_group)]
pub struct Worker {
    pub instance_name: String,
    pub details: Details,
    pub parent: EntityRef<Cluster, RgParentRef>,
}

// There must be at least one root resource group.
#[derive(Entity, Serialize, Deserialize)]
#[quent(resource_group(root))]
pub struct Cluster {
    pub instance_name: String,
}

// A multi-event entity that is also a resource group must carry the parent
// reference on one of its events. The chosen event is the variant whose named
// struct contains an EntityRef<_, RgParentRef> field.
#[derive(Serialize, Deserialize)]
pub struct MyEvent {}

#[derive(Entity, Serialize, Deserialize)]
#[quent(resource_group)]
pub enum Example {
    A {
        event: MyEvent,
        parent: EntityRef<Worker, RgParentRef>,
    },
    B(MyEvent),
}

// Attributes of an FSM state
//
// Can have resource usages.
#[derive(Serialize, Deserialize)]
pub struct Queued {
    pub instance_name: String,
    pub index: u64,
    pub worker: EntityRef<Worker>,
    pub queue: Option<Usage<Queue>>,
}

#[derive(Serialize, Deserialize)]
pub struct Computing {
    pub thread: Option<Usage<Thread>>,
    pub memory: Option<Usage<MemoryPool>>,
}

// An FSM.
//
// Must declare its states, its entry state, the states from which it can exit,
// and its possible transitions. Each enum variant is a state; its payload is
// the state's attributes.
#[derive(Fsm, Serialize, Deserialize)]
#[quent(fsm(
    entry -> Queued,
    Queued -> Computing,
    Computing -> Computing,
    Computing -> exit,
))]
pub enum Task {
    Queued(Queued),
    Computing(Computing),
}
