// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Diagnostics tests: every rejection carries a useful location and message.

use quent_yaml::{Error, load_str};

/// Load `src`, expect failure, and assert one diagnostic matches `line` and
/// all `needles` (checked against message and help).
#[track_caller]
fn expect_error(src: &str, line: usize, needles: &[&str]) {
    let diagnostics = match load_str(src) {
        Err(Error::Invalid(d)) => d,
        Err(e) => panic!("expected diagnostics, got {e:?}"),
        Ok(_) => panic!("expected failure, loaded fine:\n{src}"),
    };
    let matched = diagnostics.iter().any(|d| {
        d.line == line
            && needles.iter().all(|needle| {
                d.message.contains(needle) || d.help.as_deref().is_some_and(|h| h.contains(needle))
            })
    });
    assert!(
        matched,
        "no diagnostic at line {line} containing {needles:?}; got:\n{diagnostics}"
    );
}

// Two lines: test sources below start at line 3.
const HEADER: &str = "quent: 1\nmodel: m\n";

#[test]
fn missing_or_bad_format_version() {
    expect_error(
        "model: m\n",
        1,
        &["missing the `quent` format version key", "quent: 1"],
    );
    expect_error(
        "quent: 2\nmodel: m\n",
        1,
        &["unsupported format version `2`"],
    );
    expect_error(
        "quent: yes\nmodel: m\n",
        1,
        &["unsupported format version `yes`"],
    );
}

#[test]
fn missing_model() {
    expect_error("quent: 1\n", 1, &["missing the `model` key"]);
}

#[test]
fn unknown_keys_suggest() {
    expect_error(
        &format!("{HEADER}recrods: {{}}\n"),
        3,
        &["unknown key `recrods`", "did you mean `records`?"],
    );
    expect_error(
        &format!("{HEADER}fsm: {{}}\n"),
        3,
        &["unknown key `fsm`", "newer quent-yaml"],
    );
    expect_error(
        &format!(
            "{HEADER}records:\n  R:\n    fields:\n      f:\n        type: u8\n        docs: x\n"
        ),
        8,
        &["unknown key `docs`", "did you mean `doc`?"],
    );
}

#[test]
fn duplicate_keys_cite_both_locations() {
    expect_error(
        &format!(
            "{HEADER}entities:\n  E:\n    events:\n      started: once\n      started: multi\n"
        ),
        7,
        &["duplicate key `started`", "line 6"],
    );
    // Styles are equivalent: quoting does not make a different key.
    expect_error(
        &format!("{HEADER}entities:\n  E:\n    events:\n      up: once\n      'up': multi\n"),
        7,
        &["duplicate key `up`", "line 6"],
    );
}

#[test]
fn event_cardinality_is_mandatory() {
    expect_error(
        &format!("{HEADER}entities:\n  E:\n    events:\n      started:\n"),
        6,
        &["event must declare a cardinality", "once"],
    );
    expect_error(
        &format!("{HEADER}entities:\n  E:\n    events:\n      started: {{ doc: x }}\n"),
        6,
        &["event must declare a cardinality"],
    );
    expect_error(
        &format!(
            "{HEADER}entities:\n  E:\n    events:\n      started:\n        once:\n        multi:\n"
        ),
        8,
        &["both `once` and `multi`"],
    );
    expect_error(
        &format!("{HEADER}entities:\n  E:\n    events:\n      started: sometimes\n"),
        6,
        &["expected `once` or `multi`, found `sometimes`"],
    );
}

#[test]
fn unknown_types_suggest() {
    expect_error(
        &format!("{HEADER}records:\n  R:\n    fields:\n      f: Strin\n"),
        6,
        &["unknown type or record `Strin`", "did you mean `String`?"],
    );
    expect_error(
        &format!(
            "{HEADER}records:\n  R:\n    fields:\n      f: Endpoit\n  Endpoint:\n    fields:\n      h: String\n"
        ),
        6,
        &["did you mean `Endpoint`?"],
    );
}

#[test]
fn entity_in_record_position_gets_targeted_help() {
    expect_error(
        &format!(
            "{HEADER}records:\n  R:\n    fields:\n      f: Engine\nentities:\n  Engine:\n    events: {{ up: once }}\n"
        ),
        6,
        &[
            "`Engine` is an entity",
            "write `Ref` to reference an entity",
        ],
    );
}

#[test]
fn ref_takes_no_type_parameter() {
    expect_error(
        &format!(
            "{HEADER}entities:\n  Engine:\n    events:\n      up:\n        once:\n          e: Ref<Engine>\n"
        ),
        8,
        &["`Ref` takes no type parameter"],
    );
}

#[test]
fn malformed_type_expressions() {
    expect_error(
        &format!("{HEADER}records:\n  R:\n    fields:\n      f: Vec<u8\n"),
        6,
        &["invalid type expression `Vec<u8`", "missing `>`"],
    );
    expect_error(
        &format!("{HEADER}records:\n  R:\n    fields:\n      f: '&Engine'\n"),
        6,
        &["unexpected character `&`"],
    );
}

#[test]
fn yaml_12_typed_scalars_in_name_positions() {
    expect_error(
        &format!("{HEADER}entities:\n  E:\n    events:\n      true: once\n"),
        6,
        &["`true` reads as a boolean", "quote it"],
    );
    expect_error(
        &format!("{HEADER}records:\n  R:\n    fields:\n      404: u8\n"),
        6,
        &["`404` reads as an integer", "quote it"],
    );
}

#[test]
fn invalid_identifiers() {
    expect_error(
        &format!("{HEADER}records:\n  'has space':\n"),
        4,
        &["invalid name `has space`"],
    );
    expect_error(
        &format!("{HEADER}records:\n  '_lead':\n"),
        4,
        &["invalid name `_lead`"],
    );
}

#[test]
fn reserved_type_names_are_rejected() {
    expect_error(
        &format!("{HEADER}records:\n  String:\n    fields: {{ x: u8 }}\n"),
        4,
        &["`String` is a reserved type name"],
    );
}

#[test]
fn case_collisions_with_generated_code() {
    expect_error(
        &format!(
            "{HEADER}entities:\n  E:\n    events:\n      startUp: once\n      start_up: once\n"
        ),
        7,
        &[
            "`start_up` and `startUp`",
            "generate the identifier `StartUp`",
        ],
    );
    expect_error(
        &format!(
            "{HEADER}records:\n  R:\n    fields:\n      someField: u8\n      some_field: u8\n"
        ),
        7,
        &["generate the identifier `some_field`"],
    );
}

#[test]
fn structural_yaml_problems() {
    expect_error(
        &format!("{HEADER}x: !!str t\n"),
        3,
        &["tags are not supported"],
    );
    expect_error(
        "---\nquent: 1\nmodel: m\n---\nquent: 1\nmodel: n\n",
        4,
        &["multiple YAML documents"],
    );
    expect_error(
        &format!("{HEADER}base: &b {{ x: 1 }}\nmetadata:\n  <<: *b\n"),
        5,
        &["merge keys"],
    );
    expect_error(
        &format!("{HEADER}metadata: *nope\n"),
        3,
        &["unknown anchor"],
    );
    expect_error("[1, 2]\n", 1, &["expected a mapping at the document root"]);
}

#[test]
fn payload_numbers_without_json_representation() {
    expect_error(
        &format!("{HEADER}metadata:\n  acme.v1: {{ threshold: .inf }}\n"),
        4,
        &["no JSON representation", "quote it"],
    );
    expect_error(
        &format!("{HEADER}metadata:\n  acme.v1: {{ big: 99999999999999999999999 }}\n"),
        4,
        &["does not fit JSON numbers"],
    );
}

#[test]
fn ref_value_is_reserved() {
    // The `ref:` key is the structured form's marker; its value is reserved
    // for later syntax extensions (reference targets).
    expect_error(
        &format!(
            "{HEADER}entities:\n  E:\n    events:\n      up:\n        once:\n          f:\n            type:\n              ref: E\n"
        ),
        10,
        &["`ref` takes no value", "leave it empty"],
    );
    expect_error(
        &format!(
            "{HEADER}entities:\n  E:\n    events:\n      up:\n        once:\n          f:\n            type:\n              data: u64\n"
        ),
        10,
        &["reference mapping needs a `ref` key"],
    );
}

#[test]
fn validation_stage_errors_get_spans() {
    // A recursive record is caught by base validation and mapped back to its
    // declaration.
    expect_error(
        &format!("{HEADER}records:\n  Node:\n    fields:\n      next: Node?\n"),
        4,
        &["record `Node` is recursive"],
    );
}

#[test]
fn nesting_depth_is_capped() {
    let ty = format!("{}u8{}", "Vec<".repeat(65), ">".repeat(65));
    expect_error(
        &format!("{HEADER}records:\n  R:\n    fields:\n      f: {ty}\n"),
        6,
        &["deeper than 64"],
    );
    // Far past the cap: the parser must bail, not overflow its stack.
    let ty = format!("{}u8{}", "Vec<".repeat(10_000), ">".repeat(10_000));
    expect_error(
        &format!("{HEADER}records:\n  R:\n    fields:\n      f: {ty}\n"),
        6,
        &["deeper than 64"],
    );
}

#[test]
fn empty_annotation_names_are_rejected() {
    expect_error(
        &format!("{HEADER}constraints:\n  '': x\n"),
        4,
        &["constraint name must not be empty"],
    );
    expect_error(
        &format!("{HEADER}metadata:\n  '': x\n"),
        4,
        &["metadata name must not be empty"],
    );
}

#[test]
fn cross_collection_codegen_collision() {
    expect_error(
        &format!(
            "{HEADER}records:\n  EngineEvent:\n    fields: {{ x: u8 }}\nentities:\n  Engine:\n    events: {{ started: once }}\n"
        ),
        7,
        &[
            "entity `Engine` and record `EngineEvent`",
            "both generate the type `EngineEvent`",
        ],
    );
}

#[test]
fn alias_expansion_is_capped() {
    let src = format!(
        "{HEADER}metadata:\n  x0: &a [y, y, y, y, y, y, y, y]\n  x1: &b [*a, *a, *a, *a, *a, *a, *a, *a]\n  x2: &c [*b, *b, *b, *b, *b, *b, *b, *b]\n  x3: &d [*c, *c, *c, *c, *c, *c, *c, *c]\n  x4: &e [*d, *d, *d, *d, *d, *d, *d, *d]\n  x5: [*e, *e, *e, *e, *e, *e, *e, *e]\n"
    );
    let Err(Error::Invalid(diagnostics)) = load_str(&src) else {
        panic!("expected failure");
    };
    assert!(
        diagnostics.to_string().contains("alias expansion exceeds"),
        "{diagnostics}"
    );
}

#[test]
fn yaml_directives_are_rejected() {
    expect_error(
        "%YAML 1.1\n---\nquent: 1\nmodel: m\n",
        1,
        &["directives are not supported"],
    );
}

#[test]
fn merge_key_via_alias_is_rejected() {
    // The `<<` scalar is anchored as a value, then aliased into key position;
    // the diagnostic points at where the scalar is written.
    expect_error(
        &format!("{HEADER}k: &m <<\nbase: &b {{ x: 1 }}\nmetadata:\n  *m : *b\n"),
        3,
        &["merge keys"],
    );
}

#[test]
fn multiple_errors_reported_in_one_run() {
    let src = format!(
        "{HEADER}records:\n  R:\n    fields:\n      a: Strin\n      b: Vec<\n      c: u8\nentities:\n  E:\n    events:\n      up: never\n"
    );
    let Err(Error::Invalid(diagnostics)) = load_str(&src) else {
        panic!("expected failure");
    };
    assert!(
        diagnostics.iter().count() >= 3,
        "expected several diagnostics:\n{diagnostics}"
    );
}

#[test]
fn diagnostics_render_with_file_line_column() {
    let Err(Error::Invalid(diagnostics)) = load_str("model: m\n") else {
        panic!("expected failure");
    };
    let rendered = diagnostics.to_string();
    assert!(rendered.contains("<input>:1:1:"), "{rendered}");
}
