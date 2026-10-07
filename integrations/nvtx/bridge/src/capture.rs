// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Convert and forward NVTX records to a Quent observer on a worker thread.
//!
//! Using a worker thread has several benefits:
//!
//! - Text decoding and Quent event construction do not delay NVTX callers.
//! - Observer shutdown stays out of NVTX callbacks. A library may call NVTX
//!   from a global destructor while the application is shutting down. If the
//!   hook takes an observer reference just before the application releases its
//!   own, the callback becomes the last owner. Returning from the callback
//!   then drops the observer and waits for its exporter. Tokio's `block_in_place`
//!   panics on a current-thread runtime, while a call from the exporter thread
//!   deadlocks. The worker owns the observer and performs that wait instead.

use std::io;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use nvtx_injection::Record;
use quent_events::Event;
use quent_instrumentation::ObserverInner;
use quent_nvtx_events::NvtxEvent;
use quent_time::{TimeUnixNanoSec, timestamp};
use thiserror::Error;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

use crate::convert::{convert_with_thread_id, current_thread_id};

struct QueuedRecord {
    record: Record,
    timestamp: TimeUnixNanoSec,
    thread_id: Option<u32>,
}

enum Message {
    Record(QueuedRecord),
    Stop,
}

/// Error starting an NVTX capture.
#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("cannot start the NVTX capture worker: {0}")]
    Spawn(#[from] io::Error),
    #[error(transparent)]
    InstallHook(#[from] nvtx_injection::InstallHookError),
}

/// Forwards NVTX records and flushes the observer on a dedicated worker.
///
/// Installation is one-shot per process, including after this value is dropped.
/// Dropping the capture closes its queue, drains accepted records, and waits for
/// the observer's exporter to flush. Later calls and calls racing with shutdown
/// may be discarded. Do not drop the capture from the observer's exporter thread.
pub struct Capture {
    sender: Arc<UnboundedSender<Message>>,
    worker: Option<JoinHandle<()>>,
}

impl Capture {
    /// Install the NVTX hook and start forwarding records to `observer`.
    ///
    /// The observer's runtime must make progress while capture shutdown waits
    /// for its exporter. Use an owned or multithreaded runtime for the observer.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker cannot start or an NVTX hook was already installed.
    pub fn install(
        session: Uuid,
        observer: ObserverInner<NvtxEvent>,
    ) -> Result<Self, CaptureError> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let sender = Arc::new(sender);
        let worker = thread::Builder::new()
            .name("quent-nvtx-bridge".into())
            .spawn(move || forward(receiver, observer, session))?;

        let capture = Self {
            sender: Arc::clone(&sender),
            worker: Some(worker),
        };
        let weak_sender = Arc::downgrade(&sender);
        nvtx_injection::install_hook(move |record| {
            if let Some(sender) = weak_sender.upgrade() {
                let timestamp = timestamp();
                let thread_id =
                    matches!(&record, Record::RangePush { .. } | Record::RangePop { .. })
                        .then(current_thread_id);
                let _ = sender.send(Message::Record(QueuedRecord {
                    record,
                    timestamp,
                    thread_id,
                }));
            }
        })?;

        Ok(capture)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn forward(
    mut receiver: UnboundedReceiver<Message>,
    observer: ObserverInner<NvtxEvent>,
    session: Uuid,
) {
    let mut batch = Vec::new();
    loop {
        // Drain the current backlog without growing one batch indefinitely if
        // producers keep sending.
        let limit = receiver.len().max(1);
        if receiver.blocking_recv_many(&mut batch, limit) == 0 {
            break;
        }
        for message in batch.drain(..) {
            match message {
                Message::Record(queued) => {
                    observer.send(Event::new(
                        session,
                        queued.timestamp,
                        convert_with_thread_id(queued.record, queued.thread_id),
                    ));
                }
                Message::Stop => {
                    // Closing the receiver lets shutdown finish even if a callback
                    // still holds an upgraded sender.
                    receiver.close();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc as std_mpsc;
    use std::time::Duration;

    use super::*;

    #[test]
    fn shutdown_does_not_wait_for_an_in_flight_sender() {
        let (sender, receiver) = mpsc::unbounded_channel();
        let sender = Arc::new(sender);
        let in_flight_sender = Arc::clone(&sender);
        let worker = thread::spawn(move || {
            forward(receiver, ObserverInner::noop(), Uuid::now_v7());
        });
        let capture = Capture {
            sender,
            worker: Some(worker),
        };
        let (done, finished) = std_mpsc::channel();
        let shutdown = thread::spawn(move || {
            drop(capture);
            done.send(()).unwrap();
        });
        finished
            .recv_timeout(Duration::from_secs(10))
            .expect("shutdown waited for an in-flight callback sender");
        shutdown.join().unwrap();
        assert!(in_flight_sender.send(Message::Stop).is_err());
    }
}
