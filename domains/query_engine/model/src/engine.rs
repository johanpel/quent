// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_dynamic_attributes::DynamicAttributes;

#[derive(Debug)]
pub struct Engine;

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct EngineImplementationAttributes {
    pub name: Option<String>,
    pub version: Option<String>,
    pub custom_attributes: DynamicAttributes,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub enum EngineEvent {
    Init {
        implementation: EngineImplementationAttributes,
        instance_name: Option<String>,
    },
    Exit,
}

impl quent_events::EntityEvent for EngineEvent {
    const NAME: &'static str = "Engine";
}

impl quent_events::Entity for Engine {
    type Event = EngineEvent;
}
