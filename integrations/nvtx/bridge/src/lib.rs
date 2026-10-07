// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The thin bridge that lets captured NVTX events flow through Quent's typed
//! event pipeline.
//!
//! It is one adapter: [`NvtxEventEntity`], a newtype over [`NvtxEvent`]
//! implementing Quent's [`EventPayload`]. The orphan rule forbids that impl in
//! either of *their* crates, so it lives here.
//!
//! See `integrations/nvtx/example` for a complete, runnable capture.
//! Record conversion is available with the `capture` feature.

use quent_events::EventPayload;
use quent_nvtx_events::NvtxEvent;
use serde::{Deserialize, Serialize};

#[cfg(feature = "capture")]
mod convert;

/// A `#[serde(transparent)]` newtype over [`NvtxEvent`] implementing
/// [`EventPayload`], naming the `"NvtxEvent"` entity stream. Transparent, so its
/// serialized form is identical to a bare [`NvtxEvent`].
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct NvtxEventEntity(pub NvtxEvent);

impl EventPayload for NvtxEventEntity {
    const NAME: &'static str = "NvtxEvent";
}

impl From<NvtxEvent> for NvtxEventEntity {
    fn from(event: NvtxEvent) -> Self {
        Self(event)
    }
}
