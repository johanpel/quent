// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! A gRPC-based client that can send [`Event`]s to a collector.

#[doc(hidden)]
pub mod protocol;

use std::time::Duration;

use bytes::Bytes;
use quent_events::Event;
use serde::Serialize;
use tokio::{
    select,
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Status, transport::Channel};

use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use self::protocol::{
    EVENT_BATCH_HEADER_LEN, EVENT_LENGTH_HEADER_LEN, EventBatch, MAX_EVENT_BATCH_ENCODED_LEN,
    MAX_EVENTS_PER_BATCH, collector_client::CollectorClient,
};

#[derive(Debug)]
struct EventBatchBuffer {
    /// Serialized events waiting for the next gRPC message.
    ///
    /// Batching amortizes the per-message transport overhead.
    events: Vec<Bytes>,
    /// Encoded size of the pending events plus their framing.
    ///
    /// Caching the size avoids rescanning the batch whenever an event is added.
    encoded_len: usize,
    /// Largest encoded batch accepted by the receiver.
    ///
    /// The client must flush before adding an event that would exceed this limit.
    max_encoded_len: usize,
    /// Largest event count accepted in one batch.
    ///
    /// The byte limit alone permits about one million zero-length events. The
    /// count limit bounds the sender's and receiver's `Vec<Bytes>` storage and
    /// decode work.
    max_events: usize,
}

impl EventBatchBuffer {
    fn new(max_encoded_len: usize, max_events: usize) -> Self {
        assert!(
            max_events > 0,
            "event batches must allow at least one event"
        );
        Self {
            events: Vec::new(),
            encoded_len: EVENT_BATCH_HEADER_LEN,
            max_encoded_len,
            max_events,
        }
    }

    fn push(&mut self, event: Vec<u8>) -> Result<Option<EventBatch>, usize> {
        let payload_len = event.len();
        let event_encoded_len = EVENT_LENGTH_HEADER_LEN
            .checked_add(payload_len)
            .ok_or(payload_len)?;
        let single_event_batch_len = EVENT_BATCH_HEADER_LEN
            .checked_add(event_encoded_len)
            .ok_or(payload_len)?;
        if single_event_batch_len > self.max_encoded_len {
            return Err(payload_len);
        }

        let full_batch = if self.events.len() >= self.max_events
            || self.encoded_len + event_encoded_len > self.max_encoded_len
        {
            self.take()
        } else {
            None
        };
        self.encoded_len += event_encoded_len;
        self.events.push(event.into());
        Ok(full_batch)
    }

    fn take(&mut self) -> Option<EventBatch> {
        if self.events.is_empty() {
            return None;
        }
        self.encoded_len = EVENT_BATCH_HEADER_LEN;
        Some(EventBatch {
            events: std::mem::take(&mut self.events),
        })
    }
}

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

/// Encode an [`Event`] using the collector wire format.
pub fn serialize_event<T>(event: &Event<T>) -> Result<Vec<u8>, bitcode::Error>
where
    T: serde::Serialize,
{
    bitcode::serialize(event)
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
    event_sender: Sender<Event<T>>,
    cancellation_token: CancellationToken,
    // Taken via `Option::take` in `shutdown(&mut self)` so they can be joined
    // once; a second call finds `None` and is a no-op.
    events_sender_handle: Option<JoinHandle<()>>,
    events_collector_handle: Option<JoinHandle<()>>,
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

        debug!("connected, preparing channels and spawning control thread ...");
        // TODO(johanpel): consider unbounded
        let (event_sender, mut event_receiver): (Sender<Event<T>>, Receiver<Event<T>>) =
            mpsc::channel(1024);
        let (grpc_sender, grpc_receiver): (Sender<EventBatch>, Receiver<EventBatch>) =
            mpsc::channel(1024);

        let cancellation_token = CancellationToken::new();
        let cloned_token = cancellation_token.clone();

        // Spawn a task that takes events, converts them, and sends them as gRPC messages to the collector.
        let events_sender_handle = tokio::spawn(async move {
            let mut buffer =
                EventBatchBuffer::new(MAX_EVENT_BATCH_ENCODED_LEN, MAX_EVENTS_PER_BATCH);
            // Interval by which to export even if the buffer isn't full.
            let mut ticker = tokio::time::interval(Duration::from_millis(128));

            async fn flush_buffer(
                buffer: &mut EventBatchBuffer,
                grpc_sender: &Sender<EventBatch>,
            ) -> Result<(), ()> {
                if let Some(request) = buffer.take() {
                    grpc_sender.send(request).await.map_err(|_| ())?;
                }
                Ok(())
            }

            async fn buffer_event(
                event: Vec<u8>,
                buffer: &mut EventBatchBuffer,
                grpc_sender: &Sender<EventBatch>,
            ) -> Result<bool, ()> {
                match buffer.push(event) {
                    Ok(Some(full_batch)) => {
                        grpc_sender.send(full_batch).await.map_err(|_| ())?;
                        Ok(true)
                    }
                    Ok(None) => Ok(false),
                    Err(payload_len) => {
                        error!(
                            "serialized event is {payload_len} bytes and exceeds the collector message limit; dropping event"
                        );
                        Ok(false)
                    }
                }
            }

            loop {
                select! {
                    Some(event) = event_receiver.recv() => {
                        let serialized_event = match serialize_event(&event) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                error!("unable to serialize event: {e}");
                                continue;
                            }
                        };
                        match buffer_event(serialized_event, &mut buffer, &grpc_sender).await {
                            Ok(true) => {
                                ticker.reset();
                            }
                            Ok(false) => {}
                            Err(()) => {
                                error!("server disconnected");
                                break;
                            }
                        }
                    },
                    _ = ticker.tick() => {
                        if flush_buffer(&mut buffer, &grpc_sender).await.is_err() {
                            error!("server disconnected");
                            break;
                        }
                    },
                    () = cloned_token.cancelled() => {
                        event_receiver.close();
                        // drain events that are buffered
                        while let Some(event) = event_receiver.recv().await {
                            match serialize_event(&event) {
                                Ok(bytes) => {
                                    if buffer_event(bytes, &mut buffer, &grpc_sender).await.is_err() {
                                        error!("server disconnected during shutdown");
                                        return;
                                    }
                                }
                                Err(e) => error!("unable to serialize event: {e}"),
                            }
                        }

                        if flush_buffer(&mut buffer, &grpc_sender).await.is_err() {
                            error!("server disconnected during shutdown");
                        }
                        let pending = grpc_sender.max_capacity() - grpc_sender.capacity();
                        info!(
                            "client shutting down: {pending} gRPC messages pending, flushing..."
                        );
                        // Drop the sender so the gRPC stream receiver sees
                        // the channel is closed and can complete the stream.
                        drop(grpc_sender);
                        break
                    },
                    else => {
                        info!("client shutting down");
                        break
                    }
                }
            }
        });

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
            event_sender,
            cancellation_token,
            events_sender_handle: Some(events_sender_handle),
            events_collector_handle: Some(events_collector_handle),
        })
    }

    /// Send an event to the collector.
    pub async fn send(&self, event: Event<T>) -> CollectorResult<()> {
        // Convert the event into a gRPC message and stream it to the collector.
        self.event_sender
            .send(event)
            .await
            .map_err(|e| CollectorError::SendError(e.to_string()))
    }

    /// Drain and deliver all buffered events, then wait for both background
    /// tasks to finish. Async so it can be awaited from the forwarder rather
    /// than blocking in `Drop` (which would run on a runtime worker thread).
    /// Idempotent; subsequent calls are no-ops.
    pub async fn shutdown(&mut self) {
        self.cancellation_token.cancel();
        if let Some(handle) = self.events_sender_handle.take()
            && let Err(e) = handle.await
        {
            warn!("grpc sender task failed: {e}");
        }
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
        // The forwarder awaits `shutdown` before dropping the exporter, so the
        // tasks are normally already joined. Cancel as a backstop; any handle
        // still present is detached (no blocking — `Drop` may run on a worker).
        self.cancellation_token.cancel();
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::{EVENT_BATCH_HEADER_LEN, EVENT_LENGTH_HEADER_LEN, EventBatchBuffer};

    #[test]
    fn batch_splits_before_exceeding_encoded_limit() {
        let event = vec![42; 3];
        let one_event_len = EVENT_BATCH_HEADER_LEN + EVENT_LENGTH_HEADER_LEN + event.len();
        let mut buffer = EventBatchBuffer::new(one_event_len, usize::MAX);

        assert!(buffer.push(event.clone()).unwrap().is_none());
        let full_batch = buffer.push(event.clone()).unwrap().unwrap();

        assert_eq!(full_batch.events, vec![Bytes::from(event.clone())]);
        assert_eq!(buffer.take().unwrap().events, vec![Bytes::from(event)]);
    }

    #[test]
    fn event_larger_than_encoded_limit_is_rejected() {
        let mut buffer =
            EventBatchBuffer::new(EVENT_BATCH_HEADER_LEN + EVENT_LENGTH_HEADER_LEN, usize::MAX);

        let payload_len = buffer.push(vec![42]).unwrap_err();

        assert_eq!(payload_len, 1);
        assert!(buffer.take().is_none());
    }

    #[test]
    fn batch_splits_at_event_count_limit() {
        let mut buffer = EventBatchBuffer::new(usize::MAX, 1);

        assert!(buffer.push(vec![1]).unwrap().is_none());
        let full_batch = buffer.push(vec![2]).unwrap().unwrap();

        assert_eq!(full_batch.events, vec![Bytes::from_static(&[1])]);
        assert_eq!(
            buffer.take().unwrap().events,
            vec![Bytes::from_static(&[2])]
        );
    }
}
