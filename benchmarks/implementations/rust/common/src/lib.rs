// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use clap::Args;
use quent_bench_types::CaseResult;
use std::error::Error;
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub type BenchResult<T> = Result<T, Box<dyn Error>>;

/// Measurement arguments shared by Rust benchmark implementations.
#[derive(Args, Clone, Copy)]
pub struct WorkloadArgs {
    #[arg(long)]
    pub threads: NonZeroUsize,
    #[arg(long)]
    pub num_batches: NonZeroUsize,
    #[arg(long)]
    pub batch_size: NonZeroU64,
    #[arg(long)]
    pub num_warmup_batches: usize,
    #[arg(long = "batch-pause-us")]
    pub batch_pause_interval_us: u64,
    #[arg(long)]
    pub no_preflight_call: bool,
}

impl WorkloadArgs {
    pub fn batch_config(self) -> BatchConfig {
        BatchConfig {
            num_batches: self.num_batches.get(),
            batch_size: self.batch_size.get(),
            num_warmup_batches: self.num_warmup_batches,
            batch_pause_interval_us: self.batch_pause_interval_us,
        }
    }

    pub fn preflight_call(self) -> bool {
        !self.no_preflight_call
    }
}

#[derive(Clone, Copy)]
pub struct BatchConfig {
    pub num_batches: usize,
    pub batch_size: u64,
    pub num_warmup_batches: usize,
    pub batch_pause_interval_us: u64,
}

pub fn make_case_result<I, E, S>(
    implementation: I,
    exporter: Option<E>,
    event_shape: Option<S>,
    workload: WorkloadArgs,
    thread_batch_elapsed_ns: Vec<Vec<u64>>,
) -> CaseResult<I, E, S> {
    let average_ns_per_iteration = thread_batch_elapsed_ns
        .iter()
        .flatten()
        .map(|duration| *duration as f64)
        .sum::<f64>()
        / workload.threads.get() as f64
        / workload.num_batches.get() as f64
        / workload.batch_size.get() as f64;
    CaseResult {
        implementation,
        exporter,
        event_shape,
        threads: workload.threads.get(),
        num_batches: workload.num_batches.get(),
        batch_size: workload.batch_size.get(),
        num_warmup_batches: workload.num_warmup_batches,
        batch_pause_interval_us: workload.batch_pause_interval_us,
        preflight_call: workload.preflight_call(),
        child_pid: std::process::id(),
        thread_batch_elapsed_ns,
        average_ns_per_iteration,
    }
}

pub fn measure_threads<H, P, PrepareFn, EmitFn, E>(
    workload: WorkloadArgs,
    mut make_handle: impl FnMut() -> H,
    prepare: PrepareFn,
    emit: EmitFn,
) -> BenchResult<Vec<Vec<u64>>>
where
    H: Send + 'static,
    P: Send + 'static,
    PrepareFn: Fn(u64) -> P + Copy + Send + 'static,
    EmitFn: Fn(&H, P) -> Result<(), E> + Copy + Send + 'static,
    E: Error + Send + 'static,
{
    let config = workload.batch_config();
    let threads = workload.threads.get();
    let preflight_call = workload.preflight_call();
    let total_batches = config.num_warmup_batches + config.num_batches;
    let barrier = Arc::new(Barrier::new(threads + 1));
    let failed = Arc::new(AtomicBool::new(false));
    let mut joins = Vec::with_capacity(threads);
    for _ in 0..threads {
        let handle = make_handle();
        let barrier = Arc::clone(&barrier);
        let failed = Arc::clone(&failed);
        joins.push(std::thread::spawn(move || {
            let preflight_result = if preflight_call {
                emit(&handle, prepare(0))
            } else {
                Ok(())
            };
            if preflight_result.is_err() {
                failed.store(true, Ordering::SeqCst);
            }
            let batches = if preflight_result.is_ok() {
                (0..total_batches)
                    .map(|batch| {
                        let offset = (batch as u64).wrapping_mul(config.batch_size);
                        (0..config.batch_size)
                            .map(|index| prepare(offset.wrapping_add(index)))
                            .collect::<Vec<P>>()
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            barrier.wait();
            preflight_result?;
            if failed.load(Ordering::SeqCst) {
                return Ok(Vec::new());
            }
            let mut durations = Vec::with_capacity(config.num_batches);
            for (batch, payloads) in batches.into_iter().enumerate() {
                barrier.wait();
                let start = (batch >= config.num_warmup_batches).then(Instant::now);
                // Keep the batch buffer alive through the end timestamp.
                let mut calls = payloads.into_iter();
                let batch_result = calls
                    .by_ref()
                    .try_for_each(|payload| emit(&handle, payload));
                let elapsed = start.map(|start| start.elapsed().as_nanos());
                drop(calls);
                if batch_result.is_err() {
                    failed.store(true, Ordering::SeqCst);
                }
                barrier.wait();
                batch_result?;
                if let Some(elapsed) = elapsed {
                    durations.push(elapsed);
                }
                if failed.load(Ordering::SeqCst) {
                    break;
                }
                if batch + 1 < total_batches && config.batch_pause_interval_us > 0 {
                    let pause_started = Instant::now();
                    let pause = Duration::from_micros(config.batch_pause_interval_us);
                    while pause_started.elapsed() < pause {}
                }
            }
            Ok::<Vec<u128>, E>(durations)
        }));
    }
    barrier.wait();
    if !failed.load(Ordering::SeqCst) {
        for _ in 0..total_batches {
            barrier.wait();
            barrier.wait();
            if failed.load(Ordering::SeqCst) {
                break;
            }
        }
    }
    joins
        .into_iter()
        .map(|join| -> BenchResult<Vec<u64>> {
            join.join()
                .map_err(|_| "benchmark thread panicked")??
                .into_iter()
                .map(|elapsed| Ok(u64::try_from(elapsed)?))
                .collect()
        })
        .collect()
}
