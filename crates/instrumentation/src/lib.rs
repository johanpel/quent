// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Quent Instrumentation API
//!
use quent_build_info::{ArtifactInfo, ModelSource};
use quent_events::{EntityEvent, Event};
use quent_exporter::{ExporterOptions, create_exporter};
use serde::Serialize;
use std::marker::PhantomData;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::{
    runtime::{Handle, Runtime},
    sync::mpsc::{UnboundedSender, unbounded_channel},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use uuid::Uuid;

/// Wrapper around an optional channel sender. When the inner sender is `None`
/// (i.e. the noop exporter is selected), `send` is a no-op that avoids any
/// channel or event-forwarding overhead.
pub struct EventSender<T> {
    tx: Option<UnboundedSender<Event<T>>>,
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
            disable_error_log: Arc::clone(&self.disable_error_log),
        }
    }
}

impl<T> Default for EventSender<T> {
    fn default() -> Self {
        Self::noop()
    }
}

impl<T> EventSender<T> {
    /// Returns a noop sender that silently drops all events.
    pub fn noop() -> Self {
        Self {
            tx: None,
            disable_error_log: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn send(&self, event: Event<T>) {
        if let Some(tx) = &self.tx
            && tx.send(event).is_err()
            && !self.disable_error_log.swap(true, Ordering::Relaxed)
        {
            tracing::error!("unable to send event, suppressing further errors");
        }
    }

    /// Emit an event, converting it into the target type via `Into`.
    pub fn emit(&self, id: Uuid, event: impl Into<T>) {
        self.send(Event::new_now(id, event.into()));
    }
}

/// Identity and runtime owner for a model's instrumentation. Generates the
/// context id, owns (or locates) the async runtime, and writes the model
/// provenance sidecar once on construction. Mints one [`Observer`] per entity
/// event stream via [`Self::observer`].
///
/// `M` is the model marker whose [`ModelSource`] impl supplies the `model.qmi`
/// provenance. It does not appear in the event streams, which are typed per
/// entity by the [`Observer`]s.
pub struct Context<M> {
    /// Identity of this context, generated on construction.
    id: Uuid,
    /// Exporter configuration cloned into each observer it builds; `None` is a
    /// no-op context that creates no runtime.
    config: Option<ExporterOptions>,
    handle: Option<Handle>,
    /// Shared with every [`Observer`] this context builds so the runtime
    /// outlives them regardless of drop order; `None` when running on a
    /// caller-provided (ambient) runtime.
    runtime: Option<Arc<Runtime>>,
    _model: PhantomData<M>,
}

impl<M> Context<M> {
    /// Create a context. With `Some(exporter)` this locates or spawns a runtime
    /// and writes the `model.qmi` provenance sidecar into the context directory;
    /// with `None` it is a no-op context that creates no runtime and whose
    /// observers discard events.
    ///
    /// # Errors
    /// Returns an error if no runtime is available and one cannot be spawned. A
    /// failure to write the sidecar is logged, not returned.
    pub fn try_new(exporter: Option<ExporterOptions>) -> Result<Self, Box<dyn std::error::Error>>
    where
        M: ModelSource,
    {
        Self::try_with_id(Uuid::now_v7(), exporter)
    }

    /// Like [`Self::try_new`] but adopts an existing `id` instead of generating
    /// one — used when reproducing another context's output (e.g. the collector
    /// writing a remote source's streams under that source's context id).
    ///
    /// # Errors
    /// Returns an error if no runtime is available and one cannot be spawned. A
    /// failure to write the sidecar is logged, not returned.
    pub fn try_with_id(
        id: Uuid,
        exporter: Option<ExporterOptions>,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        M: ModelSource,
    {
        let Some(config) = exporter else {
            debug!("using noop exporter");
            return Ok(Context {
                id,
                config: None,
                handle: None,
                runtime: None,
                _model: PhantomData,
            });
        };

        let (runtime, handle) = if let Ok(handle) = Handle::try_current() {
            debug!("using existing async runtime");
            (None, handle)
        } else {
            debug!("spawning new async runtime");
            if let Ok(runtime) = Runtime::new() {
                let handle = runtime.handle().clone();
                (Some(Arc::new(runtime)), handle)
            } else {
                return Err("unable to spawn async runtime")?;
            }
        };

        // Write the provenance sidecar once per context, in the context
        // directory the per-entity observers nest their streams under.
        // Filesystem exporters only; the collector server writes it for
        // collector-routed events.
        if let Some(dir) = config.clone().in_context_dir(id).filesystem_root() {
            let dir = dir.to_path_buf();
            if let Err(e) = std::fs::create_dir_all(&dir)
                .and_then(|()| ArtifactInfo::new(M::model_info()).write_sidecar(&dir))
            {
                warn!("failed to write provenance sidecar: {e}");
            }
        }

        Ok(Context {
            id,
            config: Some(config),
            handle: Some(handle),
            runtime,
            _model: PhantomData,
        })
    }

    /// Identity of this context, generated on construction.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Build the pipeline for one entity's event stream `T`. Filesystem events
    /// are written to `<root>/<id>/<T::NAME>/`. Returns a no-op observer when
    /// the context has no exporter configured.
    ///
    /// # Errors
    /// Returns an error if the exporter cannot be constructed.
    pub fn observer<T>(&self) -> Result<Observer<T>, Box<dyn std::error::Error>>
    where
        T: Serialize + Send + EntityEvent + Into<M> + 'static,
        M: Serialize + Send + 'static,
    {
        let (Some(config), Some(handle)) = (&self.config, &self.handle) else {
            return Ok(Observer::noop());
        };

        debug!("constructing exporter for stream `{}`", T::NAME);
        let kind = config.clone().in_context_dir(self.id);
        // The forwarder task owns the exporter outright; no sharing, no `Arc`.
        // `M` is the model's umbrella event, the collector's wire type.
        let exporter = handle.block_on(create_exporter::<T, M>(kind))?;

        let cancellation_token = CancellationToken::new();
        let cloned_token = cancellation_token.clone();
        let (events_sender, mut events_receiver) = unbounded_channel();

        let forwarder_handle = handle.spawn(async move {
            loop {
                tokio::select! {
                    Some(event) = events_receiver.recv() => {
                        if let Err(e) = exporter.push(event).await {
                            warn!("unable to export event: {e}");
                        }
                    },
                    () = cloned_token.cancelled() => {
                        events_receiver.close();
                        // drain events that are buffered
                        while let Some(event) = events_receiver.recv().await {
                            if let Err(e) = exporter.push(event).await {
                                warn!("unable to export event: {e}");
                            }
                        }
                        break
                    },
                    // the events channel has been closed: nothing left to forward.
                    else => break,
                }
            }
            // Flush once on shutdown, however the loop exited.
            if let Err(e) = exporter.force_flush().await {
                warn!("failed to flush exporter: {e}");
            }
        });

        Ok(Observer {
            events_sender: EventSender {
                tx: Some(events_sender),
                disable_error_log: Arc::new(AtomicBool::new(false)),
            },
            cancellation_token,
            forwarder_handle: Some(forwarder_handle),
            handle: Some(handle.clone()),
            _runtime: self.runtime.clone(),
        })
    }
}

/// One entity's event stream. It owns the channel and the forwarder task (which
/// in turn owns the exporter) plus a share of the runtime, so it operates
/// independently of the [`Context`] that built it (typically held behind an
/// `Arc` and shared across the application). Emit through [`Self::send`] /
/// [`Self::emit`]. On drop it cancels and waits (via `block_on`) for the
/// forwarder to drain buffered events and flush the exporter, so it must be
/// dropped where [`Handle::block_on`] is valid (not on a runtime worker thread).
pub struct Observer<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    events_sender: EventSender<T>,
    cancellation_token: CancellationToken,
    forwarder_handle: Option<JoinHandle<()>>,
    handle: Option<Handle>,
    /// Keeps the runtime alive for as long as this observer lives, so its drop
    /// flush is valid even after the [`Context`] is gone. `None` for a no-op
    /// observer or on an ambient runtime.
    _runtime: Option<Arc<Runtime>>,
}

impl<T> Observer<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    /// A no-op observer that discards events and holds no runtime resources.
    fn noop() -> Self {
        Self {
            events_sender: EventSender::noop(),
            cancellation_token: CancellationToken::new(),
            forwarder_handle: None,
            handle: None,
            _runtime: None,
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
}

impl<T> Drop for Observer<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    fn drop(&mut self) {
        self.cancellation_token.cancel();

        // Wait for the forwarder to drain remaining events and flush the
        // exporter (it owns the exporter and flushes on shutdown).
        if let Some(handle) = &self.handle
            && let Some(forwarder_handle) = self.forwarder_handle.take()
            && let Err(e) = handle.block_on(forwarder_handle)
        {
            warn!("forwarder task failed: {e}");
        }
    }
}

/// A local model context that reproduces a remote source's output by feeding
/// its observers with received umbrella events. Implemented by generated
/// `{App}Context` types; the routing lives in this trait impl, keeping the
/// context's inherent API a pure local-production type.
pub trait CollectorContext: Sized {
    /// The model's umbrella event type carried on the wire.
    type Event;

    /// Build a context that reproduces the source identified by `id`.
    fn with_source_id(
        id: Uuid,
        exporter: Option<ExporterOptions>,
    ) -> Result<Self, Box<dyn std::error::Error>>;

    /// Route one received umbrella event to the matching entity observer.
    fn feed(&self, event: Event<Self::Event>);
}

#[cfg(test)]
mod tests {
    use super::*;
    use quent_exporter::{FileSystemExporterOptions, FileSystemFormat};

    #[derive(Debug, serde::Serialize)]
    struct TestModel(TestEvent);

    impl From<TestEvent> for TestModel {
        fn from(e: TestEvent) -> Self {
            TestModel(e)
        }
    }

    impl ModelSource for TestModel {
        fn package() -> &'static str {
            "quent-instrumentation"
        }
        fn source() -> quent_build_info::BuildInfo {
            quent_build_info::BuildInfo::unknown()
        }
    }

    #[derive(Debug, serde::Serialize)]
    struct TestEvent;

    impl EntityEvent for TestEvent {
        const NAME: &'static str = "TestEvent";
    }

    #[test]
    fn noop_context_creates_noop_observer() {
        let ctx = Context::<TestModel>::try_new(None).unwrap();
        assert!(ctx.handle.is_none());
        assert!(ctx.runtime.is_none());

        let observer = ctx.observer::<TestEvent>().unwrap();
        assert!(observer.events_sender.tx.is_none());

        observer.send(Event::new_now(Uuid::now_v7(), TestEvent));
        observer.emit(Uuid::now_v7(), TestEvent);
        drop(observer);
        drop(ctx);
    }

    #[test]
    fn filesystem_observer_writes_under_entity_subdir_with_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = Context::<TestModel>::try_new(Some(ExporterOptions::FileSystem(
            FileSystemExporterOptions {
                format: FileSystemFormat::Ndjson,
                root: dir.path().to_path_buf(),
            },
        )))
        .unwrap();

        let context_dir = dir.path().join(ctx.id().to_string());

        {
            let observer = ctx.observer::<TestEvent>().unwrap();
            observer.send(Event::new_now(Uuid::now_v7(), TestEvent));
            // Drop the observer to drain and flush before asserting.
        }

        assert!(
            context_dir.join("model.qmi").is_file(),
            "sidecar should sit in the context directory"
        );
        let ndjson_files: Vec<_> = std::fs::read_dir(context_dir.join("TestEvent"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("ndjson"))
            .collect();
        assert_eq!(
            ndjson_files.len(),
            1,
            "one UUID-named ndjson batch file in the entity subdirectory"
        );
    }
}
