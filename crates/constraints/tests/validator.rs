// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_constraints::{Constraint, Error, Validator};
use quent_schema::{
    Schema,
    annotations::Annotations,
    constraint::Constraint as SchemaConstraint,
    data_type::DataType,
    entity::Entity,
    event::{Cardinality, Event, EventField},
    identifier::Identifier,
    metadata::Metadata,
    record::{Record, RecordField},
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

// A constraint's name is now type-level (`const NAME`), so distinct names
// require distinct types.
struct NoopA;
impl Constraint for NoopA {
    const NAME: &'static str = "a";
    fn validate(&self, _schema: &Schema) -> Result<(), Vec<Error>> {
        Ok(())
    }
}

struct NoopB;
impl Constraint for NoopB {
    const NAME: &'static str = "b";
    fn validate(&self, _schema: &Schema) -> Result<(), Vec<Error>> {
        Ok(())
    }
}

struct Failing {
    errors: Vec<Error>,
}

impl Constraint for Failing {
    const NAME: &'static str = "a";
    fn validate(&self, _schema: &Schema) -> Result<(), Vec<Error>> {
        Err(self.errors.clone())
    }
}

#[test]
fn empty_validator_on_empty_schema_passes() {
    assert_eq!(Validator::default().validate(&empty_schema()), Ok(()));
}

#[test]
fn try_with_rejects_duplicate_name() {
    assert_eq!(
        Validator::default()
            .try_with(NoopA)
            .unwrap()
            .try_with(NoopA)
            .err(),
        Some(Error::DuplicateConstraint("a"))
    );
}

#[test]
fn try_with_accepts_distinct_names() {
    Validator::default()
        .try_with(NoopA)
        .unwrap()
        .try_with(NoopB)
        .unwrap();
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
    let errs = Validator::default().validate(&schema).unwrap_err();
    assert_eq!(errs.len(), 1);
    assert!(matches!(
        &errs[0],
        Error::UnregisteredConstraint { constraint, .. } if constraint == "unknown"
    ));
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
    assert_eq!(Validator::default().validate(&schema), Ok(()));
}

#[test]
fn unregistered_constraint_is_detected_at_every_site() {
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
                payload: vec![EventField {
                    name: ident("ef"),
                    ty: DataType::U64,
                    annotations: unknown(),
                }],
            }],
        }],
        records: vec![Record {
            name: ident("R"),
            annotations: unknown(),
            fields: vec![RecordField {
                name: ident("rf"),
                ty: DataType::U64,
                annotations: unknown(),
            }],
        }],
    };
    let errs = Validator::default().validate(&schema).unwrap_err();
    assert_eq!(errs.len(), 6, "expected one error per site, got: {errs:?}");
    assert!(
        errs.iter().all(|e| matches!(
            e,
            Error::UnregisteredConstraint { constraint, .. } if constraint == "unknown"
        )),
        "all errors should be UnregisteredConstraint for 'unknown', got: {errs:?}",
    );
}

#[test]
fn validator_errors_are_collected() {
    let err = Error::Validation {
        constraint: "a".to_string(),
        message: "boom".to_string(),
    };
    let errs = Validator::default()
        .try_with(Failing {
            errors: vec![err.clone(), err.clone()],
        })
        .unwrap()
        .try_with(NoopB)
        .unwrap()
        .validate(&empty_schema())
        .unwrap_err();
    assert_eq!(errs, vec![err.clone(), err]);
}

#[test]
fn unregistered_and_validator_errors_aggregate() {
    let schema = Schema {
        annotations: Annotations {
            constraints: vec![constraint("unknown")],
            ..Default::default()
        },
        ..empty_schema()
    };
    let validator_err = Error::Validation {
        constraint: "a".to_string(),
        message: "boom".to_string(),
    };
    let errs = Validator::default()
        .try_with(Failing {
            errors: vec![validator_err.clone()],
        })
        .unwrap()
        .validate(&schema)
        .unwrap_err();
    assert_eq!(errs.len(), 2);
    assert!(errs.iter().any(|e| matches!(
        e,
        Error::UnregisteredConstraint { constraint, .. } if constraint == "unknown"
    )));
    assert!(errs.contains(&validator_err));
}
