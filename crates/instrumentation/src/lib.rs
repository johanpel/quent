// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Quent Instrumentation API
//!
use quent_build_info::{ArtifactInfo, ModelSource};
use quent_events::{EntityEvent, Event};
use quent_exporter::{ExporterOptions, create_exporter};
use quent_exporter_types::Exporter;
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
    /// Exporter configuration cloned into each observer's pipeline; `None` is a
    /// no-op context that creates no runtime.
    config: Option<ExporterOptions>,
    handle: Option<Handle>,
    _model: PhantomData<M>,

    // The runtime is the last field so it is dropped last (see
    // https://doc.rust-lang.org/reference/destructors.html), after the
    // observers it spawned forwarder tasks for have been dropped and drained.
    _runtime: Option<Runtime>,
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
        let id = Uuid::now_v7();

        let Some(config) = exporter else {
            debug!("using noop exporter");
            return Ok(Context {
                id,
                config: None,
                handle: None,
                _model: PhantomData,
                _runtime: None,
            });
        };

        let (runtime, handle) = if let Ok(handle) = Handle::try_current() {
            debug!("using existing async runtime");
            (None, handle)
        } else {
            debug!("spawning new async runtime");
            if let Ok(runtime) = Runtime::new() {
                let handle = runtime.handle().clone();
                (Some(runtime), handle)
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
            _model: PhantomData,
            _runtime: runtime,
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
        T: Serialize + Send + EntityEvent + 'static,
    {
        let (Some(config), Some(handle)) = (&self.config, &self.handle) else {
            return Ok(Observer::noop());
        };

        debug!("constructing exporter for stream `{}`", T::NAME);
        let kind = config.clone().in_context_dir(self.id);
        let exporter: Arc<dyn Exporter<T>> = handle.block_on(create_exporter(kind))?;

        let cancellation_token = CancellationToken::new();
        let cloned_token = cancellation_token.clone();
        let (events_sender, mut events_receiver) = unbounded_channel();

        let forwarder_handle = handle.spawn({
            let exporter: Arc<dyn Exporter<T>> = Arc::clone(&exporter);
            async move {
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
            }
        });

        Ok(Observer {
            events_sender: EventSender {
                tx: Some(events_sender),
                disable_error_log: Arc::new(AtomicBool::new(false)),
            },
            exporter: Some(exporter),
            cancellation_token,
            forwarder_handle: Some(forwarder_handle),
            handle: Some(handle.clone()),
        })
    }
}

/// One entity's event pipeline: channel → forwarder task → exporter. Hand out
/// cloned senders with [`Self::sender`] and emit through them concurrently. On
/// drop, drains buffered events and flushes the exporter, using the runtime
/// [`Handle`] it was built with — so an observer must be dropped where
/// [`Handle::block_on`] is valid and while that runtime is still alive.
pub struct Observer<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    events_sender: EventSender<T>,
    exporter: Option<Arc<dyn Exporter<T>>>,
    cancellation_token: CancellationToken,
    forwarder_handle: Option<JoinHandle<()>>,
    handle: Option<Handle>,
}

impl<T> Observer<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    /// A no-op observer that discards events and holds no runtime resources.
    fn noop() -> Self {
        Self {
            events_sender: EventSender::noop(),
            exporter: None,
            cancellation_token: CancellationToken::new(),
            forwarder_handle: None,
            handle: None,
        }
    }

    /// A cloned [`EventSender`] for this stream; cheap and `Send + Sync + Clone`.
    pub fn sender(&self) -> EventSender<T> {
        self.events_sender.clone()
    }
}

impl<T> Drop for Observer<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    fn drop(&mut self) {
        self.cancellation_token.cancel();

        if let Some(handle) = &self.handle {
            // Wait for the forwarder to finish processing remaining events.
            if let Some(forwarder_handle) = self.forwarder_handle.take()
                && let Err(e) = handle.block_on(forwarder_handle)
            {
                warn!("forwarder task failed: {e}");
            }

            // Flush the exporter to ensure all events are sent.
            if let Some(exporter) = &self.exporter
                && let Err(e) = handle.block_on(exporter.force_flush())
            {
                warn!("failed to flush exporter: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quent_exporter::{FileSystemExporterOptions, FileSystemFormat};

    #[derive(Debug)]
    struct TestModel;

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
        assert!(ctx._runtime.is_none());

        let observer = ctx.observer::<TestEvent>().unwrap();
        let sender = observer.sender();
        assert!(sender.tx.is_none());

        sender.send(Event::new_now(Uuid::now_v7(), TestEvent));
        sender.send(Event::new_now(Uuid::now_v7(), TestEvent));
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
            let sender = observer.sender();
            sender.send(Event::new_now(Uuid::now_v7(), TestEvent));
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
