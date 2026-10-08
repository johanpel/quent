// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    process::Command,
};

use quent_bench_types::frameworks::quent::{Channel, Clock, Exporter, variant_label};

use crate::{
    BenchResult, SharedArgs,
    case::{CaseRunner, run_child},
    langs::rust,
    progress::BuildProgress,
    report::CaseResult,
};

/// Selects the Quent-specific parameters.
#[derive(clap::Args)]
#[group(skip)]
pub(crate) struct Args {
    #[arg(
        long = "quent-channel",
        value_enum,
        value_delimiter = ',',
        default_value = "tokio,per-thread"
    )]
    channel: Vec<Channel>,
    #[arg(
        long = "quent-clock",
        value_enum,
        value_delimiter = ',',
        default_value = "std,quanta"
    )]
    clock: Vec<Clock>,
    #[arg(
        long = "quent-exporter",
        value_enum,
        value_delimiter = ',',
        default_value = "noop,ndjson,msgpack,postcard"
    )]
    exporter: Vec<Exporter>,
}

impl Args {
    pub(crate) fn build_count(&self) -> usize {
        self.channel.len() * self.clock.len()
    }
}

/// Defines one case for each requested exporter, event shape, and thread count.
pub(crate) fn cases(
    shared: &SharedArgs,
    options: &Args,
    binaries: &Path,
    progress: &mut BuildProgress,
) -> BenchResult<Vec<Box<dyn CaseRunner>>> {
    let mut cases = Vec::new();
    for channel in &options.channel {
        for clock in &options.clock {
            let variant = variant_label(*channel, *clock);
            let label = format!("quent-bench-rust-{variant}");
            let mut features = channel.features().to_vec();
            features.extend_from_slice(clock.features());
            let executable =
                rust::binary_with_features("quent-bench-rust-quent", &label, &features, progress)?;
            // Cargo uses the same output path for feature builds. Keep a
            // private copy of each binary until all cases have finished.
            let saved = binaries.join(format!("{variant}{}", std::env::consts::EXE_SUFFIX));
            std::fs::copy(executable, &saved)?;
            for exporter in &options.exporter {
                for event_shape in &shared.event_shape {
                    for threads in &shared.threads {
                        cases.push(Box::new(QuentCase {
                            executable: saved.clone(),
                            channel: *channel,
                            clock: *clock,
                            exporter: *exporter,
                            event_shape: *event_shape,
                            threads: *threads,
                        }) as Box<dyn CaseRunner>);
                    }
                }
            }
        }
    }
    Ok(cases)
}

struct QuentCase {
    executable: PathBuf,
    channel: Channel,
    clock: Clock,
    exporter: Exporter,
    event_shape: quent_bench_types::EventShape,
    threads: NonZeroUsize,
}

impl CaseRunner for QuentCase {
    fn label(&self) -> String {
        format!(
            "{}, {}, {}, threads={}",
            variant_label(self.channel, self.clock),
            self.exporter.as_str(),
            self.event_shape.as_ref(),
            self.threads
        )
    }

    fn run(&self, shared: &SharedArgs) -> BenchResult<CaseResult> {
        let mut command = Command::new(&self.executable);
        command
            .args(["--exporter", self.exporter.as_str()])
            .args(["--event-shape", self.event_shape.as_ref()]);
        shared.apply_workload(&mut command, self.threads);
        run_child(command)
    }
}
