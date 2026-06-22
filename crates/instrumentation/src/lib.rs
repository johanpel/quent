// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Quent Instrumentation API
//!
use quent_build_info::{ArtifactInfo, ModelSource};
use quent_events::{EntityEvent, Event};
use quent_exporter::{ExporterOptions, create_exporter};
use serde::Serialize;
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

/// The runtime an active context's observers run on. `Ambient` borrows a runtime
/// that already existed (e.g. a `#[tokio::main]` or caller-managed one); `Owned`
/// keeps alive the one this context spawned.
#[derive(Clone)]
enum ActiveRuntime {
    Ambient(Handle),
    Owned(Arc<Runtime>),
}

impl ActiveRuntime {
    /// The handle observers spawn and block on.
    fn handle(&self) -> Handle {
        match self {
            Self::Ambient(h) => h.clone(),
            Self::Owned(rt) => rt.handle().clone(),
        }
    }
}

/// What a context does with events. `Noop` drops them; `Active` carries the
/// exporter configuration and the runtime its observers run on.
enum Backend {
    Noop,
    Active {
        config: ExporterOptions,
        runtime: ActiveRuntime,
    },
}

pub struct Context {
    /// Identity of this context, generated on construction.
    id: Uuid,
    backend: Backend,
}

impl Context {
    /// Create a context for the given `exporter` configuration (see
    /// [`ExporterOptions`]).
    ///
    /// `M` defines the model provenance, which is written into a sidecar file
    /// alongside events when `exporter` is a filesystem exporter variant.
    ///
    /// # Errors
    ///
    /// Returns an error when an async runtime is needed but cannot be obtained.
    pub fn try_new<M: ModelSource>(
        exporter: Option<ExporterOptions>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::try_with_id::<M>(Uuid::now_v7(), exporter)
    }

    /// Like [`Self::try_new`], but adopts an existing `id`.
    pub fn try_with_id<M: ModelSource>(
        id: Uuid,
        exporter: Option<ExporterOptions>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let Some(config) = exporter else {
            debug!("using noop exporter");
            return Ok(Context {
                id,
                backend: Backend::Noop,
            });
        };

        let runtime = if let Ok(handle) = Handle::try_current() {
            debug!("using existing async runtime");
            ActiveRuntime::Ambient(handle)
        } else {
            debug!("spawning new async runtime");
            let runtime =
                Runtime::new().map_err(|e| format!("unable to spawn async runtime: {e}"))?;
            ActiveRuntime::Owned(Arc::new(runtime))
        };

        // Write the provenance sidecar once per context, in the context
        // directory the per-entity observers nest their streams under.
        // Filesystem exporters only.
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
            backend: Backend::Active { config, runtime },
        })
    }

    /// Identity of this context, generated or set on construction.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Create an [`Observer`] of events of one *type* of entity `T`.
    ///
    /// # Errors
    ///
    /// Returns an error if the exporter cannot be constructed.
    pub fn observer<T>(&self) -> Result<Observer<T>, Box<dyn std::error::Error>>
    where
        T: Serialize + Send + EntityEvent + 'static,
    {
        let Backend::Active { config, runtime } = &self.backend else {
            return Ok(Observer::noop());
        };
        let handle = runtime.handle();

        debug!("constructing exporter for stream `{}`", T::NAME);
        let kind = config.clone().in_context_dir(self.id);
        let exporter = handle.block_on(create_exporter::<T>(kind))?;

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
            runtime: Some(runtime.clone()),
        })
    }
}

/// Observes events of one *type* of entity `T`.
///
/// It does so by dealing out (generated) handles per entity. These handles have
/// functions with names and signatures generated from an application event
/// model. Upon calling these functions, events are sent to an exporter that is
/// managed by this observer.
pub struct Observer<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    events_sender: EventSender<T>,
    cancellation_token: CancellationToken,
    forwarder_handle: Option<JoinHandle<()>>,
    /// The runtime this observer's forwarder runs on; `None` for a no-op
    /// observer. An `Owned` runtime is kept alive here for the observer's
    /// lifetime, so its drop flush is valid even after the [`Context`] is gone.
    runtime: Option<ActiveRuntime>,
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
            runtime: None,
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

        let (Some(runtime), Some(forwarder_handle)) = (&self.runtime, self.forwarder_handle.take())
        else {
            return;
        };
        let handle = runtime.handle();

        // The forwarder drains remaining events and flushes the exporter on
        // cancellation; joining only waits for that to finish. `block_on`
        // panics on a runtime worker thread, so when dropped inside a runtime
        // detach the join instead — the flush still runs, but drop no longer
        // waits for it.
        if Handle::try_current().is_ok() {
            handle.spawn(async move {
                let _ = forwarder_handle.await;
            });
        } else if let Err(e) = handle.block_on(forwarder_handle) {
            warn!("forwarder task failed: {e}");
        }
    }
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
        let ctx = Context::try_new::<TestModel>(None).unwrap();
        assert!(matches!(ctx.backend, Backend::Noop));

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
        let ctx = Context::try_new::<TestModel>(Some(ExporterOptions::FileSystem(
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
