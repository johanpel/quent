// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! YAML source format for [`quent_schema`] application event models, format 1.
//!
//! A model file declares `quent: 1`, a `model` name, `records`, and `entities`
//! with `once`/`multi` events. Field types use a Rust-spelled mini-language
//! (`String`, `Vec<T>`, `Option<T>` with `T?` sugar, `Ref`, `Uuid`, `Dynamic`,
//! the integer and float primitives, and bare record names). Every level takes
//! `doc:` plus generic `constraints:`/`metadata:` annotation maps.
//!
//! Loading parses (YAML 1.2, spanned), lowers through the [`quent_schema`]
//! builders, and validates the always-on base constraints. Constraint
//! annotations are opaque pass-through data: nothing validates them at load
//! time, and each is surfaced once as a [`Loaded::warnings`] entry. All
//! failures are located [`Diagnostics`].

use std::path::Path;

use quent_constraints::validate;
use quent_schema::Schema;

mod diag;
mod json;
mod lint;
mod lower;
mod tree;
mod types;
mod walk;

pub use diag::{Diagnostic, Diagnostics};
pub use lint::lint;

/// A successfully loaded model.
#[derive(Debug)]
pub struct Loaded {
    pub schema: Schema,
    /// Constraint names in the model that no validator handles. These are
    /// passed through for downstream validators, not errors.
    pub warnings: Vec<Diagnostic>,
}

/// Failure while loading a YAML model source.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(Diagnostics),
}

/// Parse and lower YAML source text into a validated [`Schema`].
///
/// Diagnostics name the source as `<input>`; see [`load_str_named`] and
/// [`load`] to name it.
pub fn load_str(src: &str) -> Result<Loaded, Error> {
    load_str_named(src, "<input>")
}

/// Read a YAML model file and load it via [`load_str`] semantics.
pub fn load(path: impl AsRef<Path>) -> Result<Loaded, Error> {
    let path = path.as_ref();
    let src = std::fs::read_to_string(path)?;
    load_str_named(&src, &path.display().to_string())
}

/// Like [`load_str`], naming the source `file` in diagnostics.
pub fn load_str_named(src: &str, file: &str) -> Result<Loaded, Error> {
    let mut sink = diag::Sink::new(file);
    let Some(root) = tree::parse(src, &mut sink) else {
        return Err(Error::Invalid(sink.into_diagnostics()));
    };
    let Some((schema, map)) = lower::lower(&root, &mut sink) else {
        return Err(Error::Invalid(sink.into_diagnostics()));
    };
    if sink.has_errors() {
        return Err(Error::Invalid(sink.into_diagnostics()));
    }

    let report = validate::<()>(&schema);
    if let Err(e) = report.base_constraints {
        for record in e.recursive_records {
            let span = map
                .record_spans
                .get(&record)
                .copied()
                .unwrap_or(map.model_span);
            sink.error(
                span,
                &format!("records.{record}"),
                format!("record `{record}` is recursive"),
                Some(
                    "records cannot contain themselves, directly or through other records"
                        .to_string(),
                ),
            );
        }
        // Unresolved references are pre-empted by the loader's own deferred
        // checks; this is a backstop for anything those miss.
        for reference in e.invalid_references {
            sink.error(
                map.model_span,
                "",
                format!("unresolved reference: {reference}"),
                None,
            );
        }
    }
    if sink.has_errors() {
        return Err(Error::Invalid(sink.into_diagnostics()));
    }

    let warnings = report
        .unregistered_constraints
        .into_iter()
        .map(|name| {
            let span = map
                .constraint_first
                .get(&name)
                .copied()
                .unwrap_or(map.model_span);
            sink.make(
                span,
                "",
                format!("constraint `{name}` has no registered validator"),
                Some(
                    "it is passed through untouched; a downstream validator may check it"
                        .to_string(),
                ),
            )
        })
        .collect();
    Ok(Loaded { schema, warnings })
}
