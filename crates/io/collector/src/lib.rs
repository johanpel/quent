// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Exporter sending events to a Collector service

use quent_collector_client::Client;
pub use quent_collector_client::ClientOptions;
use quent_events::{EntityEvent, Event};
use quent_io_types::{Exporter, ExporterError, ExporterProvider, ExporterResult};
use serde::Serialize;
use uuid::Uuid;

/// User-facing options for the collector exporter.
///
/// Streams events over gRPC to a remote collector service.
#[derive(Debug, Default, Clone)]
pub struct Options {
    address: http::Uri,
    client: ClientOptions,
}

/// Error returned when a collector address is not a valid URI.
#[derive(Debug, thiserror::Error)]
#[error("invalid collector address: {source}")]
pub struct CollectorAddressError {
    #[source]
    source: http::uri::InvalidUri,
}

impl Options {
    pub fn new(address: http::Uri) -> Self {
        Self {
            address,
            client: ClientOptions::default(),
        }
    }

    /// Parses a collector address into exporter options.
    pub fn try_new(address: &str) -> Result<Self, CollectorAddressError> {
        let address = address
            .parse()
            .map_err(|source| CollectorAddressError { source })?;
        Ok(Self::new(address))
    }

    /// Sets the collector client's buffering and batching options.
    pub fn with_client_options(mut self, options: ClientOptions) -> Self {
        self.client = options;
        self
    }
}

#[async_trait::async_trait]
impl<T> ExporterProvider<T> for Options
where
    T: Send + EntityEvent + 'static,
    T: serde::Serialize,
{
    async fn create_exporter(&self, context_id: Uuid) -> ExporterResult<Box<dyn Exporter<T>>> {
        Ok(Box::new(
            CollectorExporter::<T>::try_new_with_options(
                self.address.clone(),
                context_id,
                self.client,
            )
            .await
            .map_err(ExporterError::Other)?,
        ) as Box<dyn Exporter<T>>)
    }
}

/// Streams one entity's events to a collector. The stream is tagged with the
/// entity name (`T::NAME`) so the collector routes each batch to the matching
/// entity observer.
#[derive(Debug)]
pub struct CollectorExporter<T> {
    /// `None` once [`shutdown`](Exporter::shutdown) has drained and released it.
    client: Option<Client<T>>,
}

impl<T> CollectorExporter<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    /// `source_context_id` identifies this stream to the collector, which
    /// reproduces the source's output under that id.
    pub async fn try_new(
        address: http::Uri,
        source_context_id: Uuid,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::try_new_with_options(address, source_context_id, ClientOptions::default()).await
    }

    /// Connects an exporter with explicit collector client options.
    pub async fn try_new_with_options(
        address: http::Uri,
        source_context_id: Uuid,
        options: ClientOptions,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::new_with_options(source_context_id, T::NAME, address, options).await?;
        Ok(Self {
            client: Some(client),
        })
    }
}

#[async_trait::async_trait]
impl<T> Exporter<T> for CollectorExporter<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    async fn push(&mut self, event: Event<T>) -> ExporterResult<()> {
        let client = self.client.as_ref().ok_or(ExporterError::Shutdown)?;
        client.send(event).await.map_err(ExporterError::other)?;
        Ok(())
    }
    async fn shutdown(mut self: Box<Self>) -> ExporterResult<()> {
        let Some(mut client) = self.client.take() else {
            return Ok(());
        };
        // Drain buffered events and wait for delivery. The forwarder awaits this
        // on shutdown, so the client's tasks are joined here rather than in
        // `Client::drop` (which may run on a runtime worker, where blocking
        // panics).
        client.shutdown().await;
        Ok(())
    }
}
