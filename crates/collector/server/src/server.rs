// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! A gRPC-based server that collects `Event`s from multiple sources and exports them.

use std::sync::Arc;

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

// Simple service to centralize telemetry from distributed clients
//
// TODO(johanpel): clean up exporter after timeout or application end.
pub struct CollectorService<T> {
    exporter: ExporterOptions,
    _phantom: std::marker::PhantomData<fn() -> T>,
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
            exporter: options.exporter,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[tonic::async_trait]
impl<T> proto::collector_server::Collector for CollectorService<T>
where
    for<'de> T: Serialize + Deserialize<'de> + Send + 'static,
{
    #[tracing::instrument]
    async fn collect_events(
        &self,
        request: Request<Streaming<proto::CollectEventRequest>>,
    ) -> Result<Response<proto::CollectEventResponse>, Status> {
        let mut stream = request.into_inner();
        // Give each stream its own per-context subdirectory.
        let exporter_kind = self.exporter.clone().in_context_dir(Uuid::now_v7());
        let export_join_handle = tokio::spawn(async move {
            // One exporter per stream, created lazily on the first batch so an
            // empty stream produces no output.
            let mut exporter: Option<Arc<dyn Exporter<T>>> = None;
            while let Some(item) = stream.next().await {
                match item {
                    Ok(request) => {
                        let exporter = match &exporter {
                            Some(exporter) => exporter,
                            None => match create_exporter::<T>(exporter_kind.clone()).await {
                                Ok(created) => exporter.insert(created),
                                Err(e) => {
                                    error!("unable to construct exporter: {e}");
                                    break;
                                }
                            },
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
                        if let Some(exporter) = &exporter
                            && let Err(e) = exporter.force_flush().await
                        {
                            warn!("unable to flush exporter: {e}");
                        }
                        break;
                    }
                }
            }

            // Flush the exporter when stream ends normally
            if let Some(exporter) = &exporter
                && let Err(e) = exporter.force_flush().await
            {
                warn!("unable to flush exporter after stream completion: {e}");
            }
        });
        let _ = export_join_handle.await;
        Ok(Response::new(proto::CollectEventResponse {}))
    }
}
