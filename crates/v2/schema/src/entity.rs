// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{convention::Convention, event::Event, identifier::Identifier};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Entity {
    /// The name of the entity.
    pub name: Identifier,
    /// Potential documentation.
    pub docs: Option<String>,
    /// The events that this entity can emit.
    pub events: Vec<Event>,
    /// Convention-specific metadata attached to this entity.
    pub conventions: Vec<Convention>,
}
