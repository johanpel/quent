// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Microbenchmarks for the instrumentation API hot path, using the noop exporter.
//!
//! Measures the overhead of `EventSender::send` and `EventSender::emit` when the
//! underlying channel is disabled (i.e. `Context::try_new(_, None)`). This is the
//! cost a caller pays when instrumentation is compiled in but not active.

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use quent_events::Event;
use quent_instrumentation::Context;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
struct BenchEvent;

fn bench_send_noop(c: &mut Criterion) {
    let ctx = Context::<BenchEvent>::try_new(Uuid::now_v7(), None).unwrap();
    let sender = ctx.events_sender();
    let id = Uuid::now_v7();

    let mut group = c.benchmark_group("event_sender_noop");
    group.throughput(Throughput::Elements(1));

    group.bench_function("send", |b| {
        b.iter(|| {
            sender.send(black_box(Event::new_now(id, BenchEvent)));
        });
    });

    group.bench_function("emit", |b| {
        b.iter(|| {
            sender.emit(black_box(id), black_box(BenchEvent));
        });
    });

    group.finish();
}

criterion_group!(benches, bench_send_noop);
criterion_main!(benches);
