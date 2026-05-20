// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(unused)]

use quent_v2_model::Entity;

#[derive(Entity)]
#[quent(fsm(
    entry -> A,
    A -> exit
))]
pub enum OneUnit {
    A,
}

#[derive(Entity)]
#[quent(fsm(
    entry -> A,
    A -> B,
    B -> C,
    C -> exit
))]
pub enum MultiUnit {
    A,
    B,
    C,
}
