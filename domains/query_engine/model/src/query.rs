// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_events::EntityRef;

use crate::query_group::QueryGroup;

#[derive(Debug)]
pub struct Query;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub enum QueryEvent {
    Init {
        query_group_id: EntityRef<QueryGroup>,
        instance_name: String,
    },
    Planning,
    Executing,
    Exit,
}

impl quent_events::EntityEvent for QueryEvent {
    const NAME: &'static str = "Query";
}

impl quent_events::Entity for Query {
    type Event = QueryEvent;
}
