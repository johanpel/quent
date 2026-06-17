// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! A gRPC-based server that reproduces each remote source's local output.
//!
//! Each source streams its umbrella events tagged with a `source-context-id`.
//! The server runs one local `{App}Context` (a [`CollectorContext`]) per source
//! id, feeding it the received events so it reproduces, on the server's own
//! filesystem, exactly what the source produced locally (including the
//! provenance sidecar the local context writes itself).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use quent_events::Event;
use quent_exporter::ExporterOptions;
use quent_instrumentation::CollectorContext;
use serde::Deserialize;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status, Streaming};
use tracing::{error, warn};
use uuid::Uuid;

use quent_collector_proto as proto;

#[derive(Debug, Clone)]
pub struct CollectorServiceOptions {
    pub exporter: ExporterOptions,
}

/// Local contexts keyed by source context id; one per remote source.
type Contexts<C> = Arc<RwLock<HashMap<Uuid, Arc<C>>>>;

// Centralizes telemetry from distributed sources by reproducing each source's
// local production through a per-source local context.
//
// TODO(johanpel): clean up contexts after timeout or source end.
pub struct CollectorService<C> {
    contexts: Contexts<C>,
    exporter: ExporterOptions,
}

impl<C> std::fmt::Debug for CollectorService<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollectorService")
            .field("exporter", &self.exporter)
            .finish()
    }
}

impl<C> CollectorService<C> {
    pub fn new(options: CollectorServiceOptions) -> Self {
        Self {
            contexts: Default::default(),
            exporter: options.exporter,
        }
    }
}

#[tonic::async_trait]
impl<C> proto::collector_server::Collector for CollectorService<C>
where
    C: CollectorContext + Send + Sync + 'static,
    for<'de> C::Event: Deserialize<'de>,
{
    #[tracing::instrument]
    async fn collect_events(
        &self,
        request: Request<Streaming<proto::CollectEventRequest>>,
    ) -> Result<Response<proto::CollectEventResponse>, Status> {
        // The source identifies its stream with the `source-context-id` metadata.
        let source_context_id = request
            .metadata()
            .get("source-context-id")
            .ok_or_else(|| Status::invalid_argument("missing `source-context-id` metadata"))?
            .to_str()
            .ok()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| {
                Status::invalid_argument("`source-context-id` metadata is not a valid UUID")
            })?;

        let mut stream = request.into_inner();
        let contexts = Arc::clone(&self.contexts);
        let exporter = self.exporter.clone();
        let export_join_handle = tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                match item {
                    Ok(request) => {
                        // Reuse this source's context, or create it lazily on the
                        // first batch. The read guard is dropped before any work
                        // that could block; neither guard is held across `.await`.
                        let cached = contexts.read().unwrap().get(&source_context_id).cloned();
                        let context = if let Some(context) = cached {
                            context
                        } else {
                            // The context builds its observers with `block_on`,
                            // which would panic on a runtime worker thread, so
                            // construct it on a blocking thread.
                            let exporter = exporter.clone();
                            // The error type is not `Send`, so stringify it on
                            // the blocking thread before crossing the boundary.
                            let built = tokio::task::spawn_blocking(move || {
                                C::with_source_id(source_context_id, Some(exporter))
                                    .map_err(|e| e.to_string())
                            })
                            .await;
                            let context = match built {
                                Ok(Ok(context)) => Arc::new(context),
                                Ok(Err(e)) => {
                                    error!("unable to construct local context: {e}");
                                    break;
                                }
                                Err(e) => {
                                    error!("local context construction panicked: {e}");
                                    break;
                                }
                            };
                            contexts
                                .write()
                                .unwrap()
                                .insert(source_context_id, Arc::clone(&context));
                            context
                        };

                        tracing::trace_span!("deserializing", num_events = request.event.len())
                            .in_scope(|| {
                                for serialized_event in request.event {
                                    match ciborium::from_reader::<Event<C::Event>, _>(
                                        &serialized_event[..],
                                    ) {
                                        Ok(event) => context.feed(event),
                                        Err(e) => {
                                            warn!("collector: deserialization error: {e}")
                                        }
                                    }
                                }
                            });
                    }
                    Err(err) => {
                        warn!("collector: stream error: {err:?}");
                        // TODO(johanpel): a source disconnecting (abruptly?) may result in entering this branch.
                        // The context's observers flush on drop (via `block_on`),
                        // so drop it on a blocking thread, not a runtime worker.
                        let removed = contexts.write().unwrap().remove(&source_context_id);
                        if let Some(context) = removed {
                            let _ = tokio::task::spawn_blocking(move || drop(context)).await;
                        }
                        break;
                    }
                }
            }
            // On normal stream completion the context stays cached; its
            // observers flush when the context is eventually dropped.
        });
        let _ = export_join_handle.await;
        Ok(Response::new(proto::CollectEventResponse {}))
    }
}
