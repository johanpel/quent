// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::{
    num::NonZeroUsize,
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use quent_channel::{mpsc::unbounded_channel_with_config, spsc::Config};

fn config() -> Config {
    Config {
        segment_capacity: NonZeroUsize::new(2).unwrap(),
        spare_segments: 1,
    }
}

#[repr(align(64))]
struct Aligned(String);

#[test]
fn aligned_owning_values_survive_wraparound_and_partial_drains() {
    let (sender, mut receiver) = unbounded_channel_with_config(config());
    let mut output = Vec::new();
    for batch in 0..10 {
        for offset in 0..2 {
            sender
                .send(Aligned((batch * 2 + offset).to_string()))
                .ok()
                .unwrap();
        }
        assert_eq!(receiver.drain_into(&mut output, 1), 1);
        assert_eq!(receiver.drain_into(&mut output, 1), 1);
    }
    receiver.close();
    while receiver.drain_into(&mut output, 3) != 0 {}
    assert_eq!(
        output.into_iter().map(|value| value.0).collect::<Vec<_>>(),
        (0..20).map(|value| value.to_string()).collect::<Vec<_>>()
    );
}

#[test]
fn zero_sized_values_cross_many_segments() {
    let (sender, mut receiver) = unbounded_channel_with_config(config());
    for _ in 0..10 {
        sender.send(()).unwrap();
    }
    receiver.close();
    let mut output = Vec::new();
    while receiver.drain_into(&mut output, 3) != 0 {}
    assert_eq!(output.len(), 10);
}

struct Counted(Arc<AtomicUsize>);

impl Drop for Counted {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn both_handle_drop_orders_destroy_unread_values_once() {
    for consumer_first in [false, true] {
        let count = Arc::new(AtomicUsize::new(0));
        let (sender, receiver) = unbounded_channel_with_config(config());
        let sent = Arc::new(Barrier::new(2));
        let exit = Arc::new(Barrier::new(2));
        let producer = thread::spawn({
            let count = Arc::clone(&count);
            let sent = Arc::clone(&sent);
            let exit = Arc::clone(&exit);
            move || {
                for _ in 0..5 {
                    assert!(sender.send(Counted(Arc::clone(&count))).is_ok());
                }
                sent.wait();
                exit.wait();
            }
        });
        sent.wait();
        if consumer_first {
            drop(receiver);
            exit.wait();
            producer.join().unwrap();
        } else {
            exit.wait();
            producer.join().unwrap();
            drop(receiver);
        }
        assert_eq!(count.load(Ordering::Relaxed), 5);
    }
}
