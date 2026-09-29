// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::num::{NonZeroU64, NonZeroUsize};
use std::process::Command;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Shared batch settings for benchmark command-line interfaces.
#[derive(clap::Args, Clone, Copy)]
pub struct BatchArgs {
    #[arg(long, default_value = "100")]
    pub num_batches: NonZeroUsize,
    #[arg(long, default_value = "20")]
    pub batch_size: NonZeroU64,
    #[arg(long, default_value_t = 10)]
    pub num_warmup_batches: usize,
    #[arg(long = "batch-pause-us", default_value_t = 10)]
    pub batch_pause_interval_us: u64,
    #[arg(long)]
    pub no_preflight_call: bool,
}

impl BatchArgs {
    /// Appends the batch settings to a benchmark implementation command.
    pub fn append_to_command(&self, command: &mut Command) {
        command
            .args(["--num-batches", &self.num_batches.to_string()])
            .args(["--batch-size", &self.batch_size.to_string()])
            .args(["--num-warmup-batches", &self.num_warmup_batches.to_string()])
            .args([
                "--batch-pause-us",
                &self.batch_pause_interval_us.to_string(),
            ]);
        if self.no_preflight_call {
            command.arg("--no-preflight-call");
        }
    }
}

/// Measurement settings for one benchmark process and its concurrent caller threads.
#[derive(clap::Args, Clone, Copy)]
pub struct MeasurementArgs {
    #[arg(long)]
    pub threads: NonZeroUsize,
    #[command(flatten)]
    pub batch: BatchArgs,
}

impl MeasurementArgs {
    /// Appends this process's measurement settings to a benchmark implementation command.
    pub fn append_to_command(&self, command: &mut Command) {
        command.args(["--threads", &self.threads.to_string()]);
        self.batch.append_to_command(command);
    }
}

/// Identifies the implementation that produced a benchmark result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, strum::AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum Implementation {
    EmptyLoopRs,
    Quent,
}

/// Selects the explicit attributes of each benchmark event.
///
/// Payload index `i` advances from zero across batches, including warmup batches;
/// the optional preflight call uses `i = 0`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum, strum::AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum EventShape {
    /// Adds no explicit attributes.
    Empty,
    /// Adds `value: u8` set to `i mod 256`.
    U8,
    /// Adds `value: u64` set to `i`.
    U64,
    /// Adds `value: string` containing `"s"` repeated `8 + (i mod 9)` times.
    ShortString,
    /// Adds `value: string` containing `"l"` repeated `128 + (i mod 129)` times.
    LongString,
    /// Adds `small`, `large`, `short`, and `long` with the values of the four shapes above.
    All,
}

/// Records the settings and measured batch durations of one child-process case.
///
/// - `I`: Implementation identifier type.
/// - `E`: Event exporting mechanism identifier type.
/// - `S`: Event shape identifier type.
#[derive(Debug, Deserialize, Serialize)]
pub struct CaseResult<I, E, S> {
    /// Names the benchmark implementation.
    pub implementation: I,
    /// Identifies how events are handled, or `None` when no output mechanism applies.
    pub exporter: Option<E>,
    /// Selects the event shape, or `None` when the case emits no event.
    pub event_shape: Option<S>,
    /// Number of threads making concurrent calls.
    pub threads: usize,
    /// Number of batches retained in the measurements.
    pub num_batches: usize,
    /// Number of calls or empty-loop iterations per thread in each batch.
    pub batch_size: u64,
    /// Number of initial batches excluded from the measurements.
    pub num_warmup_batches: usize,
    /// Minimum requested pause between batches, in microseconds.
    pub batch_pause_interval_us: u64,
    /// Whether each thread made one untimed call or empty-loop iteration before batching.
    pub preflight_call: bool,
    /// Process ID of the child that measured this case.
    pub child_pid: u32,
    /// Elapsed nanoseconds for each measured batch, grouped by thread in spawn order.
    pub thread_batch_elapsed_ns: Vec<Vec<u64>>,
    /// Sum of measured batch durations divided by `threads * num_batches * batch_size`.
    pub average_ns_per_iteration: f64,
}
