// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model::{Entity, entity_ref::EntityRef, scope::RgParentRef};

use crate::query_group;

#[derive(Entity)]
#[quent(
    fsm(
        entry -> Init,
        Init -> Planning,
        Planning -> Executing,
        Executing -> exit,
    ),
    resource_group,
)]
pub enum Query {
    Init {
        parent: EntityRef<query_group::QueryGroup, RgParentRef>,
    },
    Planning,
    Executing,
}
