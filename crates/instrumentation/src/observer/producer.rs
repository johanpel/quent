// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_events::Event;
use std::{
    any::Any,
    cell::{Cell, RefCell, UnsafeCell},
    mem::MaybeUninit,
    ptr,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
    },
};

const INITIAL_QUEUE_BYTES: usize = 128 * 1024;
const MIN_QUEUE_EVENTS: usize = 64;

#[repr(align(64))]
struct ProducerPosition {
    published: AtomicUsize,
}

#[repr(align(64))]
struct ConsumerPosition {
    published: AtomicUsize,
    local: UnsafeCell<usize>,
}

struct Node<T> {
    slots: Box<[UnsafeCell<MaybeUninit<Event<T>>>]>,
    deferred_timestamps: Box<[UnsafeCell<bool>]>,
    mask: usize,
    next: AtomicPtr<Node<T>>,
    producer_position: ProducerPosition,
    consumer_position: ConsumerPosition,
}

impl<T> Node<T> {
    fn new(capacity: usize) -> Box<Self> {
        let capacity = capacity.next_power_of_two();
        let mut slots = Vec::with_capacity(capacity);
        slots.resize_with(capacity, || UnsafeCell::new(MaybeUninit::uninit()));
        let mut deferred_timestamps = Vec::with_capacity(capacity);
        deferred_timestamps.resize_with(capacity, || UnsafeCell::new(false));
        Box::new(Self {
            slots: slots.into_boxed_slice(),
            deferred_timestamps: deferred_timestamps.into_boxed_slice(),
            mask: capacity - 1,
            next: AtomicPtr::new(ptr::null_mut()),
            producer_position: ProducerPosition {
                published: AtomicUsize::new(0),
            },
            consumer_position: ConsumerPosition {
                published: AtomicUsize::new(0),
                local: UnsafeCell::new(0),
            },
        })
    }
}

impl<T> Drop for Node<T> {
    fn drop(&mut self) {
        let mut read = *self.consumer_position.local.get_mut();
        let write = self.producer_position.published.load(Ordering::Relaxed);
        while read != write {
            // The queue is exclusively owned during drop, and every published
            // position contains one initialized event.
            unsafe {
                (*self.slots[read & self.mask].get()).assume_init_drop();
            }
            read = read.wrapping_add(1);
        }
    }
}

/// The consumer half of one thread's SPSC event queue.
pub(super) struct ProducerQueue<T> {
    consumer: UnsafeCell<*mut Node<T>>,
    producer_alive: AtomicBool,
    active_sends: AtomicUsize,
}

// The forwarder is the only consumer. The producer half stays on its creating
// thread, and publication uses release/acquire ordering.
unsafe impl<T: Send> Send for ProducerQueue<T> {}
unsafe impl<T: Send> Sync for ProducerQueue<T> {}

impl<T> ProducerQueue<T> {
    fn new() -> (Arc<Self>, ThreadProducer<T>) {
        let event_bytes = std::mem::size_of::<Event<T>>().max(1);
        let capacity = (INITIAL_QUEUE_BYTES / event_bytes)
            .max(MIN_QUEUE_EVENTS)
            .next_power_of_two();
        let node = Box::into_raw(Node::new(capacity));
        let queue = Arc::new(Self {
            consumer: UnsafeCell::new(node),
            producer_alive: AtomicBool::new(true),
            active_sends: AtomicUsize::new(0),
        });
        let producer = ThreadProducer {
            slots: unsafe { (*node).slots.as_ptr() },
            deferred_timestamps: unsafe { (*node).deferred_timestamps.as_ptr() },
            published_write: unsafe { &(*node).producer_position.published },
            published_read: unsafe { &(*node).consumer_position.published },
            capacity,
            mask: capacity - 1,
            write: 0,
            read_cache: 0,
            queue: Arc::clone(&queue),
            node,
        };
        (queue, producer)
    }

    pub(super) fn drain_into(
        &self,
        buffer: &mut Vec<Event<T>>,
        limit: usize,
        convert_timestamp: &mut impl FnMut(u64) -> u64,
    ) {
        // Only the forwarder accesses the consumer pointer and consumer positions.
        let consumer = unsafe { &mut *self.consumer.get() };
        while buffer.len() < limit {
            let node = unsafe { &**consumer };
            let read = unsafe { &mut *node.consumer_position.local.get() };
            let write = node.producer_position.published.load(Ordering::Acquire);

            while *read != write && buffer.len() < limit {
                let index = *read & node.mask;
                // Both slot arrays have `capacity` entries and `mask` is
                // `capacity - 1`, so a masked index is in bounds for both.
                let slot = unsafe { node.slots.get_unchecked(index) };
                // The acquire load above observes initialization before publication;
                // this consumer advances each slot exactly once.
                let mut event = unsafe { (*slot.get()).assume_init_read() };
                if unsafe { *node.deferred_timestamps.get_unchecked(index).get() } {
                    event.timestamp = convert_timestamp(event.timestamp);
                }
                buffer.push(event);
                *read = read.wrapping_add(1);
            }
            node.consumer_position
                .published
                .store(*read, Ordering::Release);

            if buffer.len() == limit || *read != write {
                return;
            }

            let next = node.next.load(Ordering::Acquire);
            if next.is_null() {
                return;
            }

            let previous = *consumer;
            *consumer = next;
            // Seeing `next` means the producer moved permanently to that node.
            unsafe { drop(Box::from_raw(previous)) };
        }
    }

    pub(super) fn has_events(&self) -> bool {
        let node = unsafe { &**self.consumer.get() };
        let read = unsafe { *node.consumer_position.local.get() };
        read != node.producer_position.published.load(Ordering::Acquire)
            || !node.next.load(Ordering::Acquire).is_null()
    }

    pub(super) fn producer_alive(&self) -> bool {
        self.producer_alive.load(Ordering::Acquire)
    }

    pub(super) fn begin_detached_send(&self) {
        self.active_sends.fetch_add(1, Ordering::SeqCst);
    }

    pub(super) fn end_detached_send(&self) {
        self.active_sends.fetch_sub(1, Ordering::SeqCst);
    }

    pub(super) fn has_active_sends(&self) -> bool {
        self.active_sends.load(Ordering::SeqCst) != 0
    }
}

impl<T> Drop for ProducerQueue<T> {
    fn drop(&mut self) {
        let mut node = *self.consumer.get_mut();
        while !node.is_null() {
            let next = unsafe { (*node).next.load(Ordering::Relaxed) };
            unsafe { drop(Box::from_raw(node)) };
            node = next;
        }
    }
}

/// The producer half, confined to the thread-local registry that creates it.
pub(super) struct ThreadProducer<T> {
    // These pointers target the current node. A node remains allocated until
    // this producer advances permanently to its successor.
    slots: *const UnsafeCell<MaybeUninit<Event<T>>>,
    deferred_timestamps: *const UnsafeCell<bool>,
    published_write: *const AtomicUsize,
    published_read: *const AtomicUsize,
    capacity: usize,
    mask: usize,
    write: usize,
    read_cache: usize,
    queue: Arc<ProducerQueue<T>>,
    node: *mut Node<T>,
}

impl<T> ThreadProducer<T> {
    pub(super) fn create() -> (Arc<ProducerQueue<T>>, Self) {
        ProducerQueue::new()
    }

    #[inline(always)]
    pub(super) fn push(&mut self, event: Event<T>, deferred_timestamp: bool) {
        if let Err(event) = self.try_push(event, deferred_timestamp) {
            self.expand_and_push(event, deferred_timestamp);
        }
    }

    #[inline(always)]
    fn try_push(&mut self, event: Event<T>, deferred_timestamp: bool) -> Result<(), Event<T>> {
        if !self.prepare_push() {
            return Err(event);
        }

        self.write_event(event, deferred_timestamp);
        Ok(())
    }

    #[inline(always)]
    fn prepare_push(&mut self) -> bool {
        if self.write.wrapping_sub(self.read_cache) == self.capacity {
            self.read_cache = unsafe { (*self.published_read).load(Ordering::Acquire) };
            if self.write.wrapping_sub(self.read_cache) == self.capacity {
                return false;
            }
        }
        true
    }

    #[inline(always)]
    fn write_event(&mut self, event: Event<T>, deferred_timestamp: bool) {
        let index = self.write & self.mask;
        unsafe {
            *(*self.deferred_timestamps.add(index)).get() = deferred_timestamp;
            (*(*self.slots.add(index)).get()).write(event);
        }
        self.write = self.write.wrapping_add(1);
        unsafe { (*self.published_write).store(self.write, Ordering::Release) };
    }

    pub(super) fn begin_detached_send(&self) {
        self.queue.begin_detached_send();
    }

    pub(super) fn end_detached_send(&self) {
        self.queue.end_detached_send();
    }

    #[cold]
    #[inline(never)]
    fn expand_and_push(&mut self, event: Event<T>, deferred_timestamp: bool) {
        self.expand();
        self.write_event(event, deferred_timestamp);
    }

    fn expand(&mut self) {
        let current = unsafe { &*self.node };
        let capacity = self
            .capacity
            .checked_mul(2)
            .expect("producer queue capacity overflow");
        let next = Box::into_raw(Node::new(capacity));
        current.next.store(next, Ordering::Release);
        self.node = next;
        self.slots = unsafe { (*next).slots.as_ptr() };
        self.deferred_timestamps = unsafe { (*next).deferred_timestamps.as_ptr() };
        self.published_write = unsafe { &(*next).producer_position.published };
        self.published_read = unsafe { &(*next).consumer_position.published };
        self.capacity = capacity;
        self.mask = capacity - 1;
        self.write = 0;
        self.read_cache = 0;
    }
}

impl<T> Drop for ThreadProducer<T> {
    fn drop(&mut self) {
        self.queue.producer_alive.store(false, Ordering::Release);
    }
}

struct TlsEntry {
    pipeline_id: usize,
    pipeline_lifetime: Weak<()>,
    producer: Box<dyn Any>,
}

thread_local! {
    static PRODUCERS: RefCell<Vec<TlsEntry>> = const { RefCell::new(Vec::new()) };
    static LAST_PRODUCER: Cell<CachedProducer> = const { Cell::new(CachedProducer::empty()) };
}

#[inline(always)]
fn last_producer() -> *const Cell<CachedProducer> {
    // The pointer is used only by the calling thread and never outlives the
    // operation that obtained it.
    LAST_PRODUCER.with(ptr::from_ref)
}

#[derive(Clone, Copy)]
struct CachedProducer {
    pipeline_id: usize,
    producer: *mut (),
}

impl CachedProducer {
    const fn empty() -> Self {
        Self {
            pipeline_id: 0,
            producer: ptr::null_mut(),
        }
    }
}

struct RestoreCachedProducer<'a> {
    cache: &'a Cell<CachedProducer>,
    producer: CachedProducer,
}

impl Drop for RestoreCachedProducer<'_> {
    fn drop(&mut self) {
        self.cache.set(self.producer);
    }
}

#[inline(always)]
pub(super) fn with_thread_producer<T, R>(
    pipeline_id: usize,
    pipeline_lifetime: &Arc<()>,
    create: impl FnOnce() -> ThreadProducer<T>,
    operation: impl FnOnce(&mut ThreadProducer<T>) -> R,
) -> R
where
    T: 'static,
{
    let cache = unsafe { &*last_producer() };
    let cached = cache.get();
    if cached.pipeline_id == pipeline_id {
        assert!(
            !cached.producer.is_null(),
            "reentrant emission into one producer queue"
        );
        cache.set(CachedProducer {
            pipeline_id,
            producer: ptr::null_mut(),
        });
        let _restore = RestoreCachedProducer {
            cache,
            producer: cached,
        };
        return operation(unsafe { &mut *cached.producer.cast::<ThreadProducer<T>>() });
    }

    cache.set(CachedProducer::empty());
    let producer = PRODUCERS.with_borrow_mut(|entries| {
        let index = entries
            .iter()
            .position(|entry| entry.pipeline_id == pipeline_id)
            .unwrap_or_else(|| {
                entries.retain(|entry| entry.pipeline_lifetime.strong_count() != 0);
                entries.push(TlsEntry {
                    pipeline_id,
                    pipeline_lifetime: Arc::downgrade(pipeline_lifetime),
                    producer: Box::new(create()),
                });
                entries.len() - 1
            });
        entries[index]
            .producer
            .downcast_mut::<ThreadProducer<T>>()
            .expect("pipeline ID must identify one event type") as *mut ThreadProducer<T>
    });
    let cached = CachedProducer {
        pipeline_id,
        producer: producer.cast(),
    };
    cache.set(CachedProducer {
        pipeline_id,
        producer: ptr::null_mut(),
    });
    let _restore = RestoreCachedProducer {
        cache,
        producer: cached,
    };
    operation(unsafe { &mut *producer })
}

#[inline(always)]
pub(super) fn push_thread_event<T>(
    pipeline_id: usize,
    pipeline_lifetime: &Arc<()>,
    create: impl FnOnce() -> ThreadProducer<T>,
    event: Event<T>,
    deferred_timestamp: bool,
) where
    T: 'static,
{
    let cache = unsafe { &*last_producer() };
    let cached = cache.get();
    if cached.pipeline_id == pipeline_id && !cached.producer.is_null() {
        let producer = unsafe { &mut *cached.producer.cast::<ThreadProducer<T>>() };
        if let Err(event) = producer.try_push(event, deferred_timestamp) {
            push_cached_slow(cache, cached, producer, event, deferred_timestamp);
        }
        return;
    }
    push_uncached(
        cache,
        pipeline_id,
        pipeline_lifetime,
        create,
        event,
        deferred_timestamp,
    );
}

#[cold]
#[inline(never)]
fn push_cached_slow<T>(
    cache: &Cell<CachedProducer>,
    cached: CachedProducer,
    producer: &mut ThreadProducer<T>,
    event: Event<T>,
    deferred_timestamp: bool,
) {
    cache.set(CachedProducer {
        pipeline_id: cached.pipeline_id,
        producer: ptr::null_mut(),
    });
    let _restore = RestoreCachedProducer {
        cache,
        producer: cached,
    };
    producer.expand_and_push(event, deferred_timestamp);
}

#[cold]
#[inline(never)]
fn push_uncached<T>(
    cache: &Cell<CachedProducer>,
    pipeline_id: usize,
    pipeline_lifetime: &Arc<()>,
    create: impl FnOnce() -> ThreadProducer<T>,
    event: Event<T>,
    deferred_timestamp: bool,
) where
    T: 'static,
{
    with_thread_producer(pipeline_id, pipeline_lifetime, create, |producer| {
        producer.push(event, deferred_timestamp);
    });
    debug_assert_eq!(cache.get().pipeline_id, pipeline_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn grows_and_preserves_order() {
        let (queue, mut producer) = ProducerQueue::new();
        let initial_capacity = producer.capacity;
        for value in 0..initial_capacity + 17 {
            producer.push(Event::new_now(Uuid::nil(), value), false);
        }

        let mut values = Vec::new();
        let mut events = Vec::new();
        while queue.has_events() {
            queue.drain_into(&mut events, 31, &mut |timestamp| timestamp);
            values.extend(events.drain(..).map(|event| event.data));
        }
        assert_eq!(values.len(), initial_capacity + 17);
        assert!(values.into_iter().eq(0..initial_capacity + 17));
    }

    #[test]
    fn reuses_space_after_consumer_progress() {
        let (queue, mut producer) = ProducerQueue::new();
        let capacity = producer.capacity;
        for value in 0..capacity {
            producer.push(Event::new_now(Uuid::nil(), value), false);
        }
        let mut events = Vec::new();
        queue.drain_into(&mut events, capacity, &mut |timestamp| timestamp);
        assert_eq!(events.len(), capacity);
        events.clear();
        for value in capacity..capacity * 2 {
            producer.push(Event::new_now(Uuid::nil(), value), false);
        }
        queue.drain_into(&mut events, capacity, &mut |timestamp| timestamp);
        assert_eq!(events.len(), capacity);
    }

    #[test]
    fn converts_only_deferred_timestamps() {
        let (queue, mut producer) = ProducerQueue::new();
        producer.push(Event::new(Uuid::nil(), 41, 1), false);
        producer.push(Event::new(Uuid::nil(), 42, 2), true);

        let mut events = Vec::new();
        queue.drain_into(&mut events, 2, &mut |timestamp| timestamp + 100);

        assert_eq!(events[0].timestamp, 41);
        assert_eq!(events[1].timestamp, 142);
    }

    #[test]
    fn concurrent_producer_and_consumer_preserve_every_event() {
        const EVENT_COUNT: usize = 100_000;

        let (queue_tx, queue_rx) = std::sync::mpsc::sync_channel(0);
        let producer_thread = std::thread::spawn(move || {
            let (queue, mut producer) = ProducerQueue::new();
            queue_tx.send(queue).unwrap();
            for value in 0..EVENT_COUNT {
                producer.push(Event::new_now(Uuid::nil(), value), false);
            }
        });

        let queue = queue_rx.recv().unwrap();
        let mut values = Vec::with_capacity(EVENT_COUNT);
        let mut events = Vec::new();
        while queue.producer_alive() || queue.has_events() {
            queue.drain_into(&mut events, 257, &mut |timestamp| timestamp);
            values.extend(events.drain(..).map(|event| event.data));
            std::thread::yield_now();
        }
        producer_thread.join().unwrap();

        assert_eq!(values.len(), EVENT_COUNT);
        assert!(values.into_iter().eq(0..EVENT_COUNT));
    }
}
