// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Unbounded single-producer, single-consumer channel.
//!
//! The consumer receives values in the order they were sent and can drain them
//! in batches. Buffered values have no fixed capacity limit, so sends do not
//! need to wait for the consumer to free space. A successful send makes its
//! value available to the consumer immediately before returning.
//!
//! Values are stored in a chain of fixed-size ring buffers called segments.
//! When the current segment is full, the producer continues in a next segment,
//! reusing an empty spare segment or allocating a new one if no empty segment
//! is available. The consumer drains older segments first, then returns them
//! for reuse or releases them if enough spares are available.
//!
//! As the consumer drains values, it advances a read index that marks the free
//! slots. The producer keeps a local copy of that index, refreshing it only
//! when the ring appears full. A push into an available slot publishes its
//! value before returning, without a lock, compare-and-swap, or allocation.
//!
//! Using `rtrb` avoids implementing slot ownership and publication with unsafe
//! code here. A single bounded ring would reject sends when full.
//!
//! The configured spare limit bounds the number of empty segments retained
//! for reuse, so a temporary burst does not keep memory usage at its peak.
//!
//! The producer checks whether the consumer has closed only when it finds its
//! current segment full, avoiding an extra atomic read on every send. This
//! reduces per-send overhead, which matters when the producer sends a burst of
//! many small values.
//!
//! Until the segment runs full, sends can succeed even after the consumer
//! closes or drops. Draining can free slots and delay the check. A send can
//! therefore report success even though the consumer will never receive its
//! value. To receive every value, stop the producer and wait for any send in
//! progress to finish before closing and draining the consumer. Dropping the
//! consumer discards unread values. This channel therefore provides Level 1
//! shutdown guarantees as described in [shutdown guarantee levels].
//!
//! [shutdown guarantee levels]: ../../instrumentation/PERFORMANCE.md#what-happens-when-i-stop-instrumentation

use std::{
    num::NonZeroUsize,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use rtrb::{Consumer as RingConsumer, Producer as RingProducer, PushError, RingBuffer};

/// Segment capacity and the number of empty segments kept for reuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    /// Number of event slots per segment.
    pub segment_capacity: NonZeroUsize,
    /// Maximum number of empty segments retained per channel.
    pub spare_segments: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            segment_capacity: NonZeroUsize::new(256).unwrap(),
            spare_segments: 2,
        }
    }
}

/// Result of one drain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DrainResult {
    /// Number of values appended to the destination.
    pub drained: usize,
    /// Whether a live or unfinished channel may yield further values.
    pub pending: bool,
}

/// Tracks consumer disconnection and spare availability shared by one SPSC channel's endpoints.
#[derive(Debug)]
struct ChannelState {
    /// Set when the consumer closes or drops.
    consumer_closed: AtomicBool,
    /// Empty segments available for reuse, used to limit spare preallocation.
    spares: AtomicUsize,
}

/// Holds the old segment's writer and the next segment's reader and shared
/// state.
///
/// The producer moves both endpoints into this value when it switches segments.
/// The consumer takes the whole value after draining the old segment, pairs the
/// old writer with its existing reader for reuse, and continues reading from
/// the next segment.
struct SegmentTransition<T> {
    /// Writer tied to the old segment's ring buffer, no longer used by the producer.
    ///
    /// Retaining it allows writes into the same allocation when reused, since
    /// a writer cannot be recreated from the reader alone.
    retired_writer: RingProducer<T>,
    /// Reader paired with the producer's new writer, used only after the old
    /// segment is drained.
    next_reader: RingConsumer<T>,
    /// Next segment's successor and closure state, shared with the producer.
    next_node: Arc<SegmentState<T>>,
}

/// Records a segment's successor or the end of the producer's writes.
struct SegmentState<T> {
    /// Endpoints and shared state passed to the consumer when the producer moves to the next segment.
    transition: OnceLock<Mutex<SegmentTransition<T>>>,
    /// Set when the producer drops, after its final writes and release of this segment's writer.
    closed: AtomicBool,
}

impl<T> SegmentState<T> {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            transition: OnceLock::new(),
            closed: AtomicBool::new(false),
        })
    }

    fn take_transition(&mut self) -> Option<SegmentTransition<T>> {
        self.transition.take().map(|transition| {
            transition
                .into_inner()
                .unwrap_or_else(|poison| poison.into_inner())
        })
    }
}

/// Groups a ring buffer's paired endpoints and state for allocation or reuse.
struct Segment<T> {
    /// Write endpoint paired with this segment's reader.
    writer: RingProducer<T>,
    /// Read endpoint for values written through this segment's writer.
    reader: RingConsumer<T>,
    /// Tracks this segment's successor or the end of the producer's writes.
    node: Arc<SegmentState<T>>,
}

impl<T> Segment<T> {
    fn new(capacity: NonZeroUsize) -> Self {
        let (writer, reader) = RingBuffer::new(capacity.get());
        Self {
            writer,
            reader,
            node: SegmentState::new(),
        }
    }
}

/// The only writer of a segmented channel.
pub(crate) struct Producer<T> {
    current: Option<RingProducer<T>>,
    spare_reader: Option<RingConsumer<Segment<T>>>,
    state: Arc<ChannelState>,
    node: Arc<SegmentState<T>>,
    capacity: NonZeroUsize,
}

/// The only reader of a segmented channel.
pub(crate) struct Consumer<T> {
    current: Option<RingConsumer<T>>,
    spare_writer: Option<RingProducer<Segment<T>>>,
    state: Arc<ChannelState>,
    node: Option<Arc<SegmentState<T>>>,
    config: Config,
}

/// Create an unbounded single-producer, single-consumer channel.
pub(crate) fn spsc<T: Send + 'static>(config: Config) -> (Producer<T>, Consumer<T>) {
    let Segment {
        writer,
        reader,
        node,
    } = Segment::new(config.segment_capacity);
    let (spare_writer, spare_reader) = RingBuffer::new(config.spare_segments.max(1));
    let state = Arc::new(ChannelState {
        consumer_closed: AtomicBool::new(false),
        spares: AtomicUsize::new(0),
    });
    (
        Producer {
            current: Some(writer),
            spare_reader: Some(spare_reader),
            state: Arc::clone(&state),
            node: Arc::clone(&node),
            capacity: config.segment_capacity,
        },
        Consumer {
            current: Some(reader),
            spare_writer: Some(spare_writer),
            state,
            node: Some(node),
            config,
        },
    )
}

impl<T: Send + 'static> Producer<T> {
    /// Publish `value` before returning, growing the channel if needed.
    ///
    /// A push into the current segment neither allocates nor checks whether the
    /// consumer is gone. Growth may allocate and inherit allocator latency.
    /// On a full segment, disconnection returns `value`.
    pub fn push(&mut self, value: T) -> Result<(), T> {
        let value = match self.current.as_mut().unwrap().push(value) {
            Ok(()) => return Ok(()),
            Err(PushError::Full(value)) => value,
        };
        if self.state.consumer_closed.load(Ordering::Acquire) {
            return Err(value);
        }

        let spare = match self.spare_reader.as_mut().unwrap().pop() {
            Ok(spare) => {
                self.state.spares.fetch_sub(1, Ordering::Relaxed);
                spare
            }
            Err(_) => Segment::new(self.capacity),
        };
        let Segment {
            writer,
            reader,
            node,
        } = spare;
        let old_writer = self.current.replace(writer).unwrap();
        let old_node = std::mem::replace(&mut self.node, Arc::clone(&node));
        if old_node
            .transition
            .set(Mutex::new(SegmentTransition {
                retired_writer: old_writer,
                next_reader: reader,
                next_node: node,
            }))
            .is_err()
        {
            unreachable!("a segment has exactly one successor");
        }
        drop(old_node);

        match self.current.as_mut().unwrap().push(value) {
            Ok(()) => Ok(()),
            Err(PushError::Full(_)) => unreachable!("a new segment has free slots"),
        }
    }
}

impl<T> Drop for Producer<T> {
    fn drop(&mut self) {
        // Closure follows the last ring publication and release of its writer.
        drop(self.current.take());
        drop(self.spare_reader.take());
        self.node.closed.store(true, Ordering::Release);
    }
}

impl<T: Send + 'static> Consumer<T> {
    /// Append at most `limit` published values to `output`.
    ///
    /// `pending` also remains true for an open but currently empty channel.
    pub fn drain_into(&mut self, output: &mut Vec<T>, limit: usize) -> DrainResult {
        let mut drained = 0;
        while drained < limit && self.current.is_some() {
            let available = self.current.as_ref().unwrap().slots();
            if available != 0 {
                let count = available.min(limit - drained);
                let chunk = self.current.as_mut().unwrap().read_chunk(count).unwrap();
                output.extend(chunk);
                drained += count;
                continue;
            }
            if !self.advance() {
                break;
            }
        }
        DrainResult {
            drained,
            pending: self.current.is_some(),
        }
    }

    /// Signal disconnection; already published values remain drainable.
    pub fn close(&mut self) {
        self.state.consumer_closed.store(true, Ordering::Release);
    }

    /// Prepare empty segments on the consumer for future producer bursts.
    ///
    /// Returns the number of new segments supplied.
    pub fn preallocate_spares(&mut self) -> usize {
        let mut supplied = 0;
        while self.state.spares.load(Ordering::Relaxed) < self.config.spare_segments {
            if self.spare_writer.as_ref().unwrap().is_abandoned() {
                break;
            }
            let spare = Segment::new(self.config.segment_capacity);
            if self.spare_writer.as_mut().unwrap().push(spare).is_err() {
                break;
            }
            self.state.spares.fetch_add(1, Ordering::Relaxed);
            supplied += 1;
        }
        supplied
    }

    fn advance(&mut self) -> bool {
        let node = self.node.as_mut().unwrap();
        let Some(inner) = Arc::get_mut(node) else {
            return false;
        };
        // The first empty check can race with the producer's final writes.
        // Once the producer has released this node, check again before recycling.
        if self.current.as_ref().unwrap().slots() != 0 {
            return false;
        }
        if let Some(transition) = inner.take_transition() {
            let old_reader = self.current.take().unwrap();
            let old_node = self.node.replace(transition.next_node).unwrap();
            let spare = Segment {
                writer: transition.retired_writer,
                reader: old_reader,
                node: old_node,
            };
            self.current = Some(transition.next_reader);
            if self.config.spare_segments != 0
                && !self.spare_writer.as_ref().unwrap().is_abandoned()
            {
                match self.spare_writer.as_mut().unwrap().push(spare) {
                    Ok(()) => {
                        self.state.spares.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(PushError::Full(spare)) => {
                        drop(spare);
                    }
                }
            } else {
                drop(spare);
            }
            return true;
        }
        if inner.closed.load(Ordering::Acquire) {
            drop(self.current.take());
            drop(self.node.take());
        }
        false
    }
}

impl<T> Drop for Consumer<T> {
    fn drop(&mut self) {
        self.state.consumer_closed.store(true, Ordering::Release);
        while let Some(mut node) = self.node.take() {
            let Some(inner) = Arc::get_mut(&mut node) else {
                drop(node);
                break;
            };
            let transition = inner.take_transition();
            drop(self.current.take());
            drop(node);
            let Some(transition) = transition else {
                break;
            };
            drop(transition.retired_writer);
            self.current = Some(transition.next_reader);
            self.node = Some(transition.next_node);
        }
    }
}
