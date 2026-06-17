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

/// Streams a model's entity events to a collector, wrapping each entity event
/// into the umbrella wire type `U` so the serde variant tag identifies the
/// entity (no separate event-type field is sent).
#[derive(Debug)]
pub struct CollectorExporter<U> {
    client: Client<U>,
}

impl<U> CollectorExporter<U>
where
    U: Serialize + Send + 'static,
{
    pub async fn try_new(
        options: CollectorExporterOptions,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::new(options.source_context_id, options.address).await?;
        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl<T, U> Exporter<T> for CollectorExporter<U>
where
    T: Serialize + Send + EntityEvent + Into<U> + 'static,
    U: Serialize + Send + 'static,
{
    async fn push(&self, event: Event<T>) -> ExporterResult<()> {
        self.client
            .send(Event::new(event.id, event.timestamp, event.data.into()))
            .await
            .map_err(|e| ExporterError::Collector(format!("{e:?}")))?;
        Ok(())
    }
    async fn force_flush(&self) -> ExporterResult<()> {
        // TODO(johanpel): figure this out, it may be that we don't need this trait fn
        Ok(())
    }
}
