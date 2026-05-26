// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{IrError, entity::Entity};

mod fsm;
mod resource;

/// A Qualifiation represents constraints of entity events.
pub trait QualificationCheck {
    /// Checks whether 'entity` qualifies as [`Self`].
    fn qualifies(entity: &Entity) -> Result<(), IrError>;
}
