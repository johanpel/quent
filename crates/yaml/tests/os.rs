// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! OS tests: canonical process and thread records are included when referenced.

use quent_os::{process_path, thread_path};
use quent_schema::test_utils::{ident, path};
use quent_schema::{DataType, Schema};
use quent_yaml::parse_from_str;

fn schema_of(src: &str) -> Schema {
    parse_from_str(src, None).expect("parses").schema
}

fn errors_of(src: &str) -> String {
    match parse_from_str(src, None) {
        Ok(_) => panic!("expected errors, but parsing succeeded"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn process_record_is_added_when_referenced() {
    let schema = schema_of(
        "\
quent: alpha
model: m
entities:
  MyProcess:
    events:
      init:
        attributes:
          process: quent::os::Process
",
    );

    let field = schema
        .entity(&path("MyProcess"))
        .unwrap()
        .event(&ident("init"))
        .unwrap()
        .field(&ident("process"))
        .unwrap();
    assert_eq!(field.ty(), &DataType::Record(process_path()));
    assert!(schema.record(&process_path()).is_some());
    assert!(schema.record(&thread_path()).is_none());
}

#[test]
fn thread_record_is_added_when_scoped_under_process() {
    let schema = schema_of(
        "\
quent: alpha
model: m
entities:
  MyProcess:
    events:
      init:
        attributes:
          process: quent::os::Process
  MyThread:
    events:
      init:
        attributes:
          thread: quent::os::Thread
          process: { scope-ref: MyProcess }
",
    );

    assert!(schema.record(&thread_path()).is_some());
    assert!(schema.record(&process_path()).is_some());
}

#[test]
fn process_record_on_multi_event_is_rejected() {
    let errors = errors_of(
        "\
quent: alpha
model: m
entities:
  MyProcess:
    events:
      observed:
        multi: true
        attributes:
          process: quent::os::Process
",
    );

    assert!(errors.contains("must carry OS record `quent::os::Process` in a `Once` event"));
}

#[test]
fn thread_without_process_ancestor_is_rejected() {
    let errors = errors_of(
        "\
quent: alpha
model: m
entities:
  MyThread:
    events:
      started:
        attributes:
          thread: quent::os::Thread
",
    );

    assert!(errors.contains("OS thread entity must be scoped under an OS process entity"));
}

#[test]
fn nested_os_records_are_rejected() {
    let errors = errors_of(
        "\
quent: alpha
model: m
entities:
  Target:
    events:
      init: {}
  Candidate:
    events:
      init:
        attributes:
          optional: { option: quent::os::Process }
          processes: { list: quent::os::Process }
          target: { ref: Target, data: quent::os::Process }
",
    );

    assert_eq!(
        errors
            .matches("must be used directly as an entity event field")
            .count(),
        3,
        "{errors}"
    );
}

#[test]
fn duplicate_os_records_in_one_event_are_rejected() {
    let errors = errors_of(
        "\
quent: alpha
model: m
entities:
  MyProcess:
    events:
      init:
        attributes:
          first: quent::os::Process
          second: quent::os::Process
",
    );

    assert!(
        errors.contains("OS record `quent::os::Process` may be carried by only one event field")
    );
}
