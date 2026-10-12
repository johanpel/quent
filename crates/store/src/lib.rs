// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Typed access to stored data.

pub mod context;
pub mod entity;
pub mod error;
pub mod event;

pub use entity::{EntityHandle, EntityStore, Id};
pub use error::Error;
pub use quent_time::TimeUnixNanoSec;

/// Result returned by store operations.
pub type Result<T> = std::result::Result<T, Error>;
