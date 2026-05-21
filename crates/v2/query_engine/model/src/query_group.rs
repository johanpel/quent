// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model::{Attributes, Entity, entity_ref::EntityRef, resource_group::RgParentRef};

use crate::engine;

#[derive(Attributes)]
pub struct QueryGroupAttributes {
    pub instance_name: String,
}

#[derive(Entity)]
#[quent(resource_group)]
pub enum QueryGroup {
    QueryGroup {
        payload: QueryGroupAttributes,
        parent: EntityRef<engine::Engine, RgParentRef>,
    },
}
