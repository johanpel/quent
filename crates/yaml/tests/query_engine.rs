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
  Distributed query engine.
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
      operator_ids: Vec<Uuid>
      estimate: CostEstimate?
  Stats:
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
    events:
      started:
        once:
          listen_on: Endpoint
          cluster:
            doc: The cluster this engine joined.
            type:
              ref:
      heartbeat:
        multi:
          load: f32
      stopped: once

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
          engine:
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
    assert_eq!(records, ["Endpoint", "CostEstimate", "Stage", "Stats"]);
    let entities: Vec<_> = schema.entities().map(|e| e.name().to_string()).collect();
    assert_eq!(entities, ["Cluster", "Engine", "Query"]);
    let events: Vec<_> = schema
        .entity(&ident("Engine"))
        .expect("Engine")
        .events()
        .map(|e| e.name().to_string())
        .collect();
    assert_eq!(events, ["started", "heartbeat", "stopped"]);
}

#[test]
fn docs_and_cardinalities() {
    let (schema, _) = schema();
    assert_eq!(
        schema
            .record(&ident("Endpoint"))
            .unwrap()
            .annotations()
            .docs(),
        Some("A network endpoint.")
    );
    let cluster = schema.entity(&ident("Cluster")).unwrap();
    assert_eq!(
        cluster.annotations().docs(),
        Some("The root of the deployment.")
    );
    let on = cluster.event(&ident("on")).unwrap();
    assert_eq!(on.annotations().docs(), Some("First engine registered."));
    assert!(matches!(on.cardinality(), Cardinality::Once));
    let heartbeat = event(&schema, "Engine", "heartbeat");
    assert!(matches!(heartbeat.cardinality(), Cardinality::Multi));
    let off = cluster.event(&ident("off")).unwrap();
    assert!(matches!(off.cardinality(), Cardinality::Once));
    assert_eq!(off.fields().count(), 0);
}

#[test]
fn field_types() {
    let (schema, _) = schema();
    let stage = schema.record(&ident("Stage")).unwrap();
    assert_eq!(
        stage.field(&ident("operator_ids")).unwrap().ty(),
        &DataType::List(Box::new(DataType::Uuid))
    );
    assert_eq!(
        stage.field(&ident("estimate")).unwrap().ty(),
        &DataType::Option(Box::new(DataType::Record(ident("CostEstimate"))))
    );
    assert_eq!(
        schema
            .record(&ident("Stats"))
            .unwrap()
            .field(&ident("extra"))
            .unwrap()
            .ty(),
        &DataType::DynamicRecord
    );
}

#[test]
fn entity_refs_carry_data_and_annotations() {
    let (schema, _) = schema();
    let cluster_ref = event(&schema, "Engine", "started")
        .field(&ident("cluster"))
        .unwrap()
        .ty();
    let DataType::EntityRef { data, annotations } = cluster_ref else {
        panic!("expected an entity ref");
    };
    assert!(data.is_none());
    assert_eq!(annotations.constraints().count(), 0);

    let engine_ref = event(&schema, "Query", "submitted")
        .field(&ident("engine"))
        .unwrap()
        .ty();
    let DataType::EntityRef { data, annotations } = engine_ref else {
        panic!("expected an entity ref");
    };
    assert_eq!(data.as_deref(), Some(&DataType::U64));
    assert_eq!(
        annotations.constraint("acme.link.v1").unwrap().data(),
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
            .unwrap()
            .data(),
        Some("approved-2026-07")
    );
    assert_eq!(
        schema
            .annotations()
            .metadata("quent.ui.theme.v1")
            .unwrap()
            .data(),
        Some(r#"{"hue":220,"icon":"database"}"#)
    );
    let lifecycle = schema
        .entity(&ident("Query"))
        .unwrap()
        .annotations()
        .constraint("acme.lifecycle.v1")
        .unwrap()
        .data()
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(lifecycle).unwrap();
    assert_eq!(parsed["initial"], "submitted");
    assert_eq!(parsed["terminal"][0], "finished");
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
fn null_bodies_equal_empty() {
    let src = "quent: 1\nmodel: m\nrecords:\n  Marker:\nentities:\n  Thing:\n";
    let schema = load_str(src).expect("loads").schema;
    assert_eq!(schema.record(&ident("Marker")).unwrap().fields().count(), 0);
    assert_eq!(schema.entity(&ident("Thing")).unwrap().events().count(), 0);
}
