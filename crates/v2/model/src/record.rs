// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Support for records.
//!
//! Records are application-specific named, structured collections of fields.

/// Trait for records expressible in the Quent IR.
pub trait Record {
    #[cfg(feature = "ir")]
    fn ir() -> quent_v2_model_ir::record::Record;
}
