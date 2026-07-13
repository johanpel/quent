// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Owned, spanned YAML tree built from the saphyr parser event stream.
//!
//! Built at the event level rather than through serde or a stock YAML tree so
//! that every node keeps its source span, duplicate mapping keys survive until
//! they can be diagnosed, and scalar styles remain observable.

use std::collections::HashMap;

use saphyr_parser::{Event, Parser, ScalarStyle, Span};

use crate::diag::Sink;

/// A YAML node with its source span.
#[derive(Debug, Clone)]
pub(crate) struct Node {
    pub(crate) span: Span,
    pub(crate) kind: Kind,
}

#[derive(Debug, Clone)]
pub(crate) enum Kind {
    Scalar { text: String, style: ScalarStyle },
    Seq(Vec<Node>),
    Map(Vec<(Node, Node)>),
}

impl Node {
    /// The scalar text and style, if this node is a scalar.
    pub(crate) fn scalar(&self) -> Option<(&str, ScalarStyle)> {
        match &self.kind {
            Kind::Scalar { text, style } => Some((text, *style)),
            _ => None,
        }
    }

    /// True if this node is a plain scalar that resolves to null.
    pub(crate) fn is_null(&self) -> bool {
        matches!(
            self.scalar(),
            Some((text, ScalarStyle::Plain)) if matches!(resolve_plain(text), Resolved::Null)
        )
    }
}

/// What a plain scalar resolves to under the YAML 1.2 Core schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Resolved {
    Null,
    Bool(bool),
    Int,
    Float,
    Str,
}

/// Resolve a plain scalar's text under the YAML 1.2 Core schema.
pub(crate) fn resolve_plain(text: &str) -> Resolved {
    match text {
        "" | "~" | "null" | "Null" | "NULL" => Resolved::Null,
        "true" | "True" | "TRUE" => Resolved::Bool(true),
        "false" | "False" | "FALSE" => Resolved::Bool(false),
        _ if is_core_int(text) => Resolved::Int,
        _ if is_core_float(text) => Resolved::Float,
        _ => Resolved::Str,
    }
}

/// What a plain scalar in a name position would wrongly resolve to, phrased
/// for a diagnostic. `None` when the scalar is a string (or not plain).
///
/// This is the single quoting rule for name positions, shared by mapping keys
/// and name-valued scalars.
pub(crate) fn non_string_kind(text: &str, style: ScalarStyle) -> Option<&'static str> {
    if style != ScalarStyle::Plain {
        return None;
    }
    match resolve_plain(text) {
        Resolved::Str => None,
        Resolved::Null => Some("null"),
        Resolved::Bool(_) => Some("a boolean"),
        Resolved::Int => Some("an integer"),
        Resolved::Float => Some("a float"),
    }
}

fn is_core_int(s: &str) -> bool {
    if let Some(hex) = s.strip_prefix("0x") {
        return !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit());
    }
    if let Some(oct) = s.strip_prefix("0o") {
        return !oct.is_empty() && oct.bytes().all(|b| (b'0'..=b'7').contains(&b));
    }
    let digits = s.strip_prefix(['-', '+']).unwrap_or(s);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

fn is_core_float(s: &str) -> bool {
    if matches!(s, ".nan" | ".NaN" | ".NAN") {
        return true;
    }
    let unsigned = s.strip_prefix(['-', '+']).unwrap_or(s);
    if matches!(unsigned, ".inf" | ".Inf" | ".INF") {
        return true;
    }
    // [0-9]+ (\. [0-9]*)? | \. [0-9]+ — followed by an optional exponent.
    let mantissa = match unsigned.split_once(['e', 'E']) {
        Some((m, exp)) => {
            let exp_digits = exp.strip_prefix(['-', '+']).unwrap_or(exp);
            if exp_digits.is_empty() || !exp_digits.bytes().all(|b| b.is_ascii_digit()) {
                return false;
            }
            m
        }
        None => unsigned,
    };
    let (int_part, frac_part) = match mantissa.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (mantissa, None),
    };
    if !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    match frac_part {
        // "." alone, or a fraction with non-digits, is not a float.
        Some(f) => (!int_part.is_empty() || !f.is_empty()) && f.bytes().all(|b| b.is_ascii_digit()),
        None => !int_part.is_empty(),
    }
}

enum Frame {
    Map {
        span: Span,
        aid: usize,
        pairs: Vec<(Node, Node)>,
        key: Option<Node>,
    },
    Seq {
        span: Span,
        aid: usize,
        items: Vec<Node>,
    },
}

/// Parse `src` into a single-document tree.
///
/// Structural problems (scan errors, tags, merge keys, aliases to unknown
/// anchors, multiple documents) abort with one located diagnostic; everything
/// past this point reports multiple diagnostics per run.
pub(crate) fn parse(src: &str, sink: &mut Sink) -> Option<Node> {
    if let Some(line) = leading_directive(src) {
        sink.error(
            span_at(line, 1),
            "",
            "YAML directives are not supported; model files are read as YAML 1.2",
            None,
        );
        return None;
    }

    let mut stack: Vec<Frame> = Vec::new();
    let mut anchors: HashMap<usize, Node> = HashMap::new();
    let mut root: Option<Node> = None;
    let mut document_seen = false;
    // Total nodes materialized through aliases, capping pathological
    // expansion (billion laughs) of otherwise tiny inputs.
    let mut alias_nodes: usize = 0;
    const MAX_ALIAS_NODES: usize = 65_536;

    for item in Parser::new_from_str(src) {
        let (event, span) = match item {
            Ok(ok) => ok,
            Err(e) => {
                let at = Span {
                    start: *e.marker(),
                    end: *e.marker(),
                };
                sink.error(at, "", format!("YAML syntax error: {}", e.info()), None);
                return None;
            }
        };
        if let Event::Scalar(_, _, _, Some(_))
        | Event::SequenceStart(_, Some(_))
        | Event::MappingStart(_, Some(_)) = &event
        {
            sink.error(span, "", "YAML tags are not supported in model files", None);
            return None;
        }
        match event {
            Event::StreamStart | Event::StreamEnd | Event::DocumentEnd | Event::Nothing => {}
            Event::DocumentStart(_) => {
                if document_seen {
                    sink.error(
                        span,
                        "",
                        "multiple YAML documents are not supported",
                        Some("keep the model in a single document".to_string()),
                    );
                    return None;
                }
                document_seen = true;
            }
            Event::Scalar(text, style, aid, _) => {
                let node = Node {
                    span,
                    kind: Kind::Scalar {
                        text: text.into_owned(),
                        style,
                    },
                };
                if !attach(&mut stack, &mut root, &mut anchors, aid, node, sink) {
                    return None;
                }
            }
            Event::Alias(aid) => match anchors.get(&aid) {
                Some(node) => {
                    alias_nodes += node_count(node);
                    if alias_nodes > MAX_ALIAS_NODES {
                        sink.error(
                            span,
                            "",
                            format!("alias expansion exceeds {MAX_ALIAS_NODES} nodes"),
                            None,
                        );
                        return None;
                    }
                    // Keep the anchor's spans: diagnostics inside aliased
                    // content should point at where the content is written.
                    let node = node.clone();
                    if !attach(&mut stack, &mut root, &mut anchors, 0, node, sink) {
                        return None;
                    }
                }
                None => {
                    sink.error(
                        span,
                        "",
                        "alias refers to an unknown or still-open anchor",
                        None,
                    );
                    return None;
                }
            },
            Event::SequenceStart(aid, _) => {
                stack.push(Frame::Seq {
                    span,
                    aid,
                    items: Vec::new(),
                });
            }
            Event::SequenceEnd => {
                let Some(Frame::Seq { span, aid, items }) = stack.pop() else {
                    unreachable!("parser balances sequence events");
                };
                let node = Node {
                    span,
                    kind: Kind::Seq(items),
                };
                if !attach(&mut stack, &mut root, &mut anchors, aid, node, sink) {
                    return None;
                }
            }
            Event::MappingStart(aid, _) => {
                stack.push(Frame::Map {
                    span,
                    aid,
                    pairs: Vec::new(),
                    key: None,
                });
            }
            Event::MappingEnd => {
                let Some(Frame::Map {
                    span, aid, pairs, ..
                }) = stack.pop()
                else {
                    unreachable!("parser balances mapping events");
                };
                let node = Node {
                    span,
                    kind: Kind::Map(pairs),
                };
                if !attach(&mut stack, &mut root, &mut anchors, aid, node, sink) {
                    return None;
                }
            }
        }
    }

    if root.is_none() {
        sink.error(
            span_at(1, 1),
            "",
            "empty document; expected a mapping at the root",
            None,
        );
    }
    root
}

/// A zero-width span at a 1-based line and column, for errors that have no
/// parsed node.
fn span_at(line: usize, column: usize) -> Span {
    // Sink renders 0-based marker columns as 1-based.
    let marker = saphyr_parser::Marker::new(0, line, column.saturating_sub(1));
    Span {
        start: marker,
        end: marker,
    }
}

/// Attach a completed node as the next key, value, or item.
///
/// This is the one point where a node becomes a mapping key, so merge keys
/// (`<<`) are rejected here. Returns false after reporting one.
fn attach(
    stack: &mut [Frame],
    root: &mut Option<Node>,
    anchors: &mut HashMap<usize, Node>,
    aid: usize,
    node: Node,
    sink: &mut Sink,
) -> bool {
    if aid != 0 {
        anchors.insert(aid, node.clone());
    }
    match stack.last_mut() {
        None => *root = Some(node),
        Some(Frame::Map { pairs, key, .. }) => match key.take() {
            None => {
                if matches!(node.scalar(), Some(("<<", ScalarStyle::Plain))) {
                    sink.error(node.span, "", "merge keys (`<<`) are not supported", None);
                    return false;
                }
                *key = Some(node);
            }
            Some(k) => pairs.push((k, node)),
        },
        Some(Frame::Seq { items, .. }) => items.push(node),
    }
    true
}

/// The 1-based line of a leading `%` directive, if the document has one.
///
/// Directives are only legal before the content starts, so scanning stops at
/// the first line that is not blank, a comment, or a directive.
fn leading_directive(src: &str) -> Option<usize> {
    for (index, line) in src.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if line.starts_with('%') {
            return Some(index + 1);
        }
        return None;
    }
    None
}

/// The number of nodes in a subtree, matching the cost of cloning it.
fn node_count(node: &Node) -> usize {
    1 + match &node.kind {
        Kind::Scalar { .. } => 0,
        Kind::Seq(items) => items.iter().map(node_count).sum(),
        Kind::Map(pairs) => pairs
            .iter()
            .map(|(k, v)| node_count(k) + node_count(v))
            .sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_core_scalars() {
        for s in ["", "~", "null", "NULL"] {
            assert_eq!(resolve_plain(s), Resolved::Null, "{s:?}");
        }
        assert_eq!(resolve_plain("true"), Resolved::Bool(true));
        assert_eq!(resolve_plain("False"), Resolved::Bool(false));
        for s in ["0", "42", "-7", "+7", "0x1F", "0o755"] {
            assert_eq!(resolve_plain(s), Resolved::Int, "{s:?}");
        }
        for s in [
            "1.5", "-0.1", ".5", "2.", "1e5", "1.2e-3", ".inf", "-.Inf", ".nan",
        ] {
            assert_eq!(resolve_plain(s), Resolved::Float, "{s:?}");
        }
        // YAML 1.1-isms and near-misses stay strings under 1.2 Core.
        for s in [
            "on", "off", "yes", "no", "y", "n", "08x", "0o8", "1.2.3", ".", "e5", "1e", "opened",
        ] {
            assert_eq!(resolve_plain(s), Resolved::Str, "{s:?}");
        }
    }

    fn parse_ok(src: &str) -> Node {
        let mut sink = Sink::new("<test>");
        let node = parse(src, &mut sink);
        assert!(!sink.has_errors(), "{}", sink.into_diagnostics());
        node.expect("root")
    }

    fn parse_err(src: &str) -> String {
        let mut sink = Sink::new("<test>");
        let node = parse(src, &mut sink);
        assert!(node.is_none() || sink.has_errors());
        sink.into_diagnostics().to_string()
    }

    #[test]
    fn keeps_duplicate_keys_and_order() {
        let root = parse_ok("a: 1\nb: 2\na: 3\n");
        let Kind::Map(pairs) = &root.kind else {
            panic!("expected map");
        };
        let keys: Vec<_> = pairs.iter().map(|(k, _)| k.scalar().unwrap().0).collect();
        assert_eq!(keys, ["a", "b", "a"]);
    }

    #[test]
    fn resolves_aliases_by_clone() {
        let root = parse_ok("x: &a {v: 1}\ny: *a\n");
        let Kind::Map(pairs) = &root.kind else {
            panic!("expected map");
        };
        assert!(matches!(pairs[1].1.kind, Kind::Map(_)));
    }

    #[test]
    fn rejects_structural_problems() {
        assert!(parse_err("a: !!str x\n").contains("tags are not supported"));
        assert!(parse_err("---\na: 1\n---\nb: 2\n").contains("multiple YAML documents"));
        assert!(parse_err("base: &b {x: 1}\nd:\n  <<: *b\n").contains("merge keys"));
        // Undefined aliases are a scan error in saphyr itself; the in-tree
        // alias branch remains as a backstop.
        assert!(parse_err("a: *nope\n").contains("unknown anchor"));
        assert!(parse_err("").contains("empty document"));
        assert!(parse_err("a: [1\n").contains("YAML syntax error"));
    }
}
