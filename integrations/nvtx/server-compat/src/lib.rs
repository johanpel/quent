// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Preserve the `nvtx-server` API for pinned `quent-open` viewers.

pub use quent_nvtx_server::*;

#[cfg(test)]
mod tests {
    #[test]
    fn viewer_entry_points_keep_their_names() {
        let _ = super::import_context_events;
        let _ = super::routes;
    }
}
