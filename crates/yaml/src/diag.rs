// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Located diagnostics for YAML model sources.

use saphyr_parser::Span;

/// A single located problem in a YAML model source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The source file name, or `"<input>"` for [`crate::load_str`].
    pub file: String,
    /// Source line, counted from 1 as editors display it.
    pub line: usize,
    /// Source column, counted from 1 as editors display it.
    pub column: usize,
    /// Dotted semantic path, e.g. `entities.Engine.events.started.once.load`.
    pub path: String,
    /// What is wrong.
    pub message: String,
    /// Optional hint on how to fix it.
    pub help: Option<String>,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}: {} ({})",
            self.file, self.line, self.column, self.message, self.path
        )?;
        if let Some(help) = &self.help {
            write!(f, "\n  help: {help}")?;
        }
        Ok(())
    }
}

/// A non-empty collection of [`Diagnostic`]s from one load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostics(pub(crate) Vec<Diagnostic>);

impl Diagnostics {
    /// The diagnostics, in source order of detection.
    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> + '_ {
        self.0.iter()
    }
}

impl std::fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, d) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{d}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Diagnostics {}

impl IntoIterator for Diagnostics {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// Collector that stamps every diagnostic with the source file name.
///
/// Lowering pushes into one shared sink so a single run reports every problem
/// instead of aborting at the first.
pub(crate) struct Sink {
    file: String,
    out: Vec<Diagnostic>,
}

impl Sink {
    pub(crate) fn new(file: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            out: Vec::new(),
        }
    }

    pub(crate) fn error(
        &mut self,
        span: Span,
        path: &str,
        message: impl Into<String>,
        help: Option<String>,
    ) {
        self.out.push(self.make(span, path, message.into(), help));
    }

    pub(crate) fn make(
        &self,
        span: Span,
        path: &str,
        message: String,
        help: Option<String>,
    ) -> Diagnostic {
        Diagnostic {
            file: self.file.clone(),
            line: span.start.line(),
            // saphyr marker columns count from 0; displayed columns from 1.
            column: span.start.col() + 1,
            path: path.to_string(),
            message,
            help,
        }
    }

    pub(crate) fn has_errors(&self) -> bool {
        !self.out.is_empty()
    }

    pub(crate) fn into_diagnostics(self) -> Diagnostics {
        Diagnostics(self.out)
    }
}

/// Return the closest of `candidates` to `name`, if any is close enough to be
/// a plausible typo.
pub(crate) fn suggest<'c>(
    name: &str,
    candidates: impl IntoIterator<Item = &'c str>,
) -> Option<&'c str> {
    let max_distance = (name.len() / 3).max(1);
    candidates
        .into_iter()
        .map(|c| (strsim::levenshtein(name, c), c))
        .filter(|&(d, _)| d <= max_distance)
        .min_by_key(|&(d, _)| d)
        .map(|(_, c)| c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggest_picks_closest_within_bound() {
        assert_eq!(suggest("strin", ["string", "bool", "u16"]), Some("string"));
        assert_eq!(suggest("zzz", ["string", "bool"]), None);
    }
}
