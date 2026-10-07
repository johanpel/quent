// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Capture shutdown must leave a single-worker Tokio runtime able to flush.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use quent_instrumentation::{ContextInner, EventCallback};
use quent_nvtx_bridge::Capture;
use quent_nvtx_events::NvtxEvent;
use uuid::Uuid;

#[test]
fn capture_drop_on_only_runtime_worker_flushes() {
    let (done, finished) = mpsc::channel();
    let runtime_thread = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        runtime.block_on(async {
            tokio::spawn({
                let calls = Arc::clone(&calls);
                async move {
                    let session = Uuid::now_v7();
                    let context = ContextInner::try_new(session).unwrap();
                    let exporter = EventCallback::<NvtxEvent>::new(move |_| {
                        calls.fetch_add(1, Ordering::Relaxed);
                    });
                    let observer = context.observer::<NvtxEvent>(&exporter).await.unwrap();
                    let capture = Capture::install(session, observer).unwrap();
                    nvtx::mark(c"single worker");
                    drop(capture);
                }
            })
            .await
            .unwrap();
        });
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        done.send(()).unwrap();
    });
    finished
        .recv_timeout(Duration::from_secs(10))
        .expect("capture shutdown blocked the only runtime worker");
    runtime_thread.join().unwrap();
}
