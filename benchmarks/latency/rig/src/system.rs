// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

use crate::BenchResult;

/// Describes the machine, toolchain, and source state recorded with a run.
///
/// Optional fields are `None` when their values could not be obtained.
#[derive(Serialize)]
pub(crate) struct SystemProperties {
    /// Time of collection, in seconds since the Unix epoch.
    captured_at_unix_seconds: u64,
    /// Operating system for which the rig was compiled.
    os: &'static str,
    /// Operating system version reported by the host.
    os_version: Option<String>,
    /// Kernel version reported by the host.
    kernel_version: Option<String>,
    /// Architecture for which the rig was compiled.
    architecture: &'static str,
    /// Brand of the first CPU reported by the host.
    cpu_model: Option<String>,
    /// Number of logical CPUs reported by the host.
    logical_cpu_count: Option<usize>,
    /// Number of physical CPU cores reported by the host.
    physical_core_count: Option<usize>,
    /// Parallelism available to this process, which may reflect resource limits.
    available_cpu_count: Option<usize>,
    /// Total RAM reported by the host, in bytes.
    total_memory_bytes: Option<u64>,
    /// Output of `rustc --version` at collection time.
    rustc_version: Option<String>,
    /// Host triple reported by `rustc -vV` at collection time.
    target_triple: Option<String>,
    /// `release` when the rig automatically built the implementation binary.
    build_profile: Option<&'static str>,
    /// Git commit at `HEAD` in the repository containing the rig.
    git_commit: Option<String>,
    /// Whether Git reports tracked or untracked changes in that repository.
    git_dirty: Option<bool>,
}

pub(crate) fn properties(automatic_build: bool) -> BenchResult<SystemProperties> {
    let root = env!("CARGO_MANIFEST_DIR");
    let git_commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned());
    let git_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty());
    let system = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::nothing().with_ram()),
    );

    Ok(SystemProperties {
        captured_at_unix_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        os: std::env::consts::OS,
        os_version: System::long_os_version(),
        kernel_version: System::kernel_version(),
        architecture: std::env::consts::ARCH,
        cpu_model: system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_owned())
            .filter(|brand| !brand.is_empty()),
        logical_cpu_count: (!system.cpus().is_empty()).then_some(system.cpus().len()),
        physical_core_count: System::physical_core_count(),
        available_cpu_count: std::thread::available_parallelism().ok().map(usize::from),
        total_memory_bytes: (system.total_memory() > 0).then_some(system.total_memory()),
        rustc_version: command_output("rustc", &["--version"]),
        target_triple: command_output("rustc", &["-vV"]).and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("host: "))
                .map(str::to_owned)
        }),
        build_profile: automatic_build.then_some("release"),
        git_commit,
        git_dirty,
    })
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}
