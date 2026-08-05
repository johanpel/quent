// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_dynamic_attributes::DynamicAttributes;
use quent_events::EntityRef;

use crate::plan::Plan;

#[derive(Debug)]
pub struct Operator;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub enum OperatorEvent {
    Declaration {
        plan_id: EntityRef<Plan>,
        parent_operator_ids: Vec<EntityRef<Operator>>,
        instance_name: String,
        type_name: String,
        custom_attributes: DynamicAttributes,
    },
    Statistics {
        custom_attributes: DynamicAttributes,
    },
}

impl quent_events::EntityEvent for OperatorEvent {
    const NAME: &'static str = "Operator";
}

impl quent_events::Entity for Operator {
    type Event = OperatorEvent;
}
