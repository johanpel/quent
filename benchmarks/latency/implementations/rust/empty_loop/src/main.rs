// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::convert::Infallible;
use std::hint::black_box;

use clap::Parser;
use quent_latency_common::{BenchResult, WorkloadArgs, make_case_result, measure_threads};
use quent_latency_types::CaseResult;

#[derive(Parser)]
struct Args {
    #[command(flatten)]
    workload: WorkloadArgs,
}

fn main() -> BenchResult<()> {
    let args = Args::parse();
    let durations = measure_threads(
        args.workload,
        || (),
        |_| (),
        |_, ()| {
            black_box(());
            Ok::<(), Infallible>(())
        },
    )?;
    let result: CaseResult<&str, &str, &str> =
        make_case_result("empty-loop-rs", None, None, args.workload, durations);
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
