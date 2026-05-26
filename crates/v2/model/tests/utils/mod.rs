// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(unused)]

use quent_v2_model_ir::identifier::Identifier;
use uuid::Uuid;

pub const ROOT: Uuid = Uuid::nil();

pub fn ident(s: &str) -> Identifier {
    Identifier::new_unchecked(s)
}

#[macro_export]
macro_rules! rust_path {
    ($name:expr) => {
        format!("{}::{}", module_path!(), $name)
    };
}
pub use crate::rust_path;
