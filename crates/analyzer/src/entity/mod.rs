// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Entity analysis interfaces and storage implementations.

use quent_time::TimeUnixNanoSec;
use uuid::Uuid;

pub mod native;

/// Analysis-time entity.
pub trait Entity {
    /// Return the universally unique identifier of this entity.
    fn id(&self) -> Uuid;
    /// The type name of this entity.
    fn type_name(&self) -> &str;
    /// Returns the earliest observed event timestamp.
    fn earliest_timestamp(&self) -> TimeUnixNanoSec;
    /// Returns the latest observed event timestamp.
    fn latest_timestamp(&self) -> TimeUnixNanoSec;
}
