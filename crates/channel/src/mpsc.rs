// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! An unbounded multi-producer, single-consumer channel optimized for very low
//! producer latency, irrespective of the number of threads sending.
//!
//! [`unbounded_channel`] gives each sending thread its own queue, avoiding
//! contention between producers on a shared queue. A single receiver drains
//! these queues in batches. Sends can still succeed after the receiver closes,
//! so stop all sends before closing and draining to receive every value.
//!
//! ## Per-thread queues
//!
//! The shareable sender registers one SPSC queue per sending thread on its
//! first send. Values stay in order within a queue. There is no ordering
//! guarantee across threads or between a queue and the fallback.
//!
//! ## Publication and growth
//!
//! Each per-thread queue uses the publication and growth behavior described in
//! [`crate::spsc`]. General-purpose unbounded MPSC channels coordinate
//! concurrent producers that a per-thread queue does not have.
//!
//! ## Collection and reuse
//!
//! The receiver visits registered queues and drains values in batches. It
//! delegates segment reuse to each SPSC channel.
//!
//! ## Teardown and shutdown
//!
//! Sends during thread-local destruction use a separate fallback queue if the
//! thread's queue has already been destroyed. The channel does not wake an
//! async task. After stopping sends, drain before dropping the receiver; values
//! still buffered when it is dropped are destroyed.
//!
//! ## Acknowledgments
//!
//! [Quill](https://github.com/odygrd/quill/blob/master/include/quill/core/ThreadContextManager.h),
//! [ticklog](https://github.com/tensorbinge/ticklog),
//! [fmtlog](https://github.com/MengRao/fmtlog/blob/main/fmtlog.h), and
//! [fastrace](https://github.com/fast/fastrace/blob/main/crates/fastrace/src/util/command_bus.rs)
//! inspired this design through their use of per-thread producer queues. These
//! projects provide valuable examples of low-overhead event collection, though
//! their capacity and overflow policies differ.
//!
//! # Quent-specific implications
//!
//! Quent events carry timestamps. Moving one entity handle between threads
//! normally takes longer than a send or obtaining a timestamp, so its events
//! are usually easy to order. If several handles for the same entity emit
//! concurrently, callers must synchronize them when order matters. Whether
//! timestamps alone can distinguish the correct order of events depends on the
//! configured clock. (Re-)consider that case carefully before using this
//! channel.

use std::{
    any::Any,
    cell::RefCell,
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use rustc_hash::FxHashMap;

use crate::spsc::{Config, Consumer, Producer, spsc};

/// Supplies unique channel IDs used to distinguish producers in each thread's
/// local storage.
static NEXT_CHANNEL_ID: AtomicUsize = AtomicUsize::new(1);

/// Holds one channel's producer and closure flag for the current thread,
/// regardless of payload type.
struct LocalEntry {
    /// This thread's SPSC producer, type-erased so channels with different
    /// payload types can share the TLS map.
    producer: Box<dyn Any>,
    /// Set when the receiver closes or drops, allowing stale thread-local
    /// entries to be removed.
    closed: Arc<AtomicBool>,
}

// TODO(johanpel): we could cache the last used channel entry separately to
//                 avoid map lookups on repeated sends. This would need to be
//                 measured on some real workloads.
thread_local! {
    /// Maps channel IDs to this thread's producers, with one entry per channel
    /// used by the thread. Every send looks up its channel ID unless
    /// thread-local storage has already been destroyed.
    static LOCAL: RefCell<FxHashMap<usize, LocalEntry>> = RefCell::new(FxHashMap::default());
}

/// Holds newly registered consumers and teardown fallback events under a common
/// admission cutoff.
struct Registry<T> {
    /// Newly registered SPSC consumers not yet collected by the receiver.
    pending: Vec<Consumer<T>>,
    /// Holds values emitted by thread-local destructors after this channel's
    /// thread-local producer map has been destroyed, while the receiver remains open.
    fallback: VecDeque<T>,
    /// Rejects new registrations and fallback sends once the receiver closes or
    /// drops.
    closed: bool,
}

/// Holds the channel identity, configuration, and registration state shared by
/// all endpoints.
struct Shared<T> {
    /// Unique channel ID used to find its producer in each thread's local storage.
    id: usize,
    /// Settings applied to every per-thread SPSC channel.
    config: Config,
    /// Notifies thread-local entries of receiver closure so they can be removed.
    closed: Arc<AtomicBool>,
    /// Serializes registration and fallback sends with receiver closure.
    registry: Mutex<Registry<T>>,
}

/// A cloneable, synchronous sender with one SPSC producer per calling thread.
pub struct Sender<T> {
    /// Channel identity and registration state retained by every sender clone.
    shared: Arc<Shared<T>>,
}

/// The single collector of a thread registry.
pub struct Receiver<T> {
    /// Registration and closure state shared with the senders.
    shared: Arc<Shared<T>>,
    /// Collected per-thread consumers that may still yield values.
    channels: Vec<Consumer<T>>,
    /// Next per-thread consumer to visit, wrapped to the current channel count
    /// before use.
    cursor: usize,
    /// Alternates which queue group drains first so neither fallback nor
    /// per-thread queues always take priority.
    fallback_first: bool,
}

/// Creates an unbounded channel with default per-thread SPSC configuration.
pub fn unbounded_channel<T: Send + 'static>() -> (Sender<T>, Receiver<T>) {
    unbounded_channel_with_config(Config::default())
}

/// Creates an unbounded channel with the supplied configuration for each per-thread SPSC channel.
pub fn unbounded_channel_with_config<T: Send + 'static>(
    config: Config,
) -> (Sender<T>, Receiver<T>) {
    let id = NEXT_CHANNEL_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .expect("channel identity space exhausted");
    let shared = Arc::new(Shared {
        id,
        config,
        closed: Arc::new(AtomicBool::new(false)),
        registry: Mutex::new(Registry {
            pending: Vec::new(),
            fallback: VecDeque::new(),
            closed: false,
        }),
    });
    (
        Sender {
            shared: Arc::clone(&shared),
        },
        Receiver {
            shared,
            channels: Vec::new(),
            cursor: 0,
            fallback_first: false,
        },
    )
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<T: Send + 'static> Sender<T> {
    /// Queues a value for the receiver without waiting for it to be read.
    ///
    /// Success does not guarantee delivery because receiver closure may not be
    /// detected immediately, so sends can succeed after the receiver closes or drops.
    ///
    /// The first send through this channel on each thread and sends during
    /// thread-local destruction may acquire a lock and allocate memory.
    /// Subsequent sends outside thread-local destruction do not acquire a lock,
    /// but may allocate when buffered values require more storage.
    ///
    /// # Errors
    ///
    /// Returns the original value if the receiver is detected to be closed or dropped.
    pub fn send(&self, value: T) -> Result<(), T> {
        let mut value = Some(value);
        let mut removed = Vec::new();
        let result = LOCAL.try_with(|local| {
            let mut entries = local.borrow_mut();
            if let Some(entry) = entries.get_mut(&self.shared.id) {
                // safety: A channel ID is never reused and always identifies
                // the same payload type.
                let producer = entry.producer.downcast_mut::<Producer<T>>().unwrap();
                // safety: This is the first and only take on this send path.
                let result = producer.push(value.take().unwrap());
                if result.is_err() {
                    // safety: The entry was just found under this exclusive map
                    // borrow.
                    removed.push(entries.remove(&self.shared.id).unwrap());
                }
                return result;
            }

            removed.extend(
                entries
                    .extract_if(|_, entry| entry.closed.load(Ordering::Acquire))
                    .take(8)
                    .map(|(_, entry)| entry),
            );
            let (mut producer, consumer) = spsc(self.shared.config);
            let mut registry = self
                .shared
                .registry
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if registry.closed {
                drop(registry);
                drop(producer);
                drop(consumer);
                // safety: Registration failed before the value was taken.
                return Err(value.take().unwrap());
            }
            registry.pending.push(consumer);
            drop(registry);
            // safety: Successful registration has not taken the value yet.
            let result = producer.push(value.take().unwrap());
            entries.insert(
                self.shared.id,
                LocalEntry {
                    producer: Box::new(producer),
                    closed: Arc::clone(&self.shared.closed),
                },
            );
            result
        });
        drop(removed);
        match result {
            Ok(result) => result,
            Err(_) => {
                let mut registry = self
                    .shared
                    .registry
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if registry.closed {
                    // safety: Failed TLS access never ran the closure that takes the value.
                    Err(value.take().unwrap())
                } else {
                    // safety: Failed TLS access never ran the closure that takes the value.
                    registry.fallback.push_back(value.take().unwrap());
                    Ok(())
                }
            }
        }
    }
}

impl<T: Send + 'static> Receiver<T> {
    /// Appends at most `limit` available values to `output` and returns the number appended.
    ///
    /// Preserves existing contents of `output` and does not wait for new values.
    /// A zero return does not mean all senders have
    /// disconnected or that no further values can arrive.
    ///
    /// Values sent by one thread retain their order, except that values sent
    /// during thread-local destruction may arrive before that thread's earlier
    /// values. Values from different threads have no relative ordering guarantee.
    ///
    /// To receive every accepted value, stop further sends and wait for sends in
    /// progress to finish, then call [`Self::close`] and drain until this returns
    /// zero. Senders need not be dropped first.
    ///
    /// This method may acquire locks and allocate memory.
    pub fn drain_into(&mut self, output: &mut Vec<T>, limit: NonZeroUsize) -> usize {
        self.collect_registrations();
        let limit = limit.get();
        let start = output.len();
        if self.fallback_first {
            self.drain_fallback(output, limit.div_ceil(2));
        }
        let visits = self.channels.len();
        let mut remaining = limit - (output.len() - start);
        for _ in 0..visits {
            if remaining == 0 || self.channels.is_empty() {
                break;
            }
            self.cursor %= self.channels.len();
            let budget = remaining.min(self.shared.config.segment_capacity.get());
            // safety: The loop excludes zero remaining budget, and segment
            // capacity is nonzero.
            let report =
                self.channels[self.cursor].drain_into(output, NonZeroUsize::new(budget).unwrap());
            remaining -= report.drained;
            if !report.pending {
                self.channels.swap_remove(self.cursor);
            } else {
                self.cursor += 1;
            }
        }
        if !self.fallback_first {
            self.drain_fallback(output, limit - (output.len() - start));
        }
        self.fallback_first = !self.fallback_first;
        output.len() - start
    }

    /// Stop accepting registration and teardown fallback events.
    ///
    /// Already registered channels remain drainable.
    ///
    /// Senders notice closure only when they need to switch segments.
    pub fn close(&mut self) {
        let mut registry = self
            .shared
            .registry
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        registry.closed = true;
        self.shared.closed.store(true, Ordering::Release);
        for channel in &mut registry.pending {
            channel.close();
        }
        drop(registry);
        for channel in &mut self.channels {
            channel.close();
        }
    }

    fn collect_registrations(&mut self) {
        let mut registry = self
            .shared
            .registry
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.channels.append(&mut registry.pending);
    }

    fn drain_fallback(&mut self, output: &mut Vec<T>, limit: usize) {
        if limit == 0 {
            return;
        }
        let mut values = Vec::new();
        let mut registry = self
            .shared
            .registry
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for _ in 0..limit {
            let Some(value) = registry.fallback.pop_front() else {
                break;
            };
            values.push(value);
        }
        drop(registry);
        output.extend(values);
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut registry = self
            .shared
            .registry
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        registry.closed = true;
        self.shared.closed.store(true, Ordering::Release);
        let pending = std::mem::take(&mut registry.pending);
        let fallback = std::mem::take(&mut registry.fallback);
        drop(registry);
        drop(pending);
        drop(fallback);
    }
}
