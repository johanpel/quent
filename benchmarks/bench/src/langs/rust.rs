// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::Command;

use crate::case::{CaseRunner, run_child};
use crate::report::CaseResult;
use crate::{BenchResult, SharedArgs};

/// Builds a Rust implementation in release mode and returns its executable path.
///
/// The Cargo subprocess inherits the caller's environment, including an active Pixi environment.
pub(crate) fn binary(package: &str) -> BenchResult<PathBuf> {
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

/// Defines one Rust empty-loop case for each requested thread count.
pub(crate) fn empty_loop_cases(shared: &SharedArgs) -> BenchResult<Vec<Box<dyn CaseRunner>>> {
    let executable = binary("quent-bench-rust-empty-loop")?;
    let mut cases = Vec::with_capacity(shared.threads.len());
    for threads in &shared.threads {
        cases.push(Box::new(EmptyLoopCase {
            executable: executable.clone(),
            threads: *threads,
        }) as Box<dyn CaseRunner>);
    }
    Ok(cases)
}

struct EmptyLoopCase {
    executable: PathBuf,
    threads: NonZeroUsize,
}

impl CaseRunner for EmptyLoopCase {
    fn run(&self, shared: &SharedArgs) -> BenchResult<CaseResult> {
        let mut command = Command::new(&self.executable);
        shared.apply_workload(&mut command, self.threads);
        run_child(command)
    }
}
