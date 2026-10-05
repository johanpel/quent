// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Simplified models of publication, segment links, and mutex-protected admission.
//!
//! These tests do not execute the channel implementation or model its concurrent
//! segment transition, second empty check, or segment reuse.

use loom::{
    cell::UnsafeCell,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

struct MockRing {
    slot: UnsafeCell<Option<usize>>,
    published: AtomicBool,
    consumed: AtomicBool,
}

impl MockRing {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            slot: UnsafeCell::new(None),
            published: AtomicBool::new(false),
            consumed: AtomicBool::new(false),
        })
    }

    fn push(&self, value: usize) {
        self.slot.with_mut(|ptr| unsafe { *ptr = Some(value) });
        self.published.store(true, Ordering::Release);
    }

    fn take(&self) -> Option<usize> {
        if !self.published.load(Ordering::Acquire) || self.consumed.swap(true, Ordering::Relaxed) {
            return None;
        }
        self.slot.with(|ptr| unsafe { *ptr })
    }
}

struct Handoff {
    next_ring: Arc<MockRing>,
    next_node: Arc<Node>,
}

struct Node {
    handoff: Mutex<Option<Handoff>>,
    closed: AtomicBool,
}

impl Node {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            handoff: Mutex::new(None),
            closed: AtomicBool::new(false),
        })
    }
}

#[test]
fn model_segment_link_preserves_values_after_producer_join() {
    loom::model(|| {
        let first_ring = MockRing::new();
        let first_node = Node::new();
        let producer_ring = Arc::clone(&first_ring);
        let producer_node = Arc::clone(&first_node);
        let producer = thread::spawn(move || {
            producer_ring.push(1);
            let next_ring = MockRing::new();
            let next_node = Node::new();
            *producer_node.handoff.lock().unwrap() = Some(Handoff {
                next_ring: Arc::clone(&next_ring),
                next_node: Arc::clone(&next_node),
            });
            drop(producer_node);
            next_ring.push(2);
            next_node.closed.store(true, Ordering::Release);
            drop(next_node);
        });

        let ring = first_ring;
        let mut node = first_node;
        let mut values = Vec::new();
        if let Some(value) = ring.take() {
            values.push(value);
        }
        producer.join().unwrap();
        if let Some(value) = ring.take() {
            values.push(value);
        }
        let handoff = Arc::get_mut(&mut node)
            .unwrap()
            .handoff
            .get_mut()
            .unwrap()
            .take()
            .unwrap();
        values.push(handoff.next_ring.take().unwrap());
        node = handoff.next_node;
        assert_eq!(values, [1, 2]);
        assert!(node.closed.load(Ordering::Acquire));
    });
}

#[test]
fn model_observed_closure_makes_the_last_value_visible() {
    loom::model(|| {
        let ring = MockRing::new();
        let mut node = Node::new();
        let writer_ring = Arc::clone(&ring);
        let writer_node = Arc::clone(&node);
        let producer = thread::spawn(move || {
            writer_ring.push(7);
            writer_node.closed.store(true, Ordering::Release);
            drop(writer_node);
        });

        let first = if node.closed.load(Ordering::Acquire) {
            let value = ring.take();
            assert_eq!(value, Some(7));
            value
        } else {
            None
        };
        if first.is_some() {
            assert_eq!(first, Some(7));
        }
        producer.join().unwrap();
        assert!(Arc::get_mut(&mut node).is_some());
        assert!(node.closed.load(Ordering::Acquire));
        assert_eq!(first.or_else(|| ring.take()), Some(7));
    });
}

#[test]
fn model_mutex_serializes_admission_and_closure() {
    loom::model(|| {
        let registry = Arc::new(Mutex::new((false, 0usize)));
        let sender_registry = Arc::clone(&registry);
        let sender = thread::spawn(move || {
            let mut state = sender_registry.lock().unwrap();
            if !state.0 {
                state.1 += 1;
                true
            } else {
                false
            }
        });
        let mut state = registry.lock().unwrap();
        state.0 = true;
        drop(state);
        let accepted = sender.join().unwrap();
        let state = registry.lock().unwrap();
        assert_eq!(state.1, usize::from(accepted));
        assert!(state.0);
    });
}
