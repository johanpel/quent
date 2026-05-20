// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[derive(Debug, PartialEq, Eq)]
pub struct Resource;

/// IR of marking an entity reference is to be used by entities to qualify as a
/// Resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceRefKind {
    /// The reference is referring to the parent resource group.
    Parent,
}
