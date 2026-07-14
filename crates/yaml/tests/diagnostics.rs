// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Diagnostics tests: every rejection carries a useful message.

use quent_yaml::{Error, load_str};

const HEADER: &str = "\
quent: 1
model: m
";

/// Load `src`, expect failure, and assert one diagnostic contains all
/// `needles` (checked against message and help).
#[track_caller]
fn expect_raw(src: &str, needles: &[&str]) {
    let diagnostics = match load_str(src) {
        Err(Error::Invalid(d)) => d,
        Err(e) => panic!("expected diagnostics, got {e:?}"),
        Ok(_) => panic!("expected failure, loaded fine:\n{src}"),
    };
    let matched = diagnostics.iter().any(|d| {
        needles.iter().all(|needle| {
            d.message.contains(needle) || d.help.as_deref().is_some_and(|h| h.contains(needle))
        })
    });
    assert!(
        matched,
        "no diagnostic containing {needles:?}; got:\n{diagnostics}"
    );
}

/// Like [`expect_raw`], prefixing the standard `quent: 1` / `model: m` header
/// so a test spells out only the body under scrutiny.
#[track_caller]
fn expect_error(body: &str, needles: &[&str]) {
    expect_raw(&format!("{HEADER}{body}"), needles);
}

#[test]
fn bad_format_version() {
    expect_raw(
        "\
quent: 2
model: m
",
        &["unsupported format version `2`"],
    );
}

#[test]
fn unknown_keys_rejected() {
    // On direct structs, serde's deny_unknown_fields names the bad key.
    expect_error("fsm: {}\n", &["unknown field `fsm`"]);
    expect_error(
        "\
records:
  R:
    bogus: 1
",
        &["unknown field `bogus`"],
    );
    // Inside a field mapping (an untagged enum), serde only reports that no
    // variant matched — a known limitation, still an error.
    expect_error(
        "\
records:
  R:
    fields:
      f:
        type: u8
        docs: x
",
        &["did not match any variant"],
    );
}

#[test]
fn event_cardinality_required() {
    expect_error(
        "\
entities:
  E:
    events:
      started:
        doc: x
",
        &["event must declare a cardinality"],
    );
    expect_error(
        "\
entities:
  E:
    events:
      started:
        once: {}
        multi: {}
",
        &["both `once` and `multi`"],
    );
}

#[test]
fn malformed_types() {
    // A compact Rust-style spelling is just an unusable bare type name.
    expect_error(
        "\
records:
  R:
    fields:
      f: Vec<u8>
",
        &["invalid type `Vec<u8>`"],
    );
    // An unrecognized type-wrapper key matches no `TypeExpr` variant.
    expect_error(
        "\
records:
  R:
    fields:
      f: { lst: u8 }
",
        &["did not match any variant"],
    );
}

#[test]
fn invalid_and_reserved_names() {
    expect_error(
        "\
records:
  'has space':
",
        &["invalid name `has space`"],
    );
    expect_error(
        "\
records:
  string:
    fields: { x: u8 }
",
        &["`string` is a reserved type name"],
    );
}

#[test]
fn ref_value_reserved() {
    expect_error(
        "\
entities:
  E:
    events:
      up:
        once:
          f:
            type:
              ref: E
",
        &["`ref` takes no value"],
    );
}

#[test]
fn unknown_record_reference() {
    expect_error(
        "\
records:
  R:
    fields:
      f: Ghost
",
        &["unresolved reference"],
    );
}

#[test]
fn recursive_record() {
    expect_error(
        "\
records:
  Node:
    fields:
      next: { option: Node }
",
        &["record `Node` is recursive"],
    );
}

#[test]
fn invalid_sibling_names_do_not_panic() {
    // Two records with invalid names must both surface as diagnostics rather
    // than reaching the builder as a shared placeholder and panicking.
    expect_error(
        "\
records:
  'a b':
  'c d':
",
        &["invalid name `a b`"],
    );
}

#[test]
fn empty_annotation_name() {
    expect_error(
        "\
constraints:
  '': x
",
        &["constraint name must not be empty"],
    );
}

#[test]
fn syntax_error_has_a_location() {
    let Err(Error::Invalid(diagnostics)) = load_str(
        "\
quent: 1
model: [
",
    ) else {
        panic!("expected failure");
    };
    assert!(
        diagnostics.iter().any(|d| d.location.is_some()),
        "parse errors should carry a location: {diagnostics}"
    );
}
