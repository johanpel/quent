//! QDL: a textual source language for [`quent_schema`] application event models.
//!
//! This is the minimal subset: a `model` name, `record`s, and `entity`s with
//! `once`/`multi` events. References, FSMs, resources, and constraints are not
//! yet supported.

use std::path::Path;

use chumsky::error::Cheap;
use chumsky::span::Span;
use chumsky::Parser;
use quent_schema::Schema;

pub mod ast;
mod lower;
mod parser;

pub use lower::LowerError;

/// Failure while loading a QDL source.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("parse error:\n{0}")]
    Parse(String),
    #[error(transparent)]
    Lower(#[from] LowerError),
    #[error("schema validation failed:\n{0}")]
    Validate(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Parse and lower QDL source text into a validated [`Schema`].
///
/// Runs the always-on base constraints (unresolved record references, recursive
/// records); it does not run opt-in constraints.
pub fn load_str(src: &str) -> Result<Schema, Error> {
    let ast = parser::parser().parse(src).into_result().map_err(|errs| {
        Error::Parse(
            errs.iter()
                .map(|e: &Cheap| {
                    let (line, col) = line_col(src, e.span().start());
                    format!("  unexpected input at line {line}:{col}")
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    })?;
    let schema = lower::lower(&ast)?;
    let report = quent_constraints::validate::<()>(&schema);
    if let Err(e) = report.base_constraints {
        return Err(Error::Validate(e.to_string()));
    }
    Ok(schema)
}

/// Read a QDL file and load it via [`load_str`].
pub fn load(path: impl AsRef<Path>) -> Result<Schema, Error> {
    let src = std::fs::read_to_string(path)?;
    load_str(&src)
}

/// 1-based line and column for a byte offset into `src`.
fn line_col(src: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(src.len());
    let line = src[..offset].bytes().filter(|&b| b == b'\n').count() + 1;
    let col = offset - src[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0) + 1;
    (line, col)
}
