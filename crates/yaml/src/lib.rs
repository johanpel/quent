// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! YAML source format for `quent-schema` application event models.
//!
//! This is the foundation slice: located [`Diagnostics`] and the owned YAML
//! tree the format is parsed into, which carries source positions on every
//! node. The format itself (mapping access, the type mini-language, lowering
//! to a schema, and the load API) follows in the next changes.

// The tree is only consumed by the upcoming lowering; removed with it.
#![allow(dead_code)]

mod diag;
mod tree;

pub use diag::{Diagnostic, Diagnostics};
