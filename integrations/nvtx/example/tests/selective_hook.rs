// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Late hook installation selects paired event groups without changing NVTX returns.

use std::sync::{Arc, Mutex};

use nvtx::sys::ffi;
use nvtx_injection::{HookOptions, RawEvent};

#[test]
fn late_selection_keeps_callbacks_and_skips_unselected_events() {
    // Initialize NVTX before configuring the hook.
    let early_range = unsafe { ffi::nvtxRangeStartA(c"early".as_ptr()) };
    assert_ne!(early_range, 0);
    unsafe { ffi::nvtxRangeEnd(early_range) };

    let events = Arc::new(Mutex::new(Vec::new()));
    nvtx_injection::install_hook_with_options(
        {
            let events = Arc::clone(&events);
            move |event| events.lock().unwrap().push(event)
        },
        HookOptions {
            marks: true,
            push_pop: true,
            ..HookOptions::default()
        },
    )
    .unwrap();

    unsafe {
        ffi::nvtxMarkA(c"selected".as_ptr());
        assert_eq!(ffi::nvtxRangePushA(c"selected range".as_ptr()), 0);
        assert_eq!(ffi::nvtxRangePop(), 0);

        let range = ffi::nvtxRangeStartA(c"unselected range".as_ptr());
        assert_ne!(range, 0);
        ffi::nvtxRangeEnd(range);

        let domain = ffi::nvtxDomainCreateA(c"unselected domain".as_ptr());
        assert!(!domain.is_null());
        ffi::nvtxDomainDestroy(domain);
    }

    let events = events.lock().unwrap();
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], RawEvent::Mark { .. }));
    assert!(matches!(events[1], RawEvent::RangePush { .. }));
    assert!(matches!(events[2], RawEvent::RangePop { .. }));
}
