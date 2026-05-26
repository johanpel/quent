// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

pub mod attributes;
pub mod entity;
pub mod entity_ref;
pub mod event;
pub mod fsm;
pub mod resource;

// TODO: remove when done
// mod brainstorm;

// user facing exports
pub use entity::{Entity, EntityHandle, ObserverError};
pub use entity_ref::EntityRef;
// pub use fsm::Transition;
// pub use resource::{
//     Bounded, Capacity, CapacityValue, Fixed, Occupancy, OccupancyBound, Rate, RateBound, Resizable,
//     Resource, Unbounded, Usage,
// };

// third party crate re-exporters
pub use quent_exporter as exporter;
pub use quent_v2_model_macros::{Attributes, Entity};
