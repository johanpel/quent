// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! A gRPC-based client that can send [`Event`]s to a collector.

use std::time::Duration;

use quent_events::Event;
use serde::Serialize;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Status, transport::Channel};

use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use quent_collector_proto::{CollectEventRequest, collector_client::CollectorClient};

/// A sink for serialized per-entity event streams.
pub trait CollectorSink {
    /// Ingest a serialized `event` belonging to the entity event stream named
    /// `entity`.
    fn ingest(&self, entity: &str, event: &[u8]) -> Result<(), Box<dyn std::error::Error>>;
}

/// Decode `bytes` into an [`Event`], inverting the wire encoding this client
/// produces on send.
pub fn deserialize_event<T>(bytes: &[u8]) -> Result<Event<T>, bitcode::Error>
where
    T: for<'de> serde::Deserialize<'de>,
{
    bitcode::deserialize(bytes)
}

#[derive(Debug, Error)]
pub enum CollectorError {
    #[error("Unable to connect: {0}")]
    Connect(String),
    #[error("Send error: {0}")]
    SendError(String),
    #[error("Transport error: {0}")]
    Tonic(#[from] tonic::transport::Error),
    #[error("RPC error: {0}")]
    GRPC(#[from] Status),
    #[error("invalid `{key}` metadata value: {source}")]
    Metadata {
        key: &'static str,
        #[source]
        source: tonic::metadata::errors::InvalidMetadataValue,
    },
}

pub type CollectorResult<T> = std::result::Result<T, CollectorError>;

// Trivial implementation of a gRPC client that sends events to a centralized collector
#[derive(Debug)]
pub struct Client<T> {
    _grpc_client: CollectorClient<Channel>,
    grpc_sender: Option<Sender<CollectEventRequest>>,
    events_collector_handle: Option<JoinHandle<()>>,
    _event: std::marker::PhantomData<fn(T)>,
}

impl<T> Client<T>
where
    T: Serialize + Send + 'static,
{
    pub async fn new(
        source_context_id: Uuid,
        entity_type: &str,
        address: http::Uri,
    ) -> CollectorResult<Client<T>> {
        debug!("connecting to {address}");
        // Try to connect.
        // TODO(johanpel): figure out whether this can also go through health check
        const MAX_RETRIES: usize = 42;
        let mut client = Err(CollectorError::Connect(format!(
            "failed to connect after {MAX_RETRIES} attempts..."
        )));
        for retry in 1..MAX_RETRIES + 1 {
            match CollectorClient::connect(address.clone()).await {
                Ok(c) => {
                    client = Ok(c);
                    break;
                }
                Err(e) => {
                    warn!("unable to connect: {e}, retrying in 1s... {retry}/{MAX_RETRIES}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            };
        }
        let client = client?;

        debug!("connected, preparing channel and spawning control thread ...");
        let (grpc_sender, grpc_receiver): (
            Sender<CollectEventRequest>,
            Receiver<CollectEventRequest>,
        ) = mpsc::channel(1024);

        debug!("opening stream ...");

        // Identify this stream so the collector reproduces its events under the
        // id, and tag it with the entity type so the collector routes the batch
        // to the matching entity observer.
        let mut req = Request::new(ReceiverStream::new(grpc_receiver));
        req.metadata_mut().insert(
            "source-context-id",
            source_context_id
                .to_string()
                .parse()
                .map_err(|source| CollectorError::Metadata {
                    key: "source-context-id",
                    source,
                })?,
        );
        req.metadata_mut().insert(
            "entity-type",
            entity_type
                .parse()
                .map_err(|source| CollectorError::Metadata {
                    key: "entity-type",
                    source,
                })?,
        );

        let mut cloned_client = client.clone();
        let events_collector_handle = tokio::spawn(async move {
            let _ = cloned_client.collect_events(req).await;
        });
        debug!("client ready to send events");

        Ok(Client {
            _grpc_client: client,
            grpc_sender: Some(grpc_sender),
            events_collector_handle: Some(events_collector_handle),
            _event: std::marker::PhantomData,
        })
    }

    /// Send an event to the collector.
    pub async fn send(&self, event: Event<T>) -> CollectorResult<()> {
        let event =
            bitcode::serialize(&event).map_err(|e| CollectorError::SendError(e.to_string()))?;
        self.send_request(CollectEventRequest { event: vec![event] })
            .await
    }

    /// Send an ordered event batch to the collector.
    pub async fn send_batch(&self, events: Vec<Event<T>>) -> CollectorResult<()> {
        let event = events
            .iter()
            .map(bitcode::serialize)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| CollectorError::SendError(e.to_string()))?;
        self.send_request(CollectEventRequest { event }).await
    }

    async fn send_request(&self, request: CollectEventRequest) -> CollectorResult<()> {
        self.grpc_sender
            .as_ref()
            .ok_or_else(|| CollectorError::SendError("client has shut down".to_owned()))?
            .send(request)
            .await
            .map_err(|e| CollectorError::SendError(e.to_string()))
    }

    /// Drain and deliver all buffered events, then wait for both background
    /// tasks to finish. Async so it can be awaited from the forwarder rather
    /// than blocking in `Drop` (which would run on a runtime worker thread).
    /// Idempotent; subsequent calls are no-ops.
    pub async fn shutdown(&mut self) {
        let pending = self
            .grpc_sender
            .as_ref()
            .map_or(0, |sender| sender.max_capacity() - sender.capacity());
        info!("client shutting down: {pending} gRPC messages pending, flushing...");
        drop(self.grpc_sender.take());
        if let Some(handle) = self.events_collector_handle.take()
            && let Err(e) = handle.await
        {
            warn!("grpc collector task failed: {e}");
        }
        info!("client shut down, all gRPC messages flushed");
    }
}

impl<T> Drop for Client<T> {
    fn drop(&mut self) {
        drop(self.grpc_sender.take());
    }
}
