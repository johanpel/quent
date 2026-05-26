// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model::{Entity, Record};

#[derive(Record)]
pub struct EngineImplementationAttributes {
    pub name: Option<String>,
    pub version: Option<String>,
    pub custom_attributes: quent_attributes::CustomAttributes,
}

#[derive(Record)]
pub struct Init {
    pub implementation: EngineImplementationAttributes,
    pub instance_name: Option<String>,
}

#[derive(Entity)]
#[quent(
    fsm(entry -> Init, Init -> exit),
    resource_group(root),
)]
pub enum Engine {
    Init(Init),
}
