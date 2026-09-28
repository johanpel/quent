// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

mod report;
mod system;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

use clap::{Args as ClapArgs, Parser, ValueEnum};
use quent_latency_types::{EventShape, Implementation};
use serde::{Deserialize, Serialize};

use report::CaseResult;

type BenchResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum Framework {
    Quent,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Language {
    Rust,
}

impl Framework {
    fn language(self) -> Language {
        match self {
            Self::Quent => Language::Rust,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum QuentExporter {
    Noop,
    Ndjson,
    Msgpack,
    Postcard,
}

impl QuentExporter {
    fn as_str(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::Ndjson => "ndjson",
            Self::Msgpack => "msgpack",
            Self::Postcard => "postcard",
        }
    }
}

#[derive(Parser)]
#[command(about = "Measure generated instrumentation calls in isolated processes")]
struct Args {
    #[command(flatten)]
    shared: SharedArgs,
    #[command(flatten)]
    quent: QuentArgs,
}

#[derive(ClapArgs)]
struct SharedArgs {
    #[arg(long, value_enum, value_delimiter = ',', default_value = "quent")]
    frameworks: Vec<Framework>,
    #[arg(long)]
    empty_loop: bool,
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
    #[arg(long)]
    output: Option<PathBuf>,
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

#[derive(ClapArgs)]
struct QuentArgs {
    #[arg(
        long = "quent-exporter",
        value_enum,
        value_delimiter = ',',
        default_value = "noop,ndjson,msgpack,postcard"
    )]
    exporter: Vec<QuentExporter>,
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
    let mut cases = Vec::new();
    if shared.empty_loop {
        let languages = shared
            .frameworks
            .iter()
            .map(|framework| framework.language())
            .collect::<BTreeSet<_>>();
        for language in languages {
            match language {
                Language::Rust => cases.extend(run_rust_empty_loop(shared)?),
            }
        }
    }
    for framework in shared.frameworks.iter().copied() {
        match framework {
            Framework::Quent => cases.extend(run_quent(shared, &args.quent)?),
        }
    }

    report::write(cases, system, args.shared.output)
}

fn run_quent(shared: &SharedArgs, quent: &QuentArgs) -> BenchResult<Vec<CaseResult>> {
    let executable = rust_binary("quent-latency-rust-quent")?;
    let mut cases = Vec::new();
    for exporter in &quent.exporter {
        for event_shape in &shared.event_shape {
            for threads in &shared.threads {
                let mut command = Command::new(&executable);
                command
                    .args(["--exporter", exporter.as_str()])
                    .args(["--event-shape", event_shape.as_ref()]);
                shared.apply_workload(&mut command, *threads);
                let output = command.output()?;
                if !output.status.success() {
                    return Err(format!(
                        "quent / {} / {} / {} threads failed: {}",
                        exporter.as_str(),
                        event_shape.as_ref(),
                        threads,
                        String::from_utf8_lossy(&output.stderr)
                    )
                    .into());
                }
                let result: CaseResult = serde_json::from_slice(&output.stdout)?;
                validate_result(
                    &result,
                    Implementation::Quent,
                    Some(exporter.as_str()),
                    Some(*event_shape),
                    *threads,
                    shared,
                    !shared.no_preflight_call,
                )?;
                cases.push(result);
            }
        }
    }
    Ok(cases)
}

fn run_rust_empty_loop(shared: &SharedArgs) -> BenchResult<Vec<CaseResult>> {
    let executable = rust_binary("quent-latency-rust-empty-loop")?;
    let mut cases = Vec::with_capacity(shared.threads.len());
    for threads in &shared.threads {
        let mut command = Command::new(&executable);
        shared.apply_workload(&mut command, *threads);
        let output = command.output()?;
        if !output.status.success() {
            return Err(format!(
                "empty-loop-rs / {threads} threads failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        let result: CaseResult = serde_json::from_slice(&output.stdout)?;
        validate_result(
            &result,
            Implementation::EmptyLoopRs,
            None,
            None,
            *threads,
            shared,
            !shared.no_preflight_call,
        )?;
        cases.push(result);
    }
    Ok(cases)
}

fn validate_result(
    result: &CaseResult,
    implementation: Implementation,
    exporter: Option<&str>,
    event_shape: Option<EventShape>,
    threads: usize,
    args: &SharedArgs,
    preflight_call: bool,
) -> BenchResult<()> {
    if result.implementation != implementation
        || result.exporter.as_deref() != exporter
        || result.event_shape != event_shape
        || result.threads != threads
        || result.num_batches != args.num_batches
        || result.batch_size != args.batch_size
        || result.num_warmup_batches != args.num_warmup_batches
        || result.batch_pause_interval_us != args.batch_pause_interval_us
        || result.preflight_call != preflight_call
        || result.thread_batch_elapsed_ns.len() != threads
        || result
            .thread_batch_elapsed_ns
            .iter()
            .any(|batches| batches.len() != args.num_batches)
        || !result.average_ns_per_iteration.is_finite()
    {
        return Err("implementation returned a result for a different case".into());
    }
    Ok(())
}

fn rust_binary(package: &str) -> BenchResult<PathBuf> {
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args([
            "build",
            "--release",
            "-p",
            package,
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "failed to build Rust implementation: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let mut executable = None;
    for line in output.stdout.split(|byte| *byte == b'\n') {
        if let Ok(message) = serde_json::from_slice::<serde_json::Value>(line)
            && message["reason"] == "compiler-artifact"
            && message["target"]["name"] == package
            && let Some(path) = message["executable"].as_str()
        {
            executable = Some(PathBuf::from(path));
        }
    }
    executable.ok_or_else(|| "Cargo did not report the Rust implementation binary".into())
}
