// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Custom Quent channel implementations.

mod mpsc;
mod spsc;

pub use mpsc::{Receiver, Sender, unbounded_channel};
pub use spsc::Config;
