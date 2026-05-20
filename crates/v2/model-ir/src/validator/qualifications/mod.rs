// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use thiserror::Error;

use crate::entity::Entity;

mod fsm;
mod resource;
mod resource_group;

#[derive(Debug, Error)]
pub enum QualificationError {
    #[error("entity doesn't hold the specified qualification.")]
    NotSpecified,
    #[error("entity fails to qualify: {}", .0.join("\n"))]
    Violations(Vec<String>),
}

/// A Qualifiation represents constraints of entity events.
pub trait QualificationCheck {
    /// Checks whether 'entity` qualifies as [`Self`].
    fn qualifies(entity: &Entity) -> Result<(), QualificationError>;
}
