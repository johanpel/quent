// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! FSM tests: an `fsms:` overlay declares an entity's events as states, deriving
//! their cardinality from the topology, and is validated against the entity.

use quent_schema::test_utils::ident;
use quent_schema::{Cardinality, Schema};
use quent_yaml::{Error, parse_from_str};

const FSM: &str = "quent.fsm.v0.1.0";

fn schema_of(src: &str) -> Schema {
    parse_from_str(src, None).expect("parses").schema
}

fn errors_of(src: &str) -> String {
    match parse_from_str(src, None) {
        Err(Error::Invalid(diagnostics)) => diagnostics.to_string(),
        other => panic!("expected diagnostics, got {other:?}"),
    }
}

const QUERY: &str = "\
quent: alpha
model: m
entities:
  Query:
    doc: A query.
fsms:
  Query:
    states:
      submitted:
        initial: true
        attributes: { text: string }
        to: [progress]
      progress:
        attributes: { pct: u8 }
        to: [progress, finished]
      finished:
        exit: true
        attributes: { ok: bool }
";

#[test]
fn fsm_builds_events_and_derives_cardinality() {
    let schema = schema_of(QUERY);
    let query = schema.entity(&ident("Query")).unwrap();
    assert!(query.annotations().has_constraint(FSM));
    assert_eq!(query.events().count(), 3);
    // `progress` self-loops, so it is Multi; the others are Once.
    let card = |e: &str| query.event(&ident(e)).unwrap().cardinality();
    assert!(matches!(card("progress"), Cardinality::Multi));
    assert!(matches!(card("submitted"), Cardinality::Once));
    assert!(matches!(card("finished"), Cardinality::Once));
}

#[test]
fn fsms_referencing_unknown_entity_is_rejected() {
    let errors = errors_of(
        "\
quent: alpha
model: m
fsms:
  Ghost:
    states:
      a: { initial: true, exit: true }
",
    );
    assert!(
        errors.contains("no such entity") && errors.contains("Ghost"),
        "{errors}"
    );
}

#[test]
fn fsm_entity_may_not_declare_events() {
    let errors = errors_of(
        "\
quent: alpha
model: m
entities:
  E:
    events:
      a: once
fsms:
  E:
    states:
      a: { initial: true, exit: true }
",
    );
    assert!(
        errors.contains("declares its events as FSM states"),
        "{errors}"
    );
}

#[test]
fn fsm_needs_one_initial_state() {
    let errors = errors_of(
        "\
quent: alpha
model: m
entities:
  E:
    doc: x
fsms:
  E:
    states:
      a: { exit: true }
      b: { exit: true }
",
    );
    assert!(
        errors.contains("no state marked `initial: true`"),
        "{errors}"
    );
}

#[test]
fn fsm_needs_an_exit_state() {
    let errors = errors_of(
        "\
quent: alpha
model: m
entities:
  E:
    doc: x
fsms:
  E:
    states:
      a: { initial: true, to: [a] }
",
    );
    assert!(errors.contains("no state marked `exit: true`"), "{errors}");
}

#[test]
fn unreachable_state_is_rejected() {
    // `b` is a state but nothing reaches it from the initial state.
    let errors = errors_of(
        "\
quent: alpha
model: m
entities:
  E:
    doc: x
fsms:
  E:
    states:
      a: { initial: true, exit: true }
      b: { exit: true, to: [a] }
",
    );
    assert!(errors.contains("unreachable"), "{errors}");
}
