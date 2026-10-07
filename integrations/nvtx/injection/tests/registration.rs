// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! A failed installation must not replace the process's hook.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn failed_installation_does_not_replace_existing_hook() {
    let calls = Arc::new(AtomicUsize::new(0));
    let weak_calls = Arc::downgrade(&calls);
    nvtx_injection::install_hook({
        let calls = weak_calls.clone();
        move |_| {
            if let Some(calls) = calls.upgrade() {
                calls.fetch_add(1, Ordering::Relaxed);
            }
        }
    })
    .unwrap();
    nvtx::mark(c"before duplicate");
    assert_eq!(calls.load(Ordering::Relaxed), 1);

    assert!(matches!(
        nvtx_injection::install_hook(|_| {}),
        Err(nvtx_injection::InstallHookError::AlreadyInstalled)
    ));

    nvtx::mark(c"after duplicate");
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    drop(calls);
    nvtx::mark(c"after shutdown");
    assert!(weak_calls.upgrade().is_none());
}
