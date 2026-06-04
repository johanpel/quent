// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_constraints::{Constraint, validate};
use quent_schema::{
    Annotations, Cardinality, Constraint as SchemaConstraint, DataType, Entity, Event, Field,
    Identifier, Metadata, Record, Schema,
    visitor::{Cursor, SchemaIndex, Visitor},
};

fn ident(s: &str) -> Identifier {
    Identifier::try_new(s).unwrap()
}

fn constraint(name: &str) -> SchemaConstraint {
    SchemaConstraint {
        name: name.to_string(),
        data: None,
    }
}

fn metadata(name: &str) -> Metadata {
    Metadata {
        name: name.to_string(),
        data: None,
    }
}

fn empty_schema() -> Schema {
    Schema {
        name: ident("TestSchema"),
        entities: vec![],
        records: vec![],
        annotations: Annotations::default(),
    }
}

// A constraint that finds no violations.
#[derive(Default)]
struct NoopA;
impl Visitor for NoopA {
    type Output = Result<(), Box<dyn std::error::Error>>;
    fn visit(&mut self, _cursor: &Cursor, _index: &SchemaIndex) {}
    fn finish(self) -> Self::Output {
        Ok(())
    }
}
impl Constraint for NoopA {
    const NAME: &'static str = "a";
}

#[derive(Default)]
struct NoopB;
impl Visitor for NoopB {
    type Output = Result<(), Box<dyn std::error::Error>>;
    fn visit(&mut self, _cursor: &Cursor, _index: &SchemaIndex) {}
    fn finish(self) -> Self::Output {
        Ok(())
    }
}
impl Constraint for NoopB {
    const NAME: &'static str = "b";
}

// A minimal error type for a failing constraint.
#[derive(Debug)]
struct Boom(&'static str);
impl std::fmt::Display for Boom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for Boom {}

// A constraint that always fails.
#[derive(Default)]
struct Failing;
impl Visitor for Failing {
    type Output = Result<(), Box<dyn std::error::Error>>;
    fn visit(&mut self, _cursor: &Cursor, _index: &SchemaIndex) {}
    fn finish(self) -> Self::Output {
        Err(Box::new(Boom("boom")))
    }
}
impl Constraint for Failing {
    const NAME: &'static str = "a";
}

#[test]
fn passing_constraint_on_empty_schema() {
    let report = validate::<(NoopA,)>(&empty_schema());
    assert!(report.unregistered.is_empty());
    assert!(report.results.0.is_ok());
}

#[test]
fn constraint_without_validator_is_unregistered() {
    let schema = Schema {
        annotations: Annotations {
            constraints: vec![constraint("unknown")],
            ..Default::default()
        },
        ..empty_schema()
    };
    let report = validate::<(NoopA,)>(&schema);
    assert_eq!(report.unregistered.len(), 1);
    assert!(report.unregistered.contains("unknown"));
    assert!(report.results.0.is_ok());
}

#[test]
fn metadata_is_never_validated() {
    let schema = Schema {
        annotations: Annotations {
            metadata: vec![metadata("not_validated")],
            ..Default::default()
        },
        ..empty_schema()
    };
    let report = validate::<(NoopA,)>(&schema);
    assert!(report.unregistered.is_empty());
}

#[test]
fn unregistered_constraint_is_reported_once() {
    let unknown = || Annotations {
        constraints: vec![constraint("unknown")],
        ..Default::default()
    };
    let schema = Schema {
        name: ident("S"),
        annotations: unknown(),
        entities: vec![Entity {
            name: ident("E"),
            annotations: unknown(),
            events: vec![Event {
                name: ident("Ev"),
                cardinality: Cardinality::Once,
                annotations: unknown(),
                payload: vec![Field {
                    name: ident("ef"),
                    ty: DataType::U64,
                    annotations: unknown(),
                }],
            }],
        }],
        records: vec![Record {
            name: ident("R"),
            annotations: unknown(),
            fields: vec![Field {
                name: ident("rf"),
                ty: DataType::U64,
                annotations: unknown(),
            }],
        }],
    };
    let report = validate::<(NoopA,)>(&schema);
    // The same name used at six sites is deduplicated to a single entry.
    assert_eq!(
        report.unregistered.into_iter().collect::<Vec<_>>(),
        vec!["unknown".to_string()]
    );
}

#[test]
fn constraint_failure_is_reported_per_constraint() {
    let report = validate::<(Failing, NoopB)>(&empty_schema());
    assert!(report.results.0.is_err());
    assert_eq!(report.results.0.as_ref().unwrap_err().to_string(), "boom");
    assert!(report.results.1.is_ok());
}

#[test]
fn unregistered_and_failure_aggregate() {
    let schema = Schema {
        annotations: Annotations {
            constraints: vec![constraint("unknown")],
            ..Default::default()
        },
        ..empty_schema()
    };
    let report = validate::<(Failing,)>(&schema);
    assert!(report.unregistered.contains("unknown"));
    assert!(report.results.0.is_err());
}
