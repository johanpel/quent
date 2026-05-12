// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

mod entity;
mod fsm;
mod resource;
mod resource_group;

// TODO: remove when done
mod brainstorm;

// user facing exports
pub use entity::{
    AnyEntity, EntityDeclaration, EntityHandle, EntityRef, Event, IntoErased, ObserverError,
    RegularRef,
};
pub use fsm::Transition;
pub use resource::{
    Bounded, Capacity, CapacityValue, Fixed, Occupancy, OccupancyBound, Rate, RateBound, Resizable,
    Resource, Unbounded, Usage,
};
pub use resource_group::{AnyRg, ResourceGroupDeclaration, RgParentRef};

// third party crate re-exporters
pub use quent_exporter as exporter;
pub use quent_v2_model_macros::{Entity, Fsm, Resource, ResourceGroup, RootResourceGroup};
