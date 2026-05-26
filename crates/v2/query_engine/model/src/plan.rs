// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model::{
    AnyRg, Entity, Record, entity_ref::EntityRef, scope::RgParentRef,
};

use crate::port;

#[derive(Record)]
pub struct Edge {
    pub source: EntityRef<port::Port>,
    pub target: EntityRef<port::Port>,
}

#[derive(Record)]
pub struct PlanAttributes {
    pub instance_name: String,
    pub edges: Vec<Edge>,
    pub plan_parent: Option<EntityRef<Plan>>,
}

#[derive(Entity)]
#[quent(resource_group)]
pub enum Plan {
    Declaration {
        payload: PlanAttributes,
        /// Either a worker (worker-local plan instance) or a query (cluster-wide plan).
        parent: EntityRef<AnyRg, RgParentRef>,
    },
}
