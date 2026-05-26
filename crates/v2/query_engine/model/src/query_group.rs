// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model::{Entity, Record, entity_ref::EntityRef, scope::RgParentRef};

use crate::engine;

#[derive(Record)]
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
