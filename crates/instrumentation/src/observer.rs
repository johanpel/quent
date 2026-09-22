// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Shared event forwarding state for entity observers.

mod clock;
mod producer;

use crate::context::{Runtime, drive};
use clock::TimestampConverter;
use producer::{ProducerQueue, ThreadProducer, push_thread_event, with_thread_producer};
use quent_events::{EntityEvent, Event};
use quent_io::Exporter;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::warn;
use uuid::Uuid;

static NEXT_PIPELINE_ID: AtomicUsize = AtomicUsize::new(1);

struct EventPipeline<T> {
    id: usize,
    lifetime: Arc<()>,
    producers: Mutex<Vec<Arc<ProducerQueue<T>>>>,
    accepting: AtomicBool,
}

impl<T> EventPipeline<T> {
    fn new() -> Self {
        Self {
            id: NEXT_PIPELINE_ID.fetch_add(1, Ordering::Relaxed),
            lifetime: Arc::new(()),
            producers: Mutex::new(Vec::new()),
            accepting: AtomicBool::new(true),
        }
    }

    fn new_producer(&self) -> ThreadProducer<T> {
        let (queue, producer) = ThreadProducer::create();
        self.producers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(queue);
        producer
    }

    fn producers(&self) -> Vec<Arc<ProducerQueue<T>>> {
        let mut producers = self.producers.lock().unwrap_or_else(|e| e.into_inner());
        producers.retain(|producer| producer.producer_alive() || producer.has_events());
        producers.clone()
    }

    fn stop_accepting(&self) {
        self.accepting.store(false, Ordering::SeqCst);
        // Detached senders do not keep the observer alive. Wait for sends that
        // passed their first accepting check before the final drain begins.
        loop {
            if self
                .producers()
                .iter()
                .all(|producer| !producer.has_active_sends())
            {
                break;
            }
            std::thread::yield_now();
        }
    }
}

/// Wrapper around an optional thread-local event queue.
///
/// When no pipeline is present, as with the noop exporter, [`Self::send`]
/// discards the event without queue or forwarding overhead.
pub struct EventSender<T> {
    pipeline: PipelineRef<T>,
    disable_error_log: Arc<AtomicBool>,
}

enum PipelineRef<T> {
    None,
    Retained(Arc<EventPipeline<T>>),
    Detached(Weak<EventPipeline<T>>),
}

impl<T> std::fmt::Debug for EventSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("EventSender<{}>", std::any::type_name::<T>()))
            .field("active", &!matches!(self.pipeline, PipelineRef::None))
            .field("disable_error_log", &self.disable_error_log)
            .finish()
    }
}

impl<T> Clone for EventSender<T> {
    fn clone(&self) -> Self {
        let pipeline = match &self.pipeline {
            PipelineRef::None => None,
            PipelineRef::Retained(pipeline) => Some(Arc::clone(pipeline)),
            PipelineRef::Detached(pipeline) => pipeline.upgrade(),
        };
        let Some(pipeline) = pipeline else {
            return Self {
                pipeline: PipelineRef::None,
                disable_error_log: Arc::clone(&self.disable_error_log),
            };
        };
        Self {
            pipeline: PipelineRef::Detached(Arc::downgrade(&pipeline)),
            disable_error_log: Arc::clone(&self.disable_error_log),
        }
    }
}

impl<T> EventSender<T> {
    /// Returns a noop sender that silently drops all events.
    pub fn noop() -> Self {
        Self {
            pipeline: PipelineRef::None,
            disable_error_log: Arc::new(AtomicBool::new(true)),
        }
    }

    fn active(pipeline: &Arc<EventPipeline<T>>, disable_error_log: Arc<AtomicBool>) -> Self {
        Self {
            pipeline: PipelineRef::Retained(Arc::clone(pipeline)),
            disable_error_log,
        }
    }

    fn log_send_error(&self) {
        if !self.disable_error_log.swap(true, Ordering::Relaxed) {
            tracing::error!("unable to send event, suppressing further errors");
        }
    }

    #[inline(always)]
    pub fn send(&self, event: Event<T>)
    where
        T: Send + 'static,
    {
        send_active(self, event, false);
    }

    /// Emit an event, converting it into the target type via `Into`.
    #[inline(always)]
    pub fn emit(&self, id: Uuid, event: impl Into<T>)
    where
        T: Send + 'static,
    {
        send_active(self, Event::new(id, clock::capture(), event.into()), true);
    }
}

#[inline(always)]
fn send_active<T>(sender: &EventSender<T>, event: Event<T>, deferred_timestamp: bool)
where
    T: Send + 'static,
{
    if let PipelineRef::Retained(pipeline) = &sender.pipeline {
        push_thread_event(
            pipeline.id,
            &pipeline.lifetime,
            || pipeline.new_producer(),
            event,
            deferred_timestamp,
        );
        return;
    }

    let PipelineRef::Detached(pipeline) = &sender.pipeline else {
        return;
    };
    let Some(pipeline) = pipeline.upgrade() else {
        sender.log_send_error();
        return;
    };
    if !pipeline.accepting.load(Ordering::SeqCst) {
        sender.log_send_error();
        return;
    }

    let sent = with_thread_producer(
        pipeline.id,
        &pipeline.lifetime,
        || pipeline.new_producer(),
        |producer| {
            producer.begin_detached_send();
            let accepting = pipeline.accepting.load(Ordering::SeqCst);
            if accepting {
                producer.push(event, deferred_timestamp);
            }
            producer.end_detached_send();
            accepting
        },
    );
    if !sent {
        sender.log_send_error();
    }
}

/// Backs an entity observer with event forwarding and exporter lifecycle management.
///
/// A context creates one instance per entity type and shares it among that
/// entity's observer and handles. Dropping the final shared owner cancels the
/// forwarder and flushes the exporter.
///
/// Hidden because generated `Observer<E>` and `Handle<E>` types expose this
/// lifecycle.
#[doc(hidden)]
pub struct ObserverInner<T> {
    events_sender: EventSender<T>,
    pipeline: Option<Arc<EventPipeline<T>>>,
    cancellation_token: CancellationToken,
    forwarder_handle: Option<JoinHandle<()>>,
    /// The runtime this pipeline's forwarder runs on; `None` for a no-op
    /// pipeline. An owned runtime stays alive until the pipeline is flushed.
    runtime: Option<Runtime>,
}

impl<T> ObserverInner<T> {
    /// Construct a no-op pipeline that discards events and holds no runtime resources.
    pub fn noop() -> Self {
        Self {
            events_sender: EventSender::noop(),
            pipeline: None,
            cancellation_token: CancellationToken::new(),
            forwarder_handle: None,
            runtime: None,
        }
    }

    /// Send a pre-built event into this stream.
    pub fn send(&self, event: Event<T>)
    where
        T: Send + 'static,
    {
        self.events_sender.send(event);
    }

    /// Emit an event for entity `id`, converting it into the stream type.
    #[inline(always)]
    pub fn emit(&self, id: Uuid, event: impl Into<T>)
    where
        T: Send + 'static,
    {
        self.events_sender.emit(id, event);
    }

    /// Return an independent producer feeding this pipeline.
    ///
    /// The sender does not keep the pipeline alive. Sends after the observer is
    /// dropped are discarded and report at most one error across all clones.
    pub fn sender(&self) -> EventSender<T> {
        self.events_sender.clone()
    }

    pub(crate) fn retained_sender(&self) -> EventSender<T> {
        let PipelineRef::Retained(pipeline) = &self.events_sender.pipeline else {
            return EventSender::noop();
        };
        EventSender {
            pipeline: PipelineRef::Retained(Arc::clone(pipeline)),
            disable_error_log: Arc::clone(&self.events_sender.disable_error_log),
        }
    }
}

impl<T> Drop for ObserverInner<T> {
    fn drop(&mut self) {
        if let Some(pipeline) = &self.pipeline {
            pipeline.stop_accepting();
        }
        self.cancellation_token.cancel();

        let (Some(runtime), Some(forwarder_handle)) = (&self.runtime, self.forwarder_handle.take())
        else {
            return;
        };

        // Cancellation drains every registered producer before shutdown;
        // joining makes that flush synchronous from the caller's perspective.
        if let Err(e) = drive(&runtime.handle(), forwarder_handle) {
            warn!("forwarder task failed: {e}");
        }
    }
}

async fn export_producer<T>(
    exporter: &mut dyn Exporter<T>,
    producer: &ProducerQueue<T>,
    buffer: &mut Vec<Event<T>>,
    clock: &mut TimestampConverter,
) -> bool
where
    T: Send + EntityEvent + 'static,
{
    let limit = exporter.batch_size_hint().get();
    buffer.reserve(limit);
    producer.drain_into(buffer, limit, &mut |timestamp| clock.convert(timestamp));
    if buffer.is_empty() {
        return false;
    }
    if let Err(e) = exporter.drain_events(buffer).await {
        warn!("unable to export events: {e}");
    }
    debug_assert!(
        buffer.is_empty(),
        "drain_events must leave the buffer empty"
    );
    true
}

async fn export_producer_fully<T>(
    exporter: &mut dyn Exporter<T>,
    producer: &ProducerQueue<T>,
    buffer: &mut Vec<Event<T>>,
    clock: &mut TimestampConverter,
) where
    T: Send + EntityEvent + 'static,
{
    while producer.has_events() {
        let _ = export_producer(exporter, producer, buffer, clock).await;
    }
}

/// Spawn the forwarder task for `exporter` on `runtime` and wrap it in an
/// [`ObserverInner`]. The task drains and flushes the exporter on cancellation.
pub(crate) fn spawn_forwarder<T>(
    runtime: &Runtime,
    mut exporter: Box<dyn Exporter<T>>,
) -> ObserverInner<T>
where
    T: Send + EntityEvent + 'static,
{
    let cancellation_token = CancellationToken::new();
    let cloned_token = cancellation_token.clone();
    let pipeline = Arc::new(EventPipeline::new());
    let task_pipeline = Arc::clone(&pipeline);
    let mut clock = TimestampConverter::new();

    let forwarder_handle = runtime.handle().spawn(async move {
        // Exporters leave this allocation empty so it can be reused.
        let mut buffer = Vec::new();
        loop {
            let mut exported = false;
            for producer in task_pipeline.producers() {
                exported |=
                    export_producer(exporter.as_mut(), &producer, &mut buffer, &mut clock).await;
            }
            if cloned_token.is_cancelled() {
                break;
            }
            if !exported {
                tokio::select! {
                    () = cloned_token.cancelled() => break,
                    () = tokio::time::sleep(std::time::Duration::from_micros(100)) => {},
                }
            }
        }
        for producer in task_pipeline.producers() {
            export_producer_fully(exporter.as_mut(), &producer, &mut buffer, &mut clock).await;
        }
        if let Err(e) = exporter.shutdown().await {
            warn!("failed to shut down exporter: {e}");
        }
    });

    let disable_error_log = Arc::new(AtomicBool::new(false));
    ObserverInner {
        events_sender: EventSender::active(&pipeline, disable_error_log),
        pipeline: Some(pipeline),
        cancellation_token,
        forwarder_handle: Some(forwarder_handle),
        runtime: Some(runtime.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quent_io::ExporterResult;
    use std::{num::NonZeroUsize, sync::atomic::AtomicUsize};

    struct TestEvent(usize);
    impl EntityEvent for TestEvent {
        const NAME: &'static str = "TestEvent";
    }

    struct CountingExporter(Arc<AtomicUsize>);

    #[async_trait::async_trait]
    impl Exporter<TestEvent> for CountingExporter {
        async fn push(&mut self, event: Event<TestEvent>) -> ExporterResult<()> {
            assert!(event.data.0 < usize::MAX);
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn batch_size_hint(&self) -> NonZeroUsize {
            NonZeroUsize::new(4).unwrap()
        }

        async fn shutdown(self: Box<Self>) -> ExporterResult<()> {
            Ok(())
        }
    }

    #[test]
    fn noop_observer_holds_no_producer_and_discards_events() {
        let observer = ObserverInner::<TestEvent>::noop();
        observer.emit(Uuid::now_v7(), TestEvent(0));
    }

    #[test]
    fn shutdown_flushes_all_producer_queues() {
        const PRODUCERS: usize = 4;
        const EVENTS_PER_PRODUCER: usize = 1_000;

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let runtime_handle = Runtime::Borrowed(runtime.handle().clone());
        let count = Arc::new(AtomicUsize::new(0));
        let observer = spawn_forwarder(
            &runtime_handle,
            Box::new(CountingExporter(Arc::clone(&count))),
        );
        let senders: Vec<_> = (0..PRODUCERS).map(|_| observer.sender()).collect();
        std::thread::scope(|scope| {
            for sender in &senders {
                scope.spawn(move || {
                    for value in 0..EVENTS_PER_PRODUCER {
                        sender.emit(Uuid::nil(), TestEvent(value));
                    }
                });
            }
        });

        drop(observer);
        assert_eq!(
            count.load(Ordering::Relaxed),
            PRODUCERS * EVENTS_PER_PRODUCER
        );

        senders[0].emit(Uuid::nil(), TestEvent(0));
        assert_eq!(
            count.load(Ordering::Relaxed),
            PRODUCERS * EVENTS_PER_PRODUCER
        );
    }
}
