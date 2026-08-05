// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_dynamic_attributes::DynamicAttributes;
use quent_events::EntityRef;

use crate::operator::Operator;

#[derive(Debug)]
pub struct Port;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub enum PortEvent {
    Declaration {
        operator_id: EntityRef<Operator>,
        instance_name: String,
    },
    Statistics {
        custom_attributes: DynamicAttributes,
    },
}

impl quent_events::EntityEvent for PortEvent {
    const NAME: &'static str = "Port";
}

impl quent_events::Entity for Port {
    type Event = PortEvent;
}
