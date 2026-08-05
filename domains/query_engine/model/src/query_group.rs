// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_events::EntityRef;

use crate::engine::Engine;

#[derive(Debug)]
pub struct QueryGroup;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub enum QueryGroupEvent {
    Declaration {
        instance_name: String,
        engine_id: EntityRef<Engine>,
    },
}

impl quent_events::EntityEvent for QueryGroupEvent {
    const NAME: &'static str = "QueryGroup";
}

impl quent_events::Entity for QueryGroup {
    type Event = QueryGroupEvent;
}
