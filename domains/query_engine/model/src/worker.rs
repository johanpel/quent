// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_events::EntityRef;

use crate::engine::Engine;

#[derive(Debug)]
pub struct Worker;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub enum WorkerEvent {
    Init {
        parent_engine_id: EntityRef<Engine>,
        instance_name: String,
    },
    Exit,
}

impl quent_events::EntityEvent for WorkerEvent {
    const NAME: &'static str = "Worker";
}

impl quent_events::Entity for Worker {
    type Event = WorkerEvent;
}
