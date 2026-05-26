// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(unused)]

use quent_v2_model::Entity;

use crate::source::attributes::{MultiNested, OnePrim};

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

#[derive(Entity)]
#[quent(fsm(
    entry -> A,
    A -> exit
))]
pub enum OneAttribs {
    A(OnePrim),
}

#[derive(Entity)]
#[quent(fsm(
    entry -> A,
    A -> B,
    B -> exit
))]
pub enum MultiAttribs {
    A(OnePrim),
    B(MultiNested),
}

#[derive(Entity)]
#[quent(fsm(
    entry -> A,
    A -> A,
    A -> exit
))]
pub enum SelfLoop {
    A,
}

#[derive(Entity)]
#[quent(fsm(
    entry -> A,
    A -> B,
    A -> C,
    B -> C,
    C -> A,
    A -> exit
))]
pub enum Loop {
    A,
    B,
    C,
}
