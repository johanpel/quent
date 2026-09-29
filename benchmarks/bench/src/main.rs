// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

mod case;
mod frameworks;
mod langs;
mod report;
mod system;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

use clap::{Args as ClapArgs, Parser, ValueEnum};
use quent_bench_types::EventShape;
use serde::{Deserialize, Serialize};

use case::CaseRunner;

type BenchResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Selects an instrumentation framework for benchmark cases.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum Framework {
    Quent,
}

/// Groups implementations so each selected language gets one empty-loop measurement.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Language {
    Rust,
}

impl Framework {
    /// Identifies the language used to deduplicate empty-loop cases across frameworks.
    fn language(self) -> Language {
        match self {
            Self::Quent => Language::Rust,
        }
    }
}

/// Combines benchmark selection, shared workload settings, and framework-specific options.
#[derive(Parser)]
#[command(about = "Measure generated instrumentation calls in isolated processes")]
struct Args {
    /// Frameworks to benchmark.
    #[arg(long, value_enum, value_delimiter = ',', default_value = "quent")]
    frameworks: Vec<Framework>,
    /// Whether to measure an empty loop for each selected language.
    #[arg(long)]
    empty_loop: bool,
    /// Path for the JSON report.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Workload settings for the run.
    #[command(flatten)]
    shared: SharedArgs,
    /// Exporter settings applied only to Quent cases.
    #[command(flatten)]
    quent: frameworks::quent::Args,
}

/// Stores cross-framework workload settings.
#[derive(ClapArgs)]
struct SharedArgs {
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "empty,u8,u64,short-string,long-string,all"
    )]
    event_shape: Vec<EventShape>,
    #[arg(long, value_delimiter = ',', default_value = "1")]
    threads: Vec<usize>,
    #[arg(long, default_value_t = 100)]
    num_batches: usize,
    #[arg(long, default_value_t = 20)]
    batch_size: u64,
    #[arg(long, default_value_t = 10)]
    num_warmup_batches: usize,
    #[arg(long = "batch-pause-us", default_value_t = 10)]
    batch_pause_interval_us: u64,
    #[arg(long)]
    no_preflight_call: bool,
}

impl SharedArgs {
    fn apply_workload(&self, command: &mut Command, threads: usize) {
        command
            .args(["--threads", &threads.to_string()])
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

impl Args {
    fn case_runners(&self) -> BenchResult<Vec<Box<dyn CaseRunner>>> {
        let mut runners = Vec::new();
        if self.empty_loop {
            let languages = self
                .frameworks
                .iter()
                .map(|framework| framework.language())
                .collect::<BTreeSet<_>>();
            for language in languages {
                match language {
                    Language::Rust => runners.extend(langs::rust::empty_loop_cases(&self.shared)?),
                }
            }
        }
        for framework in self.frameworks.iter().copied() {
            match framework {
                Framework::Quent => {
                    runners.extend(frameworks::quent::cases(&self.shared, &self.quent)?);
                }
            }
        }
        Ok(runners)
    }
}

fn main() -> BenchResult<()> {
    let args = Args::parse();
    let shared = &args.shared;
    if shared.num_batches == 0 || shared.batch_size == 0 || shared.threads.contains(&0) {
        return Err(
            "--num-batches, --batch-size, and every --threads value must be positive".into(),
        );
    }
    let system = system::properties()?;
    report::print_system(&system);
    let cases = args
        .case_runners()?
        .into_iter()
        .map(|runner| runner.run(shared))
        .collect::<BenchResult<Vec<_>>>()?;

    report::write(cases, system, args.output)
}
