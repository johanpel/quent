// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_convention::{Convention, Error, Validator};
use quent_schema::{
    Schema,
    convention::Convention as SchemaConvention,
    data_type::DataType,
    entity::Entity,
    event::{Cardinality, Event, EventField},
    identifier::Identifier,
    record::{Field, Record},
};

fn ident(s: &str) -> Identifier {
    Identifier::try_new(s).unwrap()
}

fn validated(name: &str) -> SchemaConvention {
    SchemaConvention {
        name: name.to_string(),
        validated: true,
        data: None,
    }
}

fn metadata(name: &str) -> SchemaConvention {
    SchemaConvention {
        name: name.to_string(),
        validated: false,
        data: None,
    }
}

fn empty_schema() -> Schema {
    Schema {
        name: ident("TestSchema"),
        docs: None,
        entities: vec![],
        records: vec![],
        conventions: vec![],
    }
}

struct NoopA;
impl Convention for NoopA {
    const NAME: &'static str = "a";
    fn validate(&self, _schema: &Schema) -> Result<(), Vec<Error>> {
        Ok(())
    }
}

struct NoopB;
impl Convention for NoopB {
    const NAME: &'static str = "b";
    fn validate(&self, _schema: &Schema) -> Result<(), Vec<Error>> {
        Ok(())
    }
}

struct Failing {
    errors: Vec<Error>,
}

impl Convention for Failing {
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
        Some(Error::DuplicateConvention("a"))
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
fn validated_convention_without_validator_is_unregistered() {
    let schema = Schema {
        conventions: vec![validated("unknown")],
        ..empty_schema()
    };
    let errs = Validator::default().validate(&schema).unwrap_err();
    assert_eq!(errs.len(), 1);
    assert!(matches!(
        &errs[0],
        Error::UnregisteredConvention { convention, .. } if convention == "unknown"
    ));
}

#[test]
fn metadata_convention_without_validator_is_allowed() {
    let schema = Schema {
        conventions: vec![metadata("not_validated")],
        ..empty_schema()
    };
    assert_eq!(Validator::default().validate(&schema), Ok(()));
}

#[test]
fn unregistered_convention_is_detected_at_every_site() {
    let unknown = || validated("unknown");
    let schema = Schema {
        name: ident("S"),
        docs: None,
        conventions: vec![unknown()],
        entities: vec![Entity {
            name: ident("E"),
            docs: None,
            conventions: vec![unknown()],
            events: vec![Event {
                name: ident("Ev"),
                docs: None,
                cardinality: Cardinality::Once,
                conventions: vec![unknown()],
                payload: vec![EventField {
                    name: ident("ef"),
                    docs: None,
                    ty: DataType::U64,
                    conventions: vec![unknown()],
                }],
            }],
        }],
        records: vec![Record {
            name: ident("R"),
            docs: None,
            conventions: vec![unknown()],
            fields: vec![Field {
                name: ident("rf"),
                docs: None,
                ty: DataType::U64,
                conventions: vec![unknown()],
            }],
        }],
    };
    let errs = Validator::default().validate(&schema).unwrap_err();
    assert_eq!(errs.len(), 6, "expected one error per site, got: {errs:?}");
    assert!(
        errs.iter().all(|e| matches!(
            e,
            Error::UnregisteredConvention { convention, .. } if convention == "unknown"
        )),
        "all errors should be UnregisteredConvention for 'unknown', got: {errs:?}",
    );
}

#[test]
fn validator_errors_are_collected() {
    let err = Error::Validation {
        convention: "a".to_string(),
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
        conventions: vec![validated("unknown")],
        ..empty_schema()
    };
    let validator_err = Error::Validation {
        convention: "a".to_string(),
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
        Error::UnregisteredConvention { convention, .. } if convention == "unknown"
    )));
    assert!(errs.contains(&validator_err));
}
