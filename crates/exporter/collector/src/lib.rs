// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Exporter sending events to a Collector service

use quent_collector_client::Client;
use quent_events::{EntityEvent, Event};
use quent_exporter_types::{Exporter, ExporterError, ExporterResult};
use serde::Serialize;
use uuid::Uuid;

/// Options for the collector exporter.
///
/// Streams events over gRPC to a remote collector service. Use this for
/// distributed deployments where events are centralized for analysis.
/// `source_context_id` identifies this stream to the collector, which
/// reproduces the source's output under that id.
#[derive(Debug, Default, Clone)]
pub struct CollectorExporterOptions {
    pub address: String,
    pub source_context_id: Uuid,
}

/// Streams one entity's events to a collector. The stream is tagged with the
/// entity name (`T::NAME`) so the collector routes each batch to the matching
/// entity observer.
#[derive(Debug)]
pub struct CollectorExporter<T> {
    client: Client<T>,
}

impl<T> CollectorExporter<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    pub async fn try_new(
        options: CollectorExporterOptions,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::new(options.source_context_id, T::NAME, options.address).await?;
        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl<T> Exporter<T> for CollectorExporter<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    async fn push(&self, event: Event<T>) -> ExporterResult<()> {
        self.client
            .send(event)
            .await
            .map_err(|e| ExporterError::Collector(format!("{e:?}")))?;
        Ok(())
    }
    async fn force_flush(&self) -> ExporterResult<()> {
        // Drain buffered events and wait for delivery. The forwarder awaits this
        // on shutdown, so the client's tasks are joined here rather than in
        // `Client::drop` (which may run on a runtime worker, where blocking
        // panics).
        self.client.shutdown().await;
        Ok(())
    }
}
