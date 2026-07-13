// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Golden test: a full-featured model exercising every format-1 construct.

use quent_schema::test_utils::ident;
use quent_schema::{Cardinality, DataType, Schema};
use quent_yaml::load_str;

const MODEL: &str = r#"
quent: 1
model: query_engine
doc: >-
  Distributed query engine: clusters host engines, engines execute
  queries submitted by clients.
constraints:
  acme.schema-review.v1: approved-2026-07
metadata:
  team: query-runtime
  quent.ui.theme.v1: { hue: 220, icon: database }

records:
  Endpoint:
    doc: A network endpoint.
    fields:
      host: String
      port: u16

  CostEstimate:
    fields: { cpu: f64, mem_bytes: u64 }

  Stage:
    fields:
      name: String
      operator_ids: Vec<Uuid>
      estimate: CostEstimate?

  Plan:
    doc: A fully optimized execution plan.
    fields:
      stages: Vec<Stage>
      revision:
        type: u32
        doc: Bumped every time the optimizer replans.
        metadata:
          quent.ui.hidden.v1:

  Stats:
    doc: Free-form, runtime-keyed statistics.
    fields:
      rows: u64
      extra: Dynamic

entities:
  Cluster:
    doc: The root of the deployment.
    events:
      'on':
        doc: First engine registered.
        once:
          region: String
      'off': once

  Engine:
    metadata:
      owner: engine-core
    events:
      started:
        once:
          listen_on: Endpoint
          cluster:
            doc: The cluster this engine joined.
            type: Ref
      heartbeat:
        multi:
          load: f32
          inflight: u32
      stopped:
        once:
          error: Option<String>

  Query:
    constraints:
      acme.lifecycle.v1:
        initial: submitted
        terminal: [finished]
    events:
      submitted:
        once:
          text:
            type: String
            constraints:
              quent.pii.v1: redact
          plan: Plan?
          engine:
            doc: The engine executing this query.
            type:
              ref:
              data: u64
              constraints:
                acme.link.v1: strong
      progress:
        multi: { stats: Stats }
      finished:
        once:
          ok: bool
          error: String?
"#;

fn schema() -> (Schema, Vec<String>) {
    let loaded = load_str(MODEL).expect("model loads");
    let warnings = loaded.warnings.iter().map(|w| w.message.clone()).collect();
    (loaded.schema, warnings)
}

/// Navigate to an event, panicking on a missing name.
fn event<'s>(schema: &'s Schema, entity: &str, event: &str) -> &'s quent_schema::Event {
    schema
        .entity(&ident(entity))
        .expect("declared entity")
        .event(&ident(event))
        .expect("declared event")
}

#[test]
fn names_counts_and_order() {
    let (schema, _) = schema();
    assert_eq!(schema.name(), "query_engine");
    let records: Vec<_> = schema.records().map(|r| r.name().to_string()).collect();
    assert_eq!(
        records,
        ["Endpoint", "CostEstimate", "Stage", "Plan", "Stats"]
    );
    let entities: Vec<_> = schema.entities().map(|e| e.name().to_string()).collect();
    assert_eq!(entities, ["Cluster", "Engine", "Query"]);
    let engine_events: Vec<_> = schema
        .entity(&ident("Engine"))
        .expect("Engine")
        .events()
        .map(|e| e.name().to_string())
        .collect();
    assert_eq!(engine_events, ["started", "heartbeat", "stopped"]);
}

#[test]
fn docs_at_every_level() {
    let (schema, _) = schema();
    assert!(
        schema
            .annotations()
            .docs()
            .expect("schema doc")
            .starts_with("Distributed query engine")
    );
    let endpoint = schema.record(&ident("Endpoint")).expect("Endpoint");
    assert_eq!(endpoint.annotations().docs(), Some("A network endpoint."));
    let cluster = schema.entity(&ident("Cluster")).expect("Cluster");
    assert_eq!(
        cluster.annotations().docs(),
        Some("The root of the deployment.")
    );
    let on = cluster.event(&ident("on")).expect("on event");
    assert_eq!(on.annotations().docs(), Some("First engine registered."));
    let plan = schema.record(&ident("Plan")).expect("Plan");
    let revision = plan.field(&ident("revision")).expect("revision");
    assert_eq!(
        revision.annotations().docs(),
        Some("Bumped every time the optimizer replans.")
    );
}

#[test]
fn cardinalities_and_payloads() {
    let (schema, _) = schema();
    let cluster = schema.entity(&ident("Cluster")).expect("Cluster");
    assert!(matches!(
        cluster.event(&ident("on")).expect("on").cardinality(),
        Cardinality::Once
    ));
    let off = cluster.event(&ident("off")).expect("one-liner event");
    assert!(matches!(off.cardinality(), Cardinality::Once));
    assert_eq!(off.fields().count(), 0);
    let heartbeat = event(&schema, "Engine", "heartbeat");
    assert!(matches!(heartbeat.cardinality(), Cardinality::Multi));
    assert_eq!(heartbeat.fields().count(), 2);
}

#[test]
fn field_types() {
    let (schema, _) = schema();
    let stage = schema.record(&ident("Stage")).expect("Stage");
    assert_eq!(
        stage
            .field(&ident("operator_ids"))
            .expect("operator_ids")
            .ty(),
        &DataType::List(Box::new(DataType::Uuid))
    );
    // `T?` is sugar for Option<T>.
    assert_eq!(
        stage.field(&ident("estimate")).expect("estimate").ty(),
        &DataType::Option(Box::new(DataType::Record(ident("CostEstimate"))))
    );
    let stats = schema.record(&ident("Stats")).expect("Stats");
    assert_eq!(
        stats.field(&ident("extra")).expect("extra").ty(),
        &DataType::DynamicRecord
    );
    let stopped = event(&schema, "Engine", "stopped");
    assert_eq!(
        stopped.field(&ident("error")).expect("error").ty(),
        &DataType::Option(Box::new(DataType::String))
    );
}

#[test]
fn entity_refs_carry_data_and_annotations() {
    let (schema, _) = schema();
    let started = event(&schema, "Engine", "started");
    let DataType::EntityRef { data, annotations } =
        started.field(&ident("cluster")).expect("cluster").ty()
    else {
        panic!("expected an entity ref");
    };
    assert!(data.is_none());
    assert_eq!(annotations.constraints().count(), 0);

    let submitted = event(&schema, "Query", "submitted");
    let DataType::EntityRef { data, annotations } =
        submitted.field(&ident("engine")).expect("engine").ty()
    else {
        panic!("expected an entity ref");
    };
    assert_eq!(data.as_deref(), Some(&DataType::U64));
    assert_eq!(
        annotations.constraint("acme.link.v1").expect("link").data(),
        Some("strong")
    );
}

#[test]
fn constraint_and_metadata_payloads() {
    let (schema, _) = schema();
    assert_eq!(
        schema
            .annotations()
            .constraint("acme.schema-review.v1")
            .expect("review")
            .data(),
        Some("approved-2026-07")
    );
    assert_eq!(
        schema
            .annotations()
            .metadata("quent.ui.theme.v1")
            .expect("theme")
            .data(),
        Some(r#"{"hue":220,"icon":"database"}"#)
    );
    let lifecycle = schema
        .entity(&ident("Query"))
        .expect("Query")
        .annotations()
        .constraint("acme.lifecycle.v1")
        .expect("lifecycle")
        .data()
        .expect("lifecycle payload");
    let parsed: serde_json::Value = serde_json::from_str(lifecycle).expect("valid JSON");
    assert_eq!(parsed["initial"], "submitted");
    assert_eq!(parsed["terminal"][0], "finished");
    let text = event(&schema, "Query", "submitted")
        .field(&ident("text"))
        .expect("text");
    assert_eq!(
        text.annotations()
            .constraint("quent.pii.v1")
            .expect("pii")
            .data(),
        Some("redact")
    );
    let revision = schema
        .record(&ident("Plan"))
        .expect("Plan")
        .field(&ident("revision"))
        .expect("revision");
    assert_eq!(
        revision
            .annotations()
            .metadata("quent.ui.hidden.v1")
            .expect("hidden")
            .data(),
        None
    );
}

#[test]
fn unregistered_constraints_are_warnings() {
    let (_, warnings) = schema();
    assert!(
        warnings.iter().any(|w| w.contains("acme.schema-review.v1")),
        "{warnings:?}"
    );
    assert!(
        warnings.iter().any(|w| w.contains("quent.pii.v1")),
        "{warnings:?}"
    );
    // Metadata is never validated, so it never warns.
    assert!(
        !warnings.iter().any(|w| w.contains("quent.ui.theme.v1")),
        "{warnings:?}"
    );
}

#[test]
fn ref_grammar_and_structured_form_lower_identically() {
    let grammar = r#"
quent: 1
model: m
entities:
  Engine:
    events:
      started:
        once:
          cluster: Ref
"#;
    let structured = r#"
quent: 1
model: m
entities:
  Engine:
    events:
      started:
        once:
          cluster:
            type:
              ref:
"#;
    let grammar = load_str(grammar).expect("grammar form loads").schema;
    let structured = load_str(structured).expect("structured form loads").schema;
    let field_ty = |schema: &Schema| {
        event(schema, "Engine", "started")
            .field(&ident("cluster"))
            .expect("cluster")
            .ty()
            .clone()
    };
    assert_eq!(field_ty(&grammar), field_ty(&structured));
    assert!(matches!(
        field_ty(&grammar),
        DataType::EntityRef { data: None, .. }
    ));
}

#[test]
fn format_version_accepts_quoted_one() {
    assert!(load_str("quent: '1'\nmodel: m\n").is_ok());
}

#[test]
fn null_bodies_equal_empty_mappings() {
    let src = r#"
quent: 1
model: m
records:
  Marker:
entities:
  Thing: {}
  Other:
    events:
      touched:
        multi:
"#;
    let loaded = load_str(src).expect("loads");
    assert_eq!(
        loaded
            .schema
            .record(&ident("Marker"))
            .expect("Marker")
            .fields()
            .count(),
        0
    );
    assert_eq!(
        loaded
            .schema
            .entity(&ident("Thing"))
            .expect("Thing")
            .events()
            .count(),
        0
    );
    let touched = event(&loaded.schema, "Other", "touched");
    assert_eq!(touched.fields().count(), 0);
}
