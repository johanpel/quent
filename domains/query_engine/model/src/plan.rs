// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_events::EntityRef;

use crate::{port::Port, query::Query, worker::Worker};

#[derive(Debug)]
pub struct Plan;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Edge {
    pub source: EntityRef<Port>,
    pub target: EntityRef<Port>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PlanParent {
    pub query_id: EntityRef<Query>,
    pub plan_id: Option<EntityRef<Plan>>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub enum PlanEvent {
    Declaration {
        parent: PlanParent,
        instance_name: String,
        edges: Vec<Edge>,
        worker_id: Option<EntityRef<Worker>>,
    },
}

impl quent_events::EntityEvent for PlanEvent {
    const NAME: &'static str = "Plan";
}

impl quent_events::Entity for Plan {
    type Event = PlanEvent;
}
