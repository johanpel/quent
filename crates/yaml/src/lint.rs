// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Style lints for model sources: legal files that other tools or readers may
//! trip over.

use convert_case::Case;
use saphyr_parser::ScalarStyle;

use crate::diag::{Diagnostic, Sink};
use crate::lower::to_case_digits;
use crate::tree::{self, Kind, Node};

/// Names YAML 1.1 parsers resolve to booleans. YAML 1.2 (and this crate) read
/// them as strings, but files stay friendlier when they are quoted.
const YAML_11_BOOLS: &[&str] = &[
    "y", "Y", "yes", "Yes", "YES", "n", "N", "no", "No", "NO", "on", "On", "ON", "off", "Off",
    "OFF",
];

/// Return style warnings for `src`.
///
/// These never fail a load: unquoted YAML 1.1 boolean lookalikes used as
/// names, record/entity names that are not CamelCase, and a model name whose
/// lowercased form (the generated file name) is not snake_case already.
pub fn lint(src: &str, file: &str) -> Vec<Diagnostic> {
    let mut sink = Sink::new(file);
    let mut parse_sink = Sink::new(file);
    let Some(root) = tree::parse(src, &mut parse_sink) else {
        // Unparseable sources have no style, only load errors.
        return Vec::new();
    };
    truthy_keys(&root, &mut sink);
    let Kind::Map(pairs) = &root.kind else {
        return sink.into_diagnostics().0;
    };
    for (key, value) in pairs {
        match key.scalar() {
            Some(("model", _)) => {
                if let Some((name, _)) = value.scalar() {
                    let snake = to_case_digits(name, Case::Snake);
                    if name != snake {
                        sink.error(
                            value.span,
                            "model",
                            format!("model name `{name}` is not snake_case"),
                            Some(format!(
                                "the generated file is named after the lowercased model name; consider `{snake}`"
                            )),
                        );
                    }
                }
            }
            Some((section @ ("records" | "entities"), _)) => {
                if let Kind::Map(decls) = &value.kind {
                    for (name_key, _) in decls {
                        let Some((name, _)) = name_key.scalar() else {
                            continue;
                        };
                        if name.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
                            sink.error(
                                name_key.span,
                                section,
                                format!("`{name}` is conventionally CamelCase"),
                                Some(
                                    "lowercase names are legal but read like type keywords"
                                        .to_string(),
                                ),
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
    sink.into_diagnostics().0
}

/// Warn on every unquoted mapping key a YAML 1.1 parser would read as a
/// boolean.
fn truthy_keys(node: &Node, sink: &mut Sink) {
    match &node.kind {
        Kind::Scalar { .. } => {}
        Kind::Seq(items) => items.iter().for_each(|item| truthy_keys(item, sink)),
        Kind::Map(pairs) => {
            for (key, value) in pairs {
                if let Some((text, ScalarStyle::Plain)) = key.scalar()
                    && YAML_11_BOOLS.contains(&text)
                {
                    sink.error(
                        key.span,
                        "",
                        format!("YAML 1.1 parsers read unquoted `{text}` as a boolean"),
                        Some(format!("write `'{text}'` to keep the file portable")),
                    );
                }
                truthy_keys(value, sink);
            }
        }
    }
}
