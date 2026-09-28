// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use chrono::Local;
use comfy_table::{Cell, CellAlignment, Table, presets::UTF8_FULL};
use quent_latency_types::CaseResult as SharedCaseResult;
use serde::Serialize;

use crate::system::SystemProperties;
use crate::{BenchResult, EventShape, Implementation, implementation_name};

pub(crate) type CaseResult = SharedCaseResult<Implementation, String, EventShape>;

#[derive(Serialize)]
struct Report {
    schema_version: u32,
    system: SystemProperties,
    cases: Vec<ReportedCase>,
}

#[derive(Serialize)]
struct ReportedCase {
    #[serde(flatten)]
    result: CaseResult,
    batch_statistics: BatchStatistics,
}

#[derive(Serialize)]
struct BatchStatistics {
    standard_deviation_ns_per_iteration: f64,
    p50_ns_per_iteration: f64,
    p95_ns_per_iteration: f64,
    p99_ns_per_iteration: f64,
}

pub(crate) fn write(
    cases: Vec<CaseResult>,
    system: SystemProperties,
    output: Option<PathBuf>,
) -> BenchResult<()> {
    let report = Report {
        schema_version: 5,
        system,
        cases: cases
            .into_iter()
            .map(|result| ReportedCase {
                batch_statistics: batch_statistics(&result),
                result,
            })
            .collect(),
    };
    let output = match output {
        Some(path) => path,
        None => default_output(),
    };
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &output,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    print_cases(&report.cases);
    println!("{}", output.display());
    Ok(())
}

fn print_cases(cases: &[ReportedCase]) {
    if let Some(first) = cases.first() {
        let result = &first.result;
        let mut settings = Table::new();
        settings.load_style(UTF8_FULL);
        settings.set_header(["Setting", "Value"]);
        settings.add_row(vec![
            Cell::new("Measured batches"),
            Cell::new(result.num_batches),
        ]);
        settings.add_row(vec![
            Cell::new("Batch size"),
            Cell::new(format!("{} iterations/thread", result.batch_size)),
        ]);
        settings.add_row(vec![
            Cell::new("Warmup batches"),
            Cell::new(result.num_warmup_batches),
        ]);
        settings.add_row(vec![
            Cell::new("Requested spin pause"),
            Cell::new(format!("{} µs", result.batch_pause_interval_us)),
        ]);
        settings.add_row(vec![
            Cell::new("Preflight call"),
            Cell::new(if result.preflight_call {
                "enabled"
            } else {
                "disabled"
            }),
        ]);
        eprintln!("Run settings:");
        eprintln!("{settings}");
    }
    let mut table = Table::new();
    table.load_style(UTF8_FULL);
    table.set_header([
        "Implementation",
        "Exporter",
        "Event",
        "Threads",
        "Mean",
        "SD",
        "p50",
        "p95",
        "p99",
    ]);
    for case in cases {
        let result = &case.result;
        let stats = &case.batch_statistics;
        table.add_row(vec![
            Cell::new(implementation_name(result.implementation)),
            Cell::new(result.exporter.as_deref().unwrap_or("—")),
            Cell::new(result.event_shape.map(EventShape::as_str).unwrap_or("—")),
            Cell::new(result.threads).set_alignment(CellAlignment::Right),
            number(result.average_ns_per_iteration),
            number(stats.standard_deviation_ns_per_iteration),
            number(stats.p50_ns_per_iteration),
            number(stats.p95_ns_per_iteration),
            number(stats.p99_ns_per_iteration),
        ]);
    }
    eprintln!();
    eprintln!("Batch averages (ns / single instrumentation call; empty loop: ns / iteration):");
    eprintln!("{table}");
}

fn number(value: f64) -> Cell {
    Cell::new(format!("{value:.2}")).set_alignment(CellAlignment::Right)
}

fn batch_statistics(case: &CaseResult) -> BatchStatistics {
    let mut values = (0..case.num_batches)
        .map(|batch| {
            case.thread_batch_elapsed_ns
                .iter()
                .map(|thread| thread[batch] as f64)
                .sum::<f64>()
                / case.threads as f64
                / case.batch_size as f64
        })
        .collect::<Vec<_>>();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let standard_deviation_ns_per_iteration = if values.len() > 1 {
        (values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / (values.len() - 1) as f64)
            .sqrt()
    } else {
        0.0
    };
    values.sort_by(f64::total_cmp);
    BatchStatistics {
        standard_deviation_ns_per_iteration,
        p50_ns_per_iteration: percentile(&values, 50),
        p95_ns_per_iteration: percentile(&values, 95),
        p99_ns_per_iteration: percentile(&values, 99),
    }
}

fn percentile(sorted: &[f64], percent: usize) -> f64 {
    sorted[(sorted.len() * percent - 1) / 100]
}

fn default_output() -> PathBuf {
    let timestamp = Local::now().format("%Y-%m-%d-%H-%M-%S");
    let root = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target"));
    root.join("quent-latency").join(format!(
        "results-{timestamp}-pid{}.json",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_use_one_average_per_batch() {
        let case = CaseResult {
            implementation: Implementation::EmptyLoopRs,
            exporter: None,
            event_shape: None,
            threads: 2,
            num_batches: 4,
            batch_size: 1,
            num_warmup_batches: 0,
            batch_pause_interval_us: 0,
            preflight_call: false,
            child_pid: 0,
            thread_batch_elapsed_ns: vec![vec![10, 20, 30, 40], vec![30, 40, 50, 60]],
            average_ns_per_iteration: 35.0,
        };
        let stats = batch_statistics(&case);
        assert!(
            (stats.standard_deviation_ns_per_iteration - (500.0_f64 / 3.0).sqrt()).abs() < 1e-10
        );
        assert_eq!(stats.p50_ns_per_iteration, 30.0);
        assert_eq!(stats.p95_ns_per_iteration, 50.0);
        assert_eq!(stats.p99_ns_per_iteration, 50.0);

        let values = (1..=20).map(f64::from).collect::<Vec<_>>();
        assert_eq!(percentile(&values, 95), 19.0);
        assert_eq!(percentile(&values, 99), 20.0);
    }
}
