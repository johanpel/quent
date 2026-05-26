// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Operator entity: a node inside a query plan.

use quent_attributes::CustomAttributes;
use quent_v2_model::{entity_ref::EntityRef, scope::RgParentRef, Entity, Record};

use crate::plan;

#[derive(Record)]
pub struct OperatorDeclaration {
    pub parent_plan_operators: Vec<EntityRef<Operator>>,
    pub instance_name: String,
    pub type_name: String,
    pub custom_attributes: CustomAttributes,
}

#[derive(Record)]
pub struct OperatorStatistics {
    pub custom_attributes: CustomAttributes,
}

#[derive(Entity)]
#[quent(resource_group)]
pub enum Operator {
    Declaration {
        payload: OperatorDeclaration,
        parent: EntityRef<plan::Plan, RgParentRef>,
    },
    Statistics(OperatorStatistics),
}
