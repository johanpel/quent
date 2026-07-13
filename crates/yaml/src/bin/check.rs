// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! `quent-yaml-check`: load YAML model files and report located diagnostics.
//!
//! Usage: `quent-yaml-check [--warnings] <file>...`. Exits non-zero if any file
//! fails to load. With `--warnings`, also reports unregistered-constraint
//! warnings and style lints. Diagnostics are emitted as `tracing` events on
//! stderr.

use std::fmt::Write as _;
use std::process::ExitCode;

use quent_yaml::{Diagnostic, Error};
use tracing::{error, info, warn};

fn main() -> ExitCode {
    // Compiler-style diagnostics: timestamps and targets would clutter the
    // caret blocks.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .without_time()
        .with_max_level(tracing::Level::INFO)
        .init();

    let mut warnings = false;
    let mut files = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--warnings" => warnings = true,
            "--help" | "-h" => {
                println!("usage: quent-yaml-check [--warnings] <file>...");
                return ExitCode::SUCCESS;
            }
            _ => files.push(arg),
        }
    }
    if files.is_empty() {
        error!("usage: quent-yaml-check [--warnings] <file>...");
        return ExitCode::FAILURE;
    }

    let mut failed = false;
    for file in &files {
        failed |= !check(file, warnings);
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn check(file: &str, warnings: bool) -> bool {
    let src = match std::fs::read_to_string(file) {
        Ok(src) => src,
        Err(e) => {
            error!("{file}: {e}");
            return false;
        }
    };
    match quent_yaml::load_str_named(&src, file) {
        Ok(loaded) => {
            if warnings {
                for warning in &loaded.warnings {
                    warn!("{}", render(&src, warning));
                }
                for lint in quent_yaml::lint(&src, file) {
                    warn!("{}", render(&src, &lint));
                }
            }
            let events: usize = loaded.schema.entities().map(|e| e.events().count()).sum();
            info!(
                "{file}: ok — model `{}`: {} records, {} entities, {events} events",
                loaded.schema.name(),
                loaded.schema.records().count(),
                loaded.schema.entities().count(),
            );
            true
        }
        // Unreachable: the source was already read, so only diagnostics
        // remain.
        Err(Error::Io(e)) => {
            error!("{file}: {e}");
            false
        }
        Err(Error::Invalid(diagnostics)) => {
            for diagnostic in diagnostics.iter() {
                error!("{}", render(&src, diagnostic));
            }
            false
        }
    }
}

/// Render one diagnostic with its source line and a caret under the column,
/// as a single multi-line block so the event stays together.
fn render(src: &str, diagnostic: &Diagnostic) -> String {
    let mut out = format!(
        "{}:{}:{}: {}",
        diagnostic.file, diagnostic.line, diagnostic.column, diagnostic.message
    );
    if let Some(line) = src.lines().nth(diagnostic.line.saturating_sub(1)) {
        let gutter = format!("{:>4} | ", diagnostic.line);
        let padding = " ".repeat(gutter.len() + diagnostic.column.saturating_sub(1));
        write!(out, "\n{gutter}{line}\n{padding}^").expect("writing to a String");
    }
    if let Some(help) = &diagnostic.help {
        write!(out, "\n     = help: {help}").expect("writing to a String");
    }
    out
}
