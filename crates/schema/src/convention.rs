// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Convention {
    /// The name of the convention.
    pub name: String,
    /// Whether the convention is one that requires validation.
    pub validated: bool,
    /// Convention-specific metadata.
    pub data: Option<String>,
}
