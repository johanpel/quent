// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! `quent-yaml-check`: load model files and report diagnostics.
//!
//! Usage: `quent-yaml-check [--warnings] <file>...`. Exits non-zero if any
//! file fails to load. With `--warnings`, also reports unregistered-constraint
//! warnings. Diagnostics are emitted as `tracing` events on stderr.

use std::process::ExitCode;

use quent_yaml::{Diagnostic, Error};
use tracing::{error, info, warn};

fn main() -> ExitCode {
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

/// Render one diagnostic, inserting a source line and caret under the column
/// when it is located, so the caret stays directly under the message.
///
/// Builds on the diagnostic's own `Display`: the caret block is spliced in
/// after the header line, before any `help:` line, rather than re-formatting
/// the header and help here.
fn render(src: &str, diagnostic: &Diagnostic) -> String {
    let full = diagnostic.to_string();
    let Some((line, column)) = diagnostic.location else {
        return full;
    };
    let Some(text) = src.lines().nth(line.saturating_sub(1)) else {
        return full;
    };
    let gutter = format!("{line:>4} | ");
    let padding = " ".repeat(gutter.len() + column.saturating_sub(1));
    let caret = format!("\n{gutter}{text}\n{padding}^");
    match full.split_once('\n') {
        Some((header, rest)) => format!("{header}{caret}\n{rest}"),
        None => format!("{full}{caret}"),
    }
}
