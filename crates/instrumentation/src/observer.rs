// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The per-entity event pipeline that forwards events to an exporter.

use crate::context::{BackendRuntime, drive};
use quent_events::{EntityEvent, Event};
use quent_io::Exporter;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::{
    sync::mpsc::{Sender, channel},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::warn;
use uuid::Uuid;

const DEFAULT_EVENT_CHANNEL_CAPACITY: usize = 4096;

/// Wrapper around an optional channel sender.
///
/// When the inner sender is `None` (i.e. the noop exporter is selected), `send`
/// is a no-op that avoids any channel or event-forwarding overhead.
pub struct EventSender<T> {
    tx: Option<Sender<Event<T>>>,
    sink: Option<Arc<dyn Fn(Event<T>) + Send + Sync>>,
    producer_flush: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Flag shared across clones to prevent potentially massive log spam from
    /// subseQUENT sender errors after the first.
    disable_error_log: Arc<AtomicBool>,
}

impl<T> std::fmt::Debug for EventSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("EventSender<{}>", std::any::type_name::<T>()))
            .field("tx", &self.tx.as_ref().map(|_| ".."))
            .field("disable_error_log", &self.disable_error_log)
            .finish()
    }
}

impl<T> Clone for EventSender<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            sink: self.sink.clone(),
            producer_flush: self.producer_flush.clone(),
            disable_error_log: Arc::clone(&self.disable_error_log),
        }
    }
}

impl<T> EventSender<T> {
    /// Returns a noop sender that silently drops all events.
    pub fn noop() -> Self {
        Self {
            tx: None,
            sink: None,
            producer_flush: None,
            disable_error_log: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn send(&self, event: Event<T>) {
        if let Some(sink) = &self.sink {
            sink(event);
            return;
        }
        let send_failed = self.tx.as_ref().is_some_and(|tx| {
            let event = match tx.try_send(event) {
                Ok(()) => return false,
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => return true,
                Err(tokio::sync::mpsc::error::TrySendError::Full(event)) => event,
            };
            let result = if tokio::runtime::Handle::try_current().is_ok() {
                tokio::task::block_in_place(|| tx.blocking_send(event))
            } else {
                tx.blocking_send(event)
            };
            result.is_err()
        });
        if send_failed && !self.disable_error_log.swap(true, Ordering::Relaxed) {
            tracing::error!("unable to send event, suppressing further errors");
        }
    }

    fn flush_producer(&self) {
        if let Some(flush) = &self.producer_flush {
            flush();
        }
    }

    /// Emit an event, converting it into the target type via `Into`.
    pub fn emit(&self, id: Uuid, event: impl Into<T>) {
        self.send(Event::new_now(id, event.into()));
    }
}

/// Provides an event pipeline to "observe" events of one *type* of entity `T`
/// and export them.
///
/// Instrumented application code should not interact with this type directly
/// unless they have a very special reason. Instead, it interacts with the
/// generated observer only.
///
/// Generated code constructs and shares this type. Instrumented application
/// code uses the generated observer and its per-instance entity handles
/// instead. Those manage the shared ownership and flush-on-last-drop this type
/// relies on, so holding or dropping it directly can lose or prematurely flush
/// events.
#[doc(hidden)]
pub struct Observer<T> {
    events_sender: EventSender<T>,
    cancellation_token: CancellationToken,
    forwarder_handle: Option<JoinHandle<()>>,
    /// The runtime this observer's forwarder runs on; `None` for a no-op
    /// observer. An `Owned` runtime is kept alive here for the observer's
    /// lifetime, so its drop flush is valid even after the [`Context`] is gone.
    ///
    /// [`Context`]: crate::Context
    runtime: Option<BackendRuntime>,
    sink_shutdown: std::sync::Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl<T> Observer<T> {
    /// Construct a no-op observer that discards events and holds no runtime
    /// resources whatesoever.
    pub fn noop() -> Self {
        Self {
            events_sender: EventSender::noop(),
            cancellation_token: CancellationToken::new(),
            forwarder_handle: None,
            runtime: None,
            sink_shutdown: std::sync::Mutex::new(None),
        }
    }

    /// Construct an observer whose event transformation happens synchronously
    /// on the producer and whose sink is flushed by `shutdown` on drop.
    #[doc(hidden)]
    pub fn from_sink(
        sink: Arc<dyn Fn(Event<T>) + Send + Sync>,
        producer_flush: Arc<dyn Fn() + Send + Sync>,
        shutdown: impl FnOnce() + Send + 'static,
    ) -> Self {
        Self {
            events_sender: EventSender {
                tx: None,
                sink: Some(sink),
                producer_flush: Some(producer_flush),
                disable_error_log: Arc::new(AtomicBool::new(true)),
            },
            cancellation_token: CancellationToken::new(),
            forwarder_handle: None,
            runtime: None,
            sink_shutdown: std::sync::Mutex::new(Some(Box::new(shutdown))),
        }
    }

    /// Send a pre-built event into this stream.
    pub fn send(&self, event: Event<T>) {
        self.events_sender.send(event);
    }

    /// Emit an event for entity `id`, converting it into the stream type.
    pub fn emit(&self, id: Uuid, event: impl Into<T>) {
        self.events_sender.emit(id, event);
    }

    pub(crate) fn flush_producer(&self) {
        self.events_sender.flush_producer();
    }
}

impl<T> Drop for Observer<T> {
    fn drop(&mut self) {
        self.events_sender.flush_producer();
        if let Some(shutdown) = self.sink_shutdown.get_mut().unwrap().take() {
            shutdown();
        }
        self.cancellation_token.cancel();

        let (Some(runtime), Some(forwarder_handle)) = (&self.runtime, self.forwarder_handle.take())
        else {
            return;
        };

        // The forwarder drains remaining events and flushes the exporter on
        // cancellation; joining waits for that to finish. `drive` blocks here
        // whether dropped off a runtime or on a multi-threaded worker.
        if let Err(e) = drive(&runtime.handle(), forwarder_handle) {
            warn!("forwarder task failed: {e}");
        }
    }
}

/// Spawn the forwarder task for `exporter` on `runtime` and wrap it in an
/// [`Observer`]. The task drains and flushes the exporter on cancellation.
pub(crate) fn spawn_forwarder<T>(
    runtime: &BackendRuntime,
    mut exporter: Box<dyn Exporter<T>>,
) -> Observer<T>
where
    T: Send + EntityEvent + 'static,
{
    let channel_capacity = std::env::var("QUENT_EVENT_CHANNEL_CAPACITY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&capacity| capacity > 0)
        .unwrap_or(DEFAULT_EVENT_CHANNEL_CAPACITY);
    let cancellation_token = CancellationToken::new();
    let cloned_token = cancellation_token.clone();
    let (events_sender, mut events_receiver) = channel(channel_capacity);

    let forwarder_handle = runtime.handle().spawn(async move {
        // Reused across batches; `drain_events` leaves it empty and it is reserved
        // to a full batch before each receive.
        let mut buffer = Vec::new();
        loop {
            let limit = exporter.batch_size_hint().get();
            buffer.reserve(limit);
            tokio::select! {
                // Cancel-safe: if the cancellation branch wins, `buffer` is left
                // untouched (no events are lost).
                n = events_receiver.recv_many(&mut buffer, limit) => {
                    // 0 means the channel is closed and drained.
                    if n == 0 {
                        break;
                    }
                    if let Err(e) = exporter.drain_events(&mut buffer).await {
                        warn!("unable to export events: {e}");
                    }
                    debug_assert!(buffer.is_empty(), "drain_events must leave the buffer empty");
                },
                () = cloned_token.cancelled() => {
                    events_receiver.close();
                    // drain events that are buffered
                    loop {
                        let limit = exporter.batch_size_hint().get();
                        buffer.reserve(limit);
                        if events_receiver.recv_many(&mut buffer, limit).await == 0 {
                            break;
                        }
                        if let Err(e) = exporter.drain_events(&mut buffer).await {
                            warn!("unable to export events: {e}");
                        }
                        debug_assert!(buffer.is_empty(), "drain_events must leave the buffer empty");
                    }
                    break
                },
            }
        }
        // Tear down once, however the loop exited.
        if let Err(e) = exporter.shutdown().await {
            warn!("failed to shut down exporter: {e}");
        }
    });

    Observer {
        events_sender: EventSender {
            tx: Some(events_sender),
            sink: None,
            producer_flush: None,
            disable_error_log: Arc::new(AtomicBool::new(false)),
        },
        cancellation_token,
        forwarder_handle: Some(forwarder_handle),
        runtime: Some(runtime.clone()),
        sink_shutdown: std::sync::Mutex::new(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestEvent;
    impl EntityEvent for TestEvent {
        const NAME: &'static str = "TestEvent";
    }

    #[test]
    fn noop_observer_holds_no_sender_and_discards_events() {
        let observer = Observer::<TestEvent>::noop();
        assert!(observer.events_sender.tx.is_none());
        // Emitting is a silent no-op.
        observer.emit(Uuid::now_v7(), TestEvent);
    }
}
