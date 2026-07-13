// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Structured access to tree mappings: key checks, duplicates, strictness.

use std::collections::HashMap;

use saphyr_parser::Span;

use crate::diag::{Sink, suggest};
use crate::tree::{Kind, Node, non_string_kind};

struct Entry<'t> {
    name: &'t str,
    key_span: Span,
    value: &'t Node,
    used: bool,
}

/// One mapping's entries with name-safety, duplicate, and strictness checks.
///
/// A null node counts as an empty mapping, so bodies whose keys are all
/// optional can be written as `Cluster:` and `Cluster: {}` interchangeably.
pub(crate) struct MapView<'t> {
    entries: Vec<Entry<'t>>,
    requested: Vec<&'static str>,
}

impl<'t> MapView<'t> {
    pub(crate) fn new(node: &'t Node, sink: &mut Sink, path: &str) -> Self {
        let mut entries: Vec<Entry<'t>> = Vec::new();
        let mut seen: HashMap<&'t str, Span> = HashMap::new();
        match &node.kind {
            _ if node.is_null() => {}
            Kind::Map(pairs) => {
                for (key, value) in pairs {
                    let Some((text, style)) = key.scalar() else {
                        sink.error(key.span, path, "mapping keys must be scalars", None);
                        continue;
                    };
                    if let Some(kind) = non_string_kind(text, style) {
                        sink.error(
                            key.span,
                            path,
                            format!("`{text}` reads as {kind} in YAML"),
                            Some("quote it if it is meant as a name".to_string()),
                        );
                        continue;
                    }
                    if let Some(first_span) = seen.get(text) {
                        sink.error(
                            key.span,
                            path,
                            format!(
                                "duplicate key `{text}` (first defined at line {})",
                                first_span.start.line()
                            ),
                            None,
                        );
                        continue;
                    }
                    seen.insert(text, key.span);
                    entries.push(Entry {
                        name: text,
                        key_span: key.span,
                        value,
                        used: false,
                    });
                }
            }
            _ => {
                sink.error(node.span, path, "expected a mapping", None);
            }
        }
        Self {
            entries,
            requested: Vec::new(),
        }
    }

    /// Consume the entry named `key`, if present.
    ///
    /// Requested keys form the known-key set that [`Self::finish_strict`]
    /// suggests from.
    pub(crate) fn take(&mut self, key: &'static str) -> Option<(&'t Node, Span)> {
        self.requested.push(key);
        let entry = self.entries.iter_mut().find(|e| !e.used && e.name == key)?;
        entry.used = true;
        Some((entry.value, entry.key_span))
    }

    /// Error on every entry not consumed by [`Self::take`], suggesting the
    /// closest requested key.
    pub(crate) fn finish_strict(self, sink: &mut Sink, path: &str) {
        for entry in self.entries.iter().filter(|e| !e.used) {
            let help = match suggest(entry.name, self.requested.iter().copied()) {
                Some(s) => format!("did you mean `{s}`?"),
                None => "is this file for a newer quent-yaml?".to_string(),
            };
            sink.error(
                entry.key_span,
                path,
                format!("unknown key `{}`", entry.name),
                Some(help),
            );
        }
    }

    /// All entries in declaration order, for name-keyed collections.
    pub(crate) fn into_entries(self) -> impl Iterator<Item = (&'t str, Span, &'t Node)> {
        self.entries
            .into_iter()
            .filter(|e| !e.used)
            .map(|e| (e.name, e.key_span, e.value))
    }
}

/// Expect a (non-null) string scalar of any style.
pub(crate) fn expect_string(node: &Node, sink: &mut Sink, path: &str) -> Option<String> {
    match node.scalar() {
        Some((text, _)) if !node.is_null() => Some(text.to_string()),
        _ => {
            sink.error(node.span, path, "expected a string", None);
            None
        }
    }
}

/// Expect a scalar used as a name, applying the same quoting rule as mapping
/// keys in name positions.
pub(crate) fn expect_name(node: &Node, sink: &mut Sink, path: &str) -> Option<String> {
    let Some((text, style)) = node.scalar() else {
        sink.error(node.span, path, "expected a name", None);
        return None;
    };
    if let Some(kind) = non_string_kind(text, style) {
        sink.error(
            node.span,
            path,
            format!("`{text}` reads as {kind} in YAML"),
            Some("quote it if it is meant as a name".to_string()),
        );
        return None;
    }
    Some(text.to_string())
}
