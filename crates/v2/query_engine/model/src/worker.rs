// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Worker FSM: a node-local execution context, child of Engine.

use quent_v2_model::{Attributes, Entity, entity_ref::EntityRef, scope::RgParentRef};

use crate::engine;

#[derive(Attributes)]
pub struct WorkerInit {
    pub instance_name: String,
}

#[derive(Entity)]
#[quent(
    fsm(entry -> Init, Init -> exit),
    resource_group,
)]
pub enum Worker {
    Init {
        payload: WorkerInit,
        parent: EntityRef<engine::Engine, RgParentRef>,
    },
}
