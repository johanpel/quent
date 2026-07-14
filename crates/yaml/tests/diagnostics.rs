// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Diagnostics tests: every rejection carries a useful message.

use quent_yaml::{Error, load_str};

/// Load `src`, expect failure, and assert one diagnostic contains all
/// `needles` (checked against message and help).
#[track_caller]
fn expect_error(src: &str, needles: &[&str]) {
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

const HEADER: &str = "quent: 1\nmodel: m\n";

#[test]
fn bad_format_version() {
    expect_error("quent: 2\nmodel: m\n", &["unsupported format version `2`"]);
}

#[test]
fn unknown_keys_rejected() {
    // On direct structs, serde's deny_unknown_fields names the bad key.
    expect_error(&format!("{HEADER}fsm: {{}}\n"), &["unknown field `fsm`"]);
    expect_error(
        &format!("{HEADER}records:\n  R:\n    bogus: 1\n"),
        &["unknown field `bogus`"],
    );
    // Inside a field mapping (an untagged enum), serde only reports that no
    // variant matched — a known limitation, still an error.
    expect_error(
        &format!(
            "{HEADER}records:\n  R:\n    fields:\n      f:\n        type: u8\n        docs: x\n"
        ),
        &["did not match any variant"],
    );
}

#[test]
fn event_cardinality_required() {
    expect_error(
        &format!("{HEADER}entities:\n  E:\n    events:\n      started:\n        doc: x\n"),
        &["event must declare a cardinality"],
    );
    expect_error(
        &format!(
            "{HEADER}entities:\n  E:\n    events:\n      started:\n        once: {{}}\n        multi: {{}}\n"
        ),
        &["both `once` and `multi`"],
    );
}

#[test]
fn malformed_types() {
    expect_error(
        &format!("{HEADER}records:\n  R:\n    fields:\n      f: Vec<u8\n"),
        &["invalid type", "missing `>`"],
    );
    expect_error(
        &format!("{HEADER}records:\n  R:\n    fields:\n      f: Ref<Engine>\n"),
        &["`Ref` takes no type parameter"],
    );
}

#[test]
fn invalid_and_reserved_names() {
    expect_error(
        &format!("{HEADER}records:\n  'has space':\n"),
        &["invalid name `has space`"],
    );
    expect_error(
        &format!("{HEADER}records:\n  String:\n    fields: {{ x: u8 }}\n"),
        &["`String` is a reserved type name"],
    );
}

#[test]
fn ref_value_reserved() {
    expect_error(
        &format!(
            "{HEADER}entities:\n  E:\n    events:\n      up:\n        once:\n          f:\n            type:\n              ref: E\n"
        ),
        &["`ref` takes no value"],
    );
}

#[test]
fn unknown_record_reference() {
    expect_error(
        &format!("{HEADER}records:\n  R:\n    fields:\n      f: Ghost\n"),
        &["unresolved reference"],
    );
}

#[test]
fn recursive_record() {
    expect_error(
        &format!("{HEADER}records:\n  Node:\n    fields:\n      next: Node?\n"),
        &["record `Node` is recursive"],
    );
}

#[test]
fn generated_type_collision() {
    expect_error(
        &format!(
            "{HEADER}records:\n  EngineEvent:\n    fields: {{ x: u8 }}\nentities:\n  Engine:\n    events: {{ started: once }}\n"
        ),
        &["both generate the type `EngineEvent`"],
    );
}

#[test]
fn case_collision() {
    expect_error(
        &format!(
            "{HEADER}entities:\n  E:\n    events:\n      startUp: once\n      start_up: once\n"
        ),
        &["generate the identifier `StartUp`"],
    );
}

#[test]
fn empty_annotation_name() {
    expect_error(
        &format!("{HEADER}constraints:\n  '': x\n"),
        &["constraint name must not be empty"],
    );
}

#[test]
fn syntax_error_has_a_location() {
    let Err(Error::Invalid(diagnostics)) = load_str("quent: 1\nmodel: [\n") else {
        panic!("expected failure");
    };
    assert!(
        diagnostics.iter().any(|d| d.location.is_some()),
        "parse errors should carry a location: {diagnostics}"
    );
}
