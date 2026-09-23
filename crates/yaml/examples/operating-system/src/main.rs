// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/operating_system.rs"));
}

use instrumentation::{Context, Noop, OperatingSystem, Process, Thread};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::<OperatingSystem>::try_new(Noop)?;

    let mut process = context.observer::<Process>().handle();
    // This is the native OS process ID, not the Quent entity ID.
    process.started(instrumentation::quent::os::Process {
        native_id: std::process::id(),
    })?;

    let process = process.as_entity_ref();
    let mut thread = context.observer::<Thread>().handle();
    std::thread::spawn(move || {
        // This is the worker's native OS thread ID, not the Quent entity ID.
        let native_id = current_native_thread_id()?;
        thread
            .started(instrumentation::quent::os::Thread { native_id }, process)
            .map_err(std::io::Error::other)
    })
    .join()
    .unwrap()?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn current_native_thread_id() -> std::io::Result<u64> {
    // SAFETY: `gettid` takes no arguments and returns the caller's kernel task ID.
    let native_id = unsafe { libc::syscall(libc::SYS_gettid) };
    if native_id < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(native_id as u64)
    }
}

#[cfg(target_os = "macos")]
fn current_native_thread_id() -> std::io::Result<u64> {
    let mut native_id = 0;
    // SAFETY: A null thread selects the calling thread, and `native_id` is writable.
    let result = unsafe { libc::pthread_threadid_np(0, &mut native_id) };
    if result == 0 {
        Ok(native_id)
    } else {
        Err(std::io::Error::from_raw_os_error(result))
    }
}

#[cfg(windows)]
fn current_native_thread_id() -> std::io::Result<u64> {
    // SAFETY: `GetCurrentThreadId` has no preconditions.
    Ok(unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() }.into())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn current_native_thread_id() -> std::io::Result<u64> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "native thread IDs are unsupported on this platform",
    ))
}
