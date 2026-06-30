// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Backing structures for generated instrumentation libraries.
//!
//! Instrumented application code should not import this crate directly unless
//! there is a very special reason. Instead, it should interact with the
//! generated instrumentation library only.

use quent_build_info::{ArtifactInfo, ModelInfo};
use quent_exporter::{ExporterOptions, create_exporter};
use serde::Serialize;

// Re-export everything generated instrumentation code references, so a consumer
// needs only the `quent-instrumentation` dependency. Exporter
// implementations are the one exception: those come from `quent-exporter`.
pub use quent_attributes::CustomAttributes;
pub use quent_build_info as build_info;
pub use quent_events::{EntityEvent, Event};
use std::future::Future;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::{
    runtime::{Handle as RuntimeHandle, Runtime},
    sync::mpsc::{UnboundedSender, unbounded_channel},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
pub use uuid::Uuid;

/// Reference from one entity instance to another by id, optionally carrying
/// payload data `T`.
///
/// Placeholder backing the schema generator's `DataType::EntityRef` fields.
// TODO(johanpel): flesh out ref-target semantics (see `quent.ref-target.v1`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EntityRef<T = ()> {
    /// Id of the referenced entity instance.
    pub target: Uuid,
    /// Payload carried alongside the reference.
    pub data: T,
}

/// Wrapper around an optional channel sender.
///
/// When the inner sender is `None` (i.e. the noop exporter is selected), `send`
/// is a no-op that avoids any channel or event-forwarding overhead.
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

/// The runtime an active context's observers run on.
#[derive(Clone)]
enum BackendRuntime {
    /// A handle to a runtime owned elsewhere (`#[tokio::main]`, a caller-managed
    /// one) and kept alive by that owner.
    Borrowed(RuntimeHandle),
    /// The runtime this context spawned, shared by the context and every observer
    /// (hence `Arc`) and shut down by the last holder's `Drop`.
    Owned {
        handle: RuntimeHandle,
        /// `Option` only so `Drop` can move the `Arc` out of `&mut self`; `Some`
        /// for the value's whole life until then.
        runtime: Option<Arc<Runtime>>,
    },
}

impl BackendRuntime {
    /// The handle observers spawn and block on.
    fn handle(&self) -> RuntimeHandle {
        match self {
            Self::Borrowed(handle) | Self::Owned { handle, .. } => handle.clone(),
        }
    }
}

impl Drop for BackendRuntime {
    fn drop(&mut self) {
        // On the last holder of a spawned runtime, shut it down without blocking,
        // since a blocking `Runtime` drop panics on a runtime worker thread.
        // `into_inner` yields the `Runtime` only when this was the final `Arc`.
        // Safe to abandon tasks here: the observers' forwarders have already
        // flushed by the time the last holder drops.
        if let Self::Owned { runtime, .. } = self
            && let Some(runtime) = runtime.take().and_then(Arc::into_inner)
        {
            runtime.shutdown_background();
        }
    }
}

/// What a context does with events. `Noop` drops them; `Active` carries the
/// exporter configuration and the runtime its observers run on.
enum Backend {
    Noop,
    Active {
        config: ExporterOptions,
        runtime: BackendRuntime,
    },
}

/// A context responsible for providing an asynchronous back-end to a
/// synchronous context generated from an application event model.
///
/// Instrumented application code should not interact with this type directly
/// unless there is a very special reason. Instead, it should interact with the
/// generated context only through a fully synchronous API.
///
/// What it is responsible for:
/// - Resolving the runtime its observers run on. It borrows an ambient one if
///   present, otherwise spawns its own (see [`BackendRuntime`]).
/// - Writing the provenance sidecar at construction.
/// - Being the single sync→async bridge for async observer + exporter
///   construction and the drop-time flush.
///
/// # Panics
///
/// The blocking sync/async crossings work off a runtime or on a multi-threaded
/// one, but panic on a current-thread runtime.
#[doc(hidden)]
pub struct Context {
    /// Unique identifier of this context.
    id: Uuid,
    /// The asynchronous run-time the observers produced by this context operate
    /// on.
    backend: Backend,
}

impl Context {
    pub fn try_new(
        model: ModelInfo,
        exporter: Option<ExporterOptions>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::try_with_id(Uuid::now_v7(), model, exporter)
    }

    /// Construct a new context with the supplied universally unique identifier.
    pub fn try_with_id(
        id: Uuid,
        model: ModelInfo,
        exporter: Option<ExporterOptions>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let Some(config) = exporter else {
            debug!("using noop exporter");
            return Ok(Context {
                id,
                backend: Backend::Noop,
            });
        };

        let runtime = if let Ok(handle) = RuntimeHandle::try_current() {
            debug!("using existing async runtime");
            BackendRuntime::Borrowed(handle)
        } else {
            debug!("spawning new async runtime");
            let runtime =
                Runtime::new().map_err(|e| format!("unable to spawn async runtime: {e}"))?;
            BackendRuntime::Owned {
                handle: runtime.handle().clone(),
                runtime: Some(Arc::new(runtime)),
            }
        };

        let context = Context {
            id,
            backend: Backend::Active { config, runtime },
        };
        context.write_sidecar(model);
        Ok(context)
    }

    /// Write the model provenance sidecar file into the context's filesystem
    /// exporter directory.
    ///
    /// If no filesystem exporter is configured, then this is a no-op.
    fn write_sidecar(&self, model: ModelInfo) {
        let Backend::Active { config, .. } = &self.backend else {
            return;
        };
        let kind = config.clone().resolve(self.id);
        let Some(root) = kind.filesystem_root() else {
            return;
        };
        if let Err(e) = std::fs::create_dir_all(root)
            .and_then(|()| ArtifactInfo::new(model).write_sidecar(root))
        {
            warn!("failed to write provenance sidecar: {e}");
        }
    }

    /// Return the universally unique identifier of this context.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Drive `fut` to completion on this context's runtime, blocking the
    /// calling thread.
    ///
    /// # Panics
    ///
    /// Panics on a current-thread runtime.
    pub fn block_on<F: Future>(&self, fut: F) -> F::Output {
        match &self.backend {
            Backend::Active { runtime, .. } => drive(&runtime.handle(), fut),
            // A noop context has no runtime, but its async work is immediately
            // ready, so poll once. Invariant: the noop `observer()` future must
            // never pend (it early-returns before any `.await`). The
            // `unreachable!` below enforces it.
            Backend::Noop => {
                let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
                match std::pin::pin!(fut).poll(&mut cx) {
                    std::task::Poll::Ready(v) => v,
                    std::task::Poll::Pending => {
                        unreachable!("noop context future is always ready")
                    }
                }
            }
        }
    }

    /// Create an [`Observer`] of events of one *type* of entity `T`.
    ///
    /// # Errors
    ///
    /// Returns an error if the exporter cannot be constructed.
    pub async fn observer<T>(&self) -> Result<Observer<T>, Box<dyn std::error::Error>>
    where
        T: Serialize + Send + EntityEvent + 'static,
    {
        let Backend::Active { config, runtime } = &self.backend else {
            return Ok(Observer::noop());
        };
        let handle = runtime.handle();

        debug!("constructing exporter for stream `{}`", T::NAME);
        let kind = config.clone().resolve(self.id);
        let mut exporter = create_exporter::<T>(kind).await?;

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
            // Tear down once, however the loop exited.
            if let Err(e) = exporter.shutdown().await {
                warn!("failed to shut down exporter: {e}");
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
    runtime: Option<BackendRuntime>,
}

impl<T> Observer<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    /// Construct a no-op observer that discards events and holds no runtime
    /// resources whatesoever.
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

        // The forwarder drains remaining events and flushes the exporter on
        // cancellation; joining waits for that to finish. `drive` blocks here
        // whether dropped off a runtime or on a multi-threaded worker.
        if let Err(e) = drive(&runtime.handle(), forwarder_handle) {
            warn!("forwarder task failed: {e}");
        }
    }
}

/// An error from emitting through a [`Handle`].
#[derive(Debug, thiserror::Error)]
pub enum ObserverError {
    /// A once-cardinality event was emitted more than once for one entity
    /// instance.
    #[error("once-event `{event}` already emitted for this entity instance")]
    OnceAlreadyEmitted {
        /// Name of the event that was re-emitted.
        event: &'static str,
    },
}

/// A handle to one entity instance: emits that instance's events through a
/// shared [`Observer`], enforcing once-cardinality events at most once.
///
/// Holds a shared reference to the observer's export pipeline, keeping it alive
/// while any handle does. Not `Clone`: the once-emit state is unique to one
/// instance, so a clone could re-emit a once-event.
#[doc(hidden)]
pub struct Handle<E>
where
    E: Serialize + Send + EntityEvent + 'static,
{
    id: Uuid,
    /// One bit per once-cardinality event, set once that event is emitted.
    once_flags: u64,
    observer: Arc<Observer<E>>,
}

impl<E> Handle<E>
where
    E: Serialize + Send + EntityEvent + 'static,
{
    /// Create a handle for a fresh entity instance, with a generated id.
    pub fn new(observer: Arc<Observer<E>>) -> Self {
        Self::with_id(Uuid::now_v7(), observer)
    }

    /// Create a handle for the entity instance identified by `id`.
    pub fn with_id(id: Uuid, observer: Arc<Observer<E>>) -> Self {
        Self {
            id,
            once_flags: 0,
            observer,
        }
    }

    /// The entity instance id this handle emits for.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Emit a multi-cardinality event for this instance.
    pub fn emit(&self, event: E) {
        self.observer.emit(self.id, event);
    }

    /// Emit a once-cardinality event, tracked by its `bit` index.
    ///
    /// Returns [`ObserverError::OnceAlreadyEmitted`] if this handle already
    /// emitted the event; otherwise records and emits it. `bit` must be below
    /// 64.
    pub fn emit_once(
        &mut self,
        bit: u32,
        event_name: &'static str,
        event: E,
    ) -> Result<(), ObserverError> {
        let mask = 1u64 << bit;
        if self.once_flags & mask != 0 {
            return Err(ObserverError::OnceAlreadyEmitted { event: event_name });
        }
        self.once_flags |= mask;
        self.observer.emit(self.id, event);
        Ok(())
    }
}

/// Drive `fut` to completion on `handle`'s runtime, blocking the current thread.
///
/// Off a runtime, it blocks directly. On a multi-threaded runtime worker it
/// uses `block_in_place` so the scheduler keeps progressing.
///
/// # Panics
/// On a current-thread runtime, this panics.
fn drive<F: Future>(handle: &RuntimeHandle, fut: F) -> F::Output {
    if RuntimeHandle::try_current().is_ok() {
        tokio::task::block_in_place(|| handle.block_on(fut))
    } else {
        handle.block_on(fut)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quent_build_info::ModelSource;
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
        let ctx = Context::try_new(TestModel::model_info(), None).unwrap();
        assert!(matches!(ctx.backend, Backend::Noop));

        let observer = ctx.block_on(ctx.observer::<TestEvent>()).unwrap();
        assert!(observer.events_sender.tx.is_none());

        observer.send(Event::new_now(Uuid::now_v7(), TestEvent));
        observer.emit(Uuid::now_v7(), TestEvent);
        drop(observer);
        drop(ctx);
    }

    #[test]
    fn filesystem_observer_writes_under_entity_subdir_with_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = Context::try_new(
            TestModel::model_info(),
            Some(ExporterOptions::FileSystem(FileSystemExporterOptions {
                format: FileSystemFormat::Ndjson,
                root: dir.path().to_path_buf(),
            })),
        )
        .unwrap();

        let context_dir = dir.path().join(ctx.id().to_string());

        {
            let observer = ctx.block_on(ctx.observer::<TestEvent>()).unwrap();
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

    #[test]
    fn handle_emit_once_rejects_repeat_per_bit() {
        let ctx = Context::try_new(TestModel::model_info(), None).unwrap();
        let observer = Arc::new(ctx.block_on(ctx.observer::<TestEvent>()).unwrap());
        let mut handle = Handle::new(observer);

        assert!(handle.emit_once(0, "ev", TestEvent).is_ok());
        assert!(matches!(
            handle.emit_once(0, "ev", TestEvent),
            Err(ObserverError::OnceAlreadyEmitted { event: "ev" })
        ));
        // A different bit tracks independently.
        assert!(handle.emit_once(1, "other", TestEvent).is_ok());
        // Multi emit is unconditional.
        handle.emit(TestEvent);
    }
}
