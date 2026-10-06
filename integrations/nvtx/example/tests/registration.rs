// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! A failed installation must not replace the process's hook.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use quent_instrumentation::EventCallback;
use uuid::Uuid;

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

    let result = nvtx_example::run_capture(Uuid::now_v7(), EventCallback::new(|_| {}));
    assert!(result.unwrap_err().is::<nvtx_injection::InstallHookError>());

    nvtx::mark(c"after duplicate");
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    drop(calls);
    nvtx::mark(c"after shutdown");
    assert!(weak_calls.upgrade().is_none());
}
