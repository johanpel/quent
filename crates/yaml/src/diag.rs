// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Diagnostics for model sources.
//!
//! `serde` reports parse and shape errors with a source line and column;
//! lowering reports its own problems with a dotted semantic path instead
//! (the deserializer has consumed the structure by then).

/// A single problem in a model source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The source file name, or `"<input>"` for text loaded without one.
    pub file: String,
    /// A source line and column (counted from 1), for parse-stage problems.
    pub location: Option<(usize, usize)>,
    /// Dotted semantic path, e.g. `entities.Engine.events.started`, for
    /// lowering-stage problems. Empty when a location is set.
    pub path: String,
    /// What is wrong.
    pub message: String,
    /// Optional hint on how to fix it.
    pub help: Option<String>,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.file)?;
        match self.location {
            Some((line, column)) => write!(f, ":{line}:{column}")?,
            None if !self.path.is_empty() => write!(f, " ({})", self.path)?,
            None => {}
        }
        write!(f, ": {}", self.message)?;
        if let Some(help) = &self.help {
            write!(f, "\n  help: {help}")?;
        }
        Ok(())
    }
}

/// The problems collected from one load, in the order they were detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostics(pub(crate) Vec<Diagnostic>);

impl Diagnostics {
    /// The diagnostics, in order of detection.
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

    /// Report a lowering problem at semantic `path`.
    pub(crate) fn error(&mut self, path: &str, message: impl Into<String>, help: Option<String>) {
        self.out.push(self.make(path, message.into(), help));
    }

    pub(crate) fn make(&self, path: &str, message: String, help: Option<String>) -> Diagnostic {
        Diagnostic {
            file: self.file.clone(),
            location: None,
            path: path.to_string(),
            message,
            help,
        }
    }

    /// Report a parse problem at a source `line` and `column`.
    pub(crate) fn error_at(&mut self, location: Option<(usize, usize)>, message: String) {
        self.out.push(Diagnostic {
            file: self.file.clone(),
            location,
            path: String::new(),
            message,
            help: None,
        });
    }

    pub(crate) fn has_errors(&self) -> bool {
        !self.out.is_empty()
    }

    pub(crate) fn into_diagnostics(self) -> Diagnostics {
        Diagnostics(self.out)
    }
}
