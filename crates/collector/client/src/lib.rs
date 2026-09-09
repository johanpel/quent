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
    EVENT_BATCH_HEADER_LEN, EVENT_LENGTH_HEADER_LEN, EventBatch, MAX_EVENTS_PER_BATCH,
    collector_client::CollectorClient,
};

const DEFAULT_EVENT_CHANNEL_CAPACITY: usize = 1024;
const DEFAULT_BATCH_CHANNEL_CAPACITY: usize = 1024;
const DEFAULT_BATCH_TARGET_ENCODED_LEN: usize = 4 * 1024 * 1024;
const DEFAULT_MAX_EVENTS_PER_BATCH: usize = MAX_EVENTS_PER_BATCH;
const DEFAULT_BATCH_FLUSH_INTERVAL: Duration = Duration::from_millis(128);

/// Controls collector client buffering and batching.
///
/// Defaults to 1,024 events and batches per channel, a 4 MiB batch target,
/// 65,536 events per batch, and a 128 ms flush interval.
#[derive(Clone, Copy, Debug)]
pub struct ClientOptions {
    /// Maximum events waiting to be serialized.
    ///
    /// The bounded channel applies backpressure to the background exporter when
    /// serialization cannot keep up. The instrumentation API publishes into a
    /// separate unbounded queue, so this capacity does not make event emission
    /// wait for collector progress.
    event_channel_capacity: usize,
    /// Maximum encoded batches waiting to enter the gRPC stream.
    ///
    /// The bounded channel limits memory retained when network delivery is
    /// slower than encoding.
    batch_channel_capacity: usize,
    /// Preferred maximum encoded size of an ordinary batch.
    ///
    /// Events larger than this target are sent intact in a dedicated batch.
    batch_target_encoded_len: usize,
    /// Maximum number of events placed in one batch.
    ///
    /// This bounds per-batch metadata allocation and decode work independently
    /// of payload size.
    max_events_per_batch: usize,
    /// Maximum time a non-empty partial batch waits before being sent.
    ///
    /// Periodic flushing bounds delivery latency when traffic does not fill
    /// batches.
    batch_flush_interval: Duration,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            event_channel_capacity: DEFAULT_EVENT_CHANNEL_CAPACITY,
            batch_channel_capacity: DEFAULT_BATCH_CHANNEL_CAPACITY,
            batch_target_encoded_len: DEFAULT_BATCH_TARGET_ENCODED_LEN,
            max_events_per_batch: DEFAULT_MAX_EVENTS_PER_BATCH,
            batch_flush_interval: DEFAULT_BATCH_FLUSH_INTERVAL,
        }
    }
}

impl ClientOptions {
    /// Sets the maximum number of events waiting to be serialized.
    ///
    /// A zero capacity is rejected by [`Client::new_with_options`].
    pub fn with_event_channel_capacity(mut self, capacity: usize) -> Self {
        self.event_channel_capacity = capacity;
        self
    }

    /// Sets the maximum number of encoded batches waiting to enter the gRPC stream.
    ///
    /// A zero capacity is rejected by [`Client::new_with_options`].
    pub fn with_batch_channel_capacity(mut self, capacity: usize) -> Self {
        self.batch_channel_capacity = capacity;
        self
    }

    /// Sets the preferred encoded size of an ordinary batch.
    ///
    /// The target must fit the `u32` gRPC length prefix and have room for one
    /// empty framed event.
    pub fn with_batch_target_encoded_len(mut self, target_encoded_len: usize) -> Self {
        self.batch_target_encoded_len = target_encoded_len;
        self
    }

    /// Sets the maximum number of events placed in one batch.
    ///
    /// The value must be between 1 and 65,536 inclusive.
    pub fn with_max_events_per_batch(mut self, max_events: usize) -> Self {
        self.max_events_per_batch = max_events;
        self
    }

    /// Sets the maximum time a non-empty partial batch waits before being sent.
    ///
    /// A zero duration is rejected by [`Client::new_with_options`].
    pub fn with_batch_flush_interval(mut self, interval: Duration) -> Self {
        self.batch_flush_interval = interval;
        self
    }

    fn validate(&self) -> Result<(), CollectorError> {
        if self.event_channel_capacity == 0 {
            return Err(CollectorError::InvalidEventChannelCapacity);
        }
        if self.batch_channel_capacity == 0 {
            return Err(CollectorError::InvalidBatchChannelCapacity);
        }
        let min_batch_encoded_len = EVENT_BATCH_HEADER_LEN + EVENT_LENGTH_HEADER_LEN;
        if !(min_batch_encoded_len..=u32::MAX as usize).contains(&self.batch_target_encoded_len) {
            return Err(CollectorError::InvalidBatchEncodedLen {
                actual: self.batch_target_encoded_len,
                minimum: min_batch_encoded_len,
                maximum: u32::MAX as usize,
            });
        }
        if !(1..=MAX_EVENTS_PER_BATCH).contains(&self.max_events_per_batch) {
            return Err(CollectorError::InvalidBatchEventCount {
                actual: self.max_events_per_batch,
                maximum: MAX_EVENTS_PER_BATCH,
            });
        }
        if self.batch_flush_interval.is_zero() {
            return Err(CollectorError::InvalidBatchFlushInterval);
        }
        Ok(())
    }
}

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
    /// Encoded size at which a batch should be flushed.
    ///
    /// This bounds ordinary message sizes without rejecting a larger individual event.
    target_encoded_len: usize,
    /// Largest event count accepted in one batch.
    ///
    /// The byte target alone permits about one million zero-length events. The
    /// count limit bounds the sender's and receiver's `Vec<Bytes>` storage and
    /// decode work.
    max_events: usize,
}

impl EventBatchBuffer {
    fn try_new(target_encoded_len: usize, max_events: usize) -> Result<Self, CollectorError> {
        let min_encoded_len = EVENT_BATCH_HEADER_LEN + EVENT_LENGTH_HEADER_LEN;
        if target_encoded_len < min_encoded_len {
            return Err(CollectorError::InvalidBatchEncodedLen {
                actual: target_encoded_len,
                minimum: min_encoded_len,
                maximum: u32::MAX as usize,
            });
        }
        if max_events == 0 {
            return Err(CollectorError::InvalidBatchEventCount {
                actual: max_events,
                maximum: MAX_EVENTS_PER_BATCH,
            });
        }
        Ok(Self {
            events: Vec::new(),
            encoded_len: EVENT_BATCH_HEADER_LEN,
            target_encoded_len,
            max_events,
        })
    }

    fn push(&mut self, event: Vec<u8>) -> (Option<EventBatch>, bool) {
        let event_encoded_len = EVENT_LENGTH_HEADER_LEN.saturating_add(event.len());
        let single_event_batch_len = EVENT_BATCH_HEADER_LEN.saturating_add(event_encoded_len);
        let exceeds_target = single_event_batch_len > self.target_encoded_len;

        let full_batch = if self.events.len() >= self.max_events
            || self.encoded_len.saturating_add(event_encoded_len) > self.target_encoded_len
        {
            self.take()
        } else {
            None
        };
        self.encoded_len = self.encoded_len.saturating_add(event_encoded_len);
        self.events.push(event.into());
        (full_batch, exceeds_target)
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

#[derive(Debug)]
struct BatchChannelClosed {
    /// Events in the batch that the closed channel did not accept.
    ///
    /// The count is retained to quantify the minimum delivery loss in logs.
    unsent_events: usize,
}

impl BatchChannelClosed {
    fn log(self) {
        error!(
            unsent_events = self.unsent_events,
            "collector stream ended before a batch could be queued; the batch and any remaining buffered events will not be delivered"
        );
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
    #[error(
        "encoded batch size target must be between {minimum} and {maximum} bytes, got {actual}"
    )]
    InvalidBatchEncodedLen {
        actual: usize,
        minimum: usize,
        maximum: usize,
    },
    #[error("maximum events per batch must be between 1 and {maximum}, got {actual}")]
    InvalidBatchEventCount { actual: usize, maximum: usize },
    #[error("event channel capacity must be greater than zero")]
    InvalidEventChannelCapacity,
    #[error("batch channel capacity must be greater than zero")]
    InvalidBatchChannelCapacity,
    #[error("batch flush interval must be greater than zero")]
    InvalidBatchFlushInterval,
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
        Self::new_with_options(
            source_context_id,
            entity_type,
            address,
            ClientOptions::default(),
        )
        .await
    }

    /// Connects to a collector using the supplied buffering and batching options.
    ///
    /// # Errors
    ///
    /// Returns an error before connecting if a channel capacity or flush interval
    /// is zero, or a batch setting is outside the supported wire-format range.
    pub async fn new_with_options(
        source_context_id: Uuid,
        entity_type: &str,
        address: http::Uri,
        options: ClientOptions,
    ) -> CollectorResult<Client<T>> {
        options.validate()?;
        let mut buffer = EventBatchBuffer::try_new(
            options.batch_target_encoded_len,
            options.max_events_per_batch,
        )?;

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
        let (event_sender, mut event_receiver): (Sender<Event<T>>, Receiver<Event<T>>) =
            mpsc::channel(options.event_channel_capacity);
        let (grpc_sender, grpc_receiver): (Sender<EventBatch>, Receiver<EventBatch>) =
            mpsc::channel(options.batch_channel_capacity);

        let cancellation_token = CancellationToken::new();
        let cloned_token = cancellation_token.clone();

        // Spawn a task that takes events, converts them, and sends them as gRPC messages to the collector.
        let events_sender_handle = tokio::spawn(async move {
            // Interval by which to export even if the buffer isn't full.
            let mut ticker = tokio::time::interval(options.batch_flush_interval);

            async fn queue_batch(
                batch: EventBatch,
                grpc_sender: &Sender<EventBatch>,
            ) -> Result<(), BatchChannelClosed> {
                grpc_sender
                    .send(batch)
                    .await
                    .map_err(|error| BatchChannelClosed {
                        unsent_events: error.0.events.len(),
                    })
            }

            async fn flush_buffer(
                buffer: &mut EventBatchBuffer,
                grpc_sender: &Sender<EventBatch>,
            ) -> Result<(), BatchChannelClosed> {
                if let Some(request) = buffer.take() {
                    queue_batch(request, grpc_sender).await?;
                }
                Ok(())
            }

            async fn buffer_event(
                event: Vec<u8>,
                buffer: &mut EventBatchBuffer,
                grpc_sender: &Sender<EventBatch>,
            ) -> Result<bool, BatchChannelClosed> {
                let payload_len = event.len();
                let (full_batch, exceeds_target) = buffer.push(event);
                if exceeds_target {
                    let target_encoded_len = buffer.target_encoded_len;
                    warn!(
                        "serialized event has {payload_len} payload bytes and cannot fit within the {target_encoded_len}-byte collector batch target after framing; sending it intact in a dedicated message"
                    );
                }
                if let Some(full_batch) = full_batch {
                    queue_batch(full_batch, grpc_sender).await?;
                    return Ok(true);
                }
                Ok(false)
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
                            Err(error) => {
                                error.log();
                                break;
                            }
                        }
                    },
                    _ = ticker.tick() => {
                        if let Err(error) = flush_buffer(&mut buffer, &grpc_sender).await {
                            error.log();
                            break;
                        }
                    },
                    () = cloned_token.cancelled() => {
                        event_receiver.close();
                        // drain events that are buffered
                        while let Some(event) = event_receiver.recv().await {
                            match serialize_event(&event) {
                                Ok(bytes) => {
                                    if let Err(error) = buffer_event(bytes, &mut buffer, &grpc_sender).await {
                                        error.log();
                                        return;
                                    }
                                }
                                Err(e) => error!("unable to serialize event: {e}"),
                            }
                        }

                        if let Err(error) = flush_buffer(&mut buffer, &grpc_sender).await {
                            error.log();
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
    use std::time::Duration;

    use bytes::Bytes;

    use super::{
        ClientOptions, CollectorError, EVENT_BATCH_HEADER_LEN, EVENT_LENGTH_HEADER_LEN,
        EventBatchBuffer, MAX_EVENTS_PER_BATCH,
    };

    #[test]
    fn batch_splits_before_exceeding_encoded_limit() {
        let event = vec![42; 3];
        let one_event_len = EVENT_BATCH_HEADER_LEN + EVENT_LENGTH_HEADER_LEN + event.len();
        let mut buffer = EventBatchBuffer::try_new(one_event_len, usize::MAX).unwrap();

        assert!(buffer.push(event.clone()).0.is_none());
        let full_batch = buffer.push(event.clone()).0.unwrap();

        assert_eq!(full_batch.events, vec![Bytes::from(event.clone())]);
        assert_eq!(buffer.take().unwrap().events, vec![Bytes::from(event)]);
    }

    #[test]
    fn event_larger_than_encoded_target_is_retained() {
        let mut buffer =
            EventBatchBuffer::try_new(EVENT_BATCH_HEADER_LEN + EVENT_LENGTH_HEADER_LEN, usize::MAX)
                .unwrap();

        let (full_batch, exceeds_target) = buffer.push(vec![42]);

        assert!(full_batch.is_none());
        assert!(exceeds_target);
        assert_eq!(
            buffer.take().unwrap().events,
            vec![Bytes::from_static(&[42])]
        );
    }

    #[test]
    fn batch_splits_at_event_count_limit() {
        let mut buffer = EventBatchBuffer::try_new(usize::MAX, 1).unwrap();

        assert!(buffer.push(vec![1]).0.is_none());
        let full_batch = buffer.push(vec![2]).0.unwrap();

        assert_eq!(full_batch.events, vec![Bytes::from_static(&[1])]);
        assert_eq!(
            buffer.take().unwrap().events,
            vec![Bytes::from_static(&[2])]
        );
    }

    #[test]
    fn invalid_batch_limits_are_rejected() {
        let min_encoded_len = EVENT_BATCH_HEADER_LEN + EVENT_LENGTH_HEADER_LEN;

        assert!(matches!(
            EventBatchBuffer::try_new(min_encoded_len - 1, 1),
            Err(CollectorError::InvalidBatchEncodedLen {
                actual,
                minimum,
                ..
            }) if actual == min_encoded_len - 1 && minimum == min_encoded_len
        ));
        assert!(matches!(
            EventBatchBuffer::try_new(min_encoded_len, 0),
            Err(CollectorError::InvalidBatchEventCount { actual: 0, .. })
        ));
    }

    #[test]
    fn invalid_client_options_are_rejected() {
        assert!(matches!(
            ClientOptions::default()
                .with_event_channel_capacity(0)
                .validate(),
            Err(CollectorError::InvalidEventChannelCapacity)
        ));
        assert!(matches!(
            ClientOptions::default()
                .with_batch_channel_capacity(0)
                .validate(),
            Err(CollectorError::InvalidBatchChannelCapacity)
        ));
        #[cfg(target_pointer_width = "64")]
        assert!(matches!(
            ClientOptions::default()
                .with_batch_target_encoded_len(usize::MAX)
                .validate(),
            Err(CollectorError::InvalidBatchEncodedLen { .. })
        ));
        assert!(matches!(
            ClientOptions::default()
                .with_max_events_per_batch(0)
                .validate(),
            Err(CollectorError::InvalidBatchEventCount { .. })
        ));
        assert!(matches!(
            ClientOptions::default()
                .with_max_events_per_batch(MAX_EVENTS_PER_BATCH + 1)
                .validate(),
            Err(CollectorError::InvalidBatchEventCount { .. })
        ));
        assert!(matches!(
            ClientOptions::default()
                .with_batch_flush_interval(Duration::ZERO)
                .validate(),
            Err(CollectorError::InvalidBatchFlushInterval)
        ));
    }
}
