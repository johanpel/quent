// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Port entity: an input/output of an operator.

use quent_v2_model::{Attributes, Entity, entity_ref::EntityRef, resource_group::RgParentRef};

use crate::operator;

#[derive(Attributes)]
pub struct PortDeclaration {
    pub operator: EntityRef<operator::Operator>,
    pub instance_name: String,
}

#[derive(Attributes)]
pub struct PortStatistics {
    pub custom_attributes: quent_attributes::CustomAttributes,
}

#[derive(Entity)]
#[quent(resource_group)]
pub enum Port {
    Declaration {
        payload: PortDeclaration,
        parent: EntityRef<operator::Operator, RgParentRef>,
    },
    Statistics(PortStatistics),
}
