// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Constraint and metadata payload conversion.
//!
//! The payload rule: a null node carries no data, a string scalar is passed
//! through verbatim, and any other node is converted to canonical (compact,
//! key-sorted) JSON. Plain scalars resolve under the YAML 1.2 Core schema;
//! quoted and block scalars always stay strings. Mapping keys are an
//! exception: they become JSON object keys by their text as written (so
//! `404:` works, and `0x10:` stays `"0x10"`); only values are canonicalized.

use serde_json::Value;

use crate::diag::Sink;
use crate::tree::{Kind, Node, Resolved, resolve_plain};

/// Convert one annotation payload node to `Constraint`/`Metadata` data.
///
/// Returns `None` if a diagnostic was reported, `Some(None)` for a null
/// payload, and `Some(Some(data))` otherwise.
pub(crate) fn payload(node: &Node, sink: &mut Sink, path: &str) -> Option<Option<String>> {
    if node.is_null() {
        return Some(None);
    }
    if let Some((text, style)) = node.scalar()
        && (style != saphyr_parser::ScalarStyle::Plain || resolve_plain(text) == Resolved::Str)
    {
        return Some(Some(text.to_string()));
    }
    let value = to_json(node, sink, path)?;
    Some(Some(
        serde_json::to_string(&value).expect("JSON values serialize"),
    ))
}

fn to_json(node: &Node, sink: &mut Sink, path: &str) -> Option<Value> {
    match &node.kind {
        Kind::Scalar { text, style } => {
            if *style != saphyr_parser::ScalarStyle::Plain {
                return Some(Value::String(text.clone()));
            }
            match resolve_plain(text) {
                Resolved::Null => Some(Value::Null),
                Resolved::Bool(b) => Some(Value::Bool(b)),
                Resolved::Int => int_to_json(text, node, sink, path),
                Resolved::Float => float_to_json(text, node, sink, path),
                Resolved::Str => Some(Value::String(text.clone())),
            }
        }
        Kind::Seq(items) => {
            let values: Vec<Value> = items
                .iter()
                .map(|item| to_json(item, sink, path))
                .collect::<Option<_>>()?;
            Some(Value::Array(values))
        }
        Kind::Map(pairs) => {
            let mut object = serde_json::Map::new();
            let mut ok = true;
            for (key, value) in pairs {
                // Keys become JSON object keys by their string value
                // regardless of what they would resolve to, so `404:` works.
                let Some((text, _)) = key.scalar() else {
                    sink.error(key.span, path, "payload mapping keys must be scalars", None);
                    ok = false;
                    continue;
                };
                if object.contains_key(text) {
                    sink.error(
                        key.span,
                        path,
                        format!("duplicate payload key `{text}`"),
                        None,
                    );
                    ok = false;
                    continue;
                }
                match to_json(value, sink, path) {
                    Some(v) => {
                        object.insert(text.to_string(), v);
                    }
                    None => ok = false,
                }
            }
            ok.then_some(Value::Object(object))
        }
    }
}

fn int_to_json(text: &str, node: &Node, sink: &mut Sink, path: &str) -> Option<Value> {
    let parsed = if let Some(hex) = text.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok().map(Into::into)
    } else if let Some(oct) = text.strip_prefix("0o") {
        u64::from_str_radix(oct, 8).ok().map(Into::into)
    } else {
        text.parse::<i64>()
            .ok()
            .map(Into::into)
            .or_else(|| text.parse::<u64>().ok().map(Into::into))
    };
    match parsed {
        Some(number) => Some(Value::Number(number)),
        None => {
            sink.error(
                node.span,
                path,
                format!("integer `{text}` does not fit JSON numbers"),
                Some("quote it to pass it through as a string".to_string()),
            );
            None
        }
    }
}

fn float_to_json(text: &str, node: &Node, sink: &mut Sink, path: &str) -> Option<Value> {
    let float = normalize_float(text).parse::<f64>().ok();
    match float.and_then(serde_json::Number::from_f64) {
        Some(number) => Some(Value::Number(number)),
        None => {
            sink.error(
                node.span,
                path,
                format!("float `{text}` has no JSON representation"),
                Some("quote it to pass it through as a string".to_string()),
            );
            None
        }
    }
}

/// Map YAML's `.inf`/`.nan` spellings onto what `f64::from_str` accepts, so
/// they fail JSON conversion as non-finite numbers rather than parse errors.
fn normalize_float(text: &str) -> std::borrow::Cow<'_, str> {
    let unsigned = text.trim_start_matches(['-', '+']);
    match unsigned {
        ".inf" | ".Inf" | ".INF" => format!("{}inf", &text[..text.len() - unsigned.len()]).into(),
        ".nan" | ".NaN" | ".NAN" => "NaN".into(),
        _ => text.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree;

    fn payload_of(src: &str) -> Result<Option<String>, String> {
        let mut sink = Sink::new("<test>");
        let root = tree::parse(src, &mut sink).expect("parses");
        let Kind::Map(pairs) = &root.kind else {
            panic!("expected a map root");
        };
        match payload(&pairs[0].1, &mut sink, "t") {
            Some(data) => Ok(data),
            None => Err(sink.into_diagnostics().to_string()),
        }
    }

    #[test]
    fn null_is_no_data() {
        assert_eq!(payload_of("k:\n").unwrap(), None);
        assert_eq!(payload_of("k: ~\n").unwrap(), None);
    }

    #[test]
    fn strings_pass_verbatim() {
        assert_eq!(payload_of("k: redact\n").unwrap().unwrap(), "redact");
        assert_eq!(payload_of("k: 'true'\n").unwrap().unwrap(), "true");
        assert_eq!(payload_of("k: \"42\"\n").unwrap().unwrap(), "42");
    }

    #[test]
    fn structures_become_canonical_json() {
        assert_eq!(payload_of("k: 42\n").unwrap().unwrap(), "42");
        assert_eq!(payload_of("k: true\n").unwrap().unwrap(), "true");
        assert_eq!(payload_of("k: 0x1F\n").unwrap().unwrap(), "31");
        assert_eq!(
            payload_of("k: { b: 1, a: [x, 2.5, null] }\n")
                .unwrap()
                .unwrap(),
            r#"{"a":["x",2.5,null],"b":1}"#
        );
        assert_eq!(
            payload_of("k: { 404: retry }\n").unwrap().unwrap(),
            r#"{"404":"retry"}"#
        );
    }

    #[test]
    fn rejects_unrepresentable_numbers() {
        assert!(
            payload_of("k: { v: .inf }\n")
                .unwrap_err()
                .contains("no JSON representation")
        );
        assert!(
            payload_of("k: { v: .nan }\n")
                .unwrap_err()
                .contains("no JSON representation")
        );
        assert!(
            payload_of("k: { v: 99999999999999999999 }\n")
                .unwrap_err()
                .contains("does not fit")
        );
        assert!(payload_of("k: { v: 'quoted .inf is fine' }\n").is_ok());
    }

    #[test]
    fn rejects_duplicate_payload_keys() {
        assert!(
            payload_of("k: { a: 1, a: 2 }\n")
                .unwrap_err()
                .contains("duplicate payload key")
        );
    }
}
