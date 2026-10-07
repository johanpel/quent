// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Converts owned NVTX injection records into Quent NVTX events during capture.

#[cfg(feature = "capture")]
mod convert;

#[cfg(feature = "capture")]
pub use convert::convert;
