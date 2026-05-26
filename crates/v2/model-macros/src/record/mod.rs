// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! #[derive(Record)] implementation
//!
//! The `Record` derive macro only requires expansion into IR trait impls for
//! cross-language codegen. It doesn't need to expand into any specific
//! instrumentation-related code for Rust only.

pub(crate) mod ir;
