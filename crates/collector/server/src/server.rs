// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! A gRPC-based server that collects `Event`s from multiple sources and exports them.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use quent_build_info::{ArtifactInfo, ModelSource};
use quent_events::EntityEvent;
use quent_exporter::{ExporterOptions, create_exporter};
use quent_exporter_types::Exporter;
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status, Streaming};
use tracing::{error, warn};
use uuid::Uuid;

use quent_collector_proto as proto;

#[derive(Debug, Clone)]
pub struct CollectorServiceOptions {
    pub exporter: ExporterOptions,
}

/// Exporters shared across streams, keyed by `application-id`.
type Exporters<T> = Arc<RwLock<HashMap<Uuid, Arc<dyn Exporter<T>>>>>;

// Simple service to centralize telemetry from distributed clients
//
// TODO(johanpel): clean up exporter after timeout or application end.
pub struct CollectorService<T> {
    // Exporters shared across streams, keyed by `application-id` so concurrent
    // sources of the same application consolidate into one exporter.
    exporters: Exporters<T>,
    exporter: ExporterOptions,
}

impl<T> std::fmt::Debug for CollectorService<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollectorService")
            .field("exporter", &self.exporter)
            .finish()
    }
}

impl<T> CollectorService<T> {
    pub fn new(options: CollectorServiceOptions) -> Self {
        Self {
            exporters: Default::default(),
            exporter: options.exporter,
        }
    }
}

#[tonic::async_trait]
impl<T> proto::collector_server::Collector for CollectorService<T>
where
    for<'de> T: Serialize + Deserialize<'de> + Send + ModelSource + EntityEvent + 'static,
{
    #[tracing::instrument]
    async fn collect_events(
        &self,
        request: Request<Streaming<proto::CollectEventRequest>>,
    ) -> Result<Response<proto::CollectEventResponse>, Status> {
        // The client identifies its stream with the `application-id` metadata.
        let application_id = request
            .metadata()
            .get("application-id")
            .ok_or_else(|| Status::invalid_argument("missing `application-id` metadata"))?
            .to_str()
            .ok()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| {
                Status::invalid_argument("`application-id` metadata is not a valid UUID")
            })?;

        let mut stream = request.into_inner();
        let exporters = Arc::clone(&self.exporters);
        // Group each application's events under its own subdirectory.
        let exporter_kind = self.exporter.clone().in_context_dir(application_id);
        let export_join_handle = tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                match item {
                    Ok(request) => {
                        // Reuse this application's exporter, or create it lazily
                        // on the first batch (so an empty stream writes nothing).
                        // The read guard is dropped before any `.await`.
                        let cached = exporters.read().unwrap().get(&application_id).cloned();
                        let exporter = if let Some(exporter) = cached {
                            exporter
                        } else {
                            // The server shares one exporter per application across
                            // its stream handlers, so wrap the owned exporter in an
                            // `Arc` here — that sharing is local to the collector.
                            let exporter: Arc<dyn Exporter<T>> =
                                match create_exporter::<T>(exporter_kind.clone()).await {
                                    Ok(exporter) => Arc::from(exporter),
                                    Err(e) => {
                                        error!("unable to construct exporter: {e}");
                                        break;
                                    }
                                };
                            // Write the provenance sidecar once per application,
                            // alongside the events this exporter just created
                            // the directory for (filesystem exporters only).
                            if let Some(dir) = exporter_kind.filesystem_root() {
                                let info = ArtifactInfo::new(T::model_info());
                                if let Err(e) = info.write_sidecar(dir) {
                                    warn!("failed to write provenance sidecar: {e}");
                                }
                            }
                            exporters
                                .write()
                                .unwrap()
                                .insert(application_id, Arc::clone(&exporter));
                            exporter
                        };

                        let mut events = Vec::with_capacity(request.event.len());
                        tracing::trace_span!("deserializing", num_events = request.event.len())
                            .in_scope(|| {
                                for serialized_event in request.event {
                                    match ciborium::from_reader(&serialized_event[..]) {
                                        Ok(event) => events.push(event),
                                        Err(e) => {
                                            warn!("collector: deserialization error: {e}")
                                        }
                                    }
                                }
                            });

                        tracing::trace_span!("exporting")
                            .in_scope(async || {
                                for event in events {
                                    match exporter.push(event).await {
                                        Ok(_) => (), // successfully exported
                                        Err(e) => {
                                            warn!("collector: unable to export: {e}")
                                        }
                                    }
                                }
                            })
                            .await;
                    }
                    Err(err) => {
                        warn!("collector: stream error: {err:?}");
                        // TODO(johanpel): a client disconnecting (abruptly?) may result in entering this branch.
                        // We should clean up here, but the todo is to figure out what else can go wrong.
                        let exporter = exporters.read().unwrap().get(&application_id).cloned();
                        if let Some(exporter) = exporter {
                            if let Err(e) = exporter.force_flush().await {
                                warn!("unable to flush exporter: {e}");
                            }
                            exporters.write().unwrap().remove(&application_id);
                        }
                        break;
                    }
                }
            }

            // Flush the exporter when stream ends normally
            let exporter = exporters.read().unwrap().get(&application_id).cloned();
            if let Some(exporter) = exporter
                && let Err(e) = exporter.force_flush().await
            {
                warn!("unable to flush exporter after stream completion: {e}");
            }
        });
        let _ = export_join_handle.await;
        Ok(Response::new(proto::CollectEventResponse {}))
    }
}
