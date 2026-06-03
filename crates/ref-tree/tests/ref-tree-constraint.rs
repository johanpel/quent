// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_constraints::{Constraint as _, Error as ValidatorError, Validator};
use quent_ref_target::{RefTarget, RefTargetConstraint};
use quent_ref_tree::{RefTreeConstraint, RefTreeError};
use quent_schema::{
    Schema,
    annotations::Annotations,
    constraint::Constraint,
    data_type::DataType,
    entity::Entity,
    event::{Cardinality, Event, EventField},
    identifier::Identifier,
    record::{Record, RecordField},
};

fn ident(s: &str) -> Identifier {
    Identifier::try_new(s).unwrap()
}

fn tree_constraint() -> Constraint {
    Constraint {
        name: RefTreeConstraint::NAME.to_string(),
        data: None,
    }
}

fn target_constraint(target: &str) -> Constraint {
    Constraint {
        name: RefTargetConstraint::NAME.to_string(),
        data: Some(
            serde_json::to_string(&RefTarget {
                target: ident(target),
            })
            .unwrap(),
        ),
    }
}

/// A type-erased tree-forming reference (no target).
fn tree_ref() -> DataType {
    DataType::EntityRef {
        data: None,
        annotations: Annotations {
            constraints: vec![tree_constraint()],
            ..Default::default()
        },
    }
}

/// A tree-forming reference restricted to a specific parent entity type.
fn tree_ref_to(target: &str) -> DataType {
    DataType::EntityRef {
        data: None,
        annotations: Annotations {
            constraints: vec![tree_constraint(), target_constraint(target)],
            ..Default::default()
        },
    }
}

/// A plain (non-tree) entity reference carrying `data` as its payload.
fn ref_carrying(data: DataType) -> DataType {
    DataType::EntityRef {
        data: Some(Box::new(data)),
        annotations: Annotations::default(),
    }
}

fn field(name: &str, ty: DataType) -> EventField {
    EventField {
        name: ident(name),
        ty,
        annotations: Annotations::default(),
    }
}

fn event(name: &str, payload: Vec<EventField>) -> Event {
    Event {
        name: ident(name),
        cardinality: Cardinality::Once,
        payload,
        annotations: Annotations::default(),
    }
}

fn entity(name: &str, events: Vec<Event>) -> Entity {
    Entity {
        name: ident(name),
        events,
        annotations: Annotations::default(),
    }
}

/// An entity with no events — carries no tree-forming reference, so it is a root.
fn root(name: &str) -> Entity {
    entity(name, vec![])
}

/// An entity whose single event carries one tree-forming reference `ty`.
fn child(name: &str, ty: DataType) -> Entity {
    entity(name, vec![event("created", vec![field("parent", ty)])])
}

fn record(name: &str, fields: Vec<RecordField>) -> Record {
    Record {
        name: ident(name),
        fields,
        annotations: Annotations::default(),
    }
}

fn record_field(name: &str, ty: DataType) -> RecordField {
    RecordField {
        name: ident(name),
        ty,
        annotations: Annotations::default(),
    }
}

fn schema_with(entities: Vec<Entity>) -> Schema {
    schema_with_records(entities, vec![])
}

fn schema_with_records(entities: Vec<Entity>, records: Vec<Record>) -> Schema {
    Schema {
        name: ident("S"),
        entities,
        records,
        annotations: Annotations::default(),
    }
}

fn validate(schema: &Schema) -> Vec<RefTreeError> {
    let validator = Validator::default()
        .try_with(RefTreeConstraint)
        .unwrap()
        .try_with(RefTargetConstraint)
        .unwrap();
    match validator.validate(schema) {
        Ok(()) => Vec::new(),
        Err(ValidatorError::Invalid { failures, .. }) => {
            for (name, source) in failures {
                if name == RefTreeConstraint::NAME {
                    return match *source.downcast::<RefTreeError>().unwrap() {
                        RefTreeError::Multiple(errors) => errors,
                        single => vec![single],
                    };
                }
            }
            Vec::new()
        }
        Err(_) => unreachable!(),
    }
}

#[test]
fn target_chain_to_root_passes() {
    let schema = schema_with(vec![
        root("Cluster"),
        child("Worker", tree_ref_to("Cluster")),
        child("Task", tree_ref_to("Worker")),
    ]);
    assert!(validate(&schema).is_empty());
}

#[test]
fn single_child_under_root_passes() {
    let schema = schema_with(vec![
        root("Cluster"),
        child("Worker", tree_ref_to("Cluster")),
    ]);
    assert!(validate(&schema).is_empty());
}

#[test]
fn no_tree_ref_anywhere_passes() {
    // The constraint only forms a tree when at least one reference uses it.
    let schema = schema_with(vec![child("Solo", DataType::U64)]);
    assert!(validate(&schema).is_empty());
}

#[test]
fn option_nested_tree_ref_is_found() {
    let nested = DataType::Option(Box::new(tree_ref_to("Cluster")));
    let schema = schema_with(vec![root("Cluster"), child("Worker", nested)]);
    assert!(validate(&schema).is_empty());
}

#[test]
fn list_nested_tree_ref_is_found() {
    let nested = DataType::List(Box::new(tree_ref_to("Cluster")));
    let schema = schema_with(vec![root("Cluster"), child("Worker", nested)]);
    assert!(validate(&schema).is_empty());
}

#[test]
fn tree_ref_in_reference_payload_is_found() {
    let nested = ref_carrying(tree_ref_to("Cluster"));
    let schema = schema_with(vec![root("Cluster"), child("Worker", nested)]);
    assert!(validate(&schema).is_empty());
}

#[test]
fn tree_ref_via_record_field_resolves_parent() {
    // A parent reference reached through a record-typed event field counts.
    let meta = record("Meta", vec![record_field("owner", tree_ref_to("Cluster"))]);
    let worker = child("Worker", DataType::Record(ident("Meta")));
    let schema = schema_with_records(vec![root("Cluster"), worker], vec![meta]);
    assert!(validate(&schema).is_empty());
}

#[test]
fn recursive_record_does_not_loop() {
    // A record that nests itself (via Option) must not send the walker into an
    // infinite descent.
    let meta = record(
        "Meta",
        vec![
            record_field("owner", tree_ref_to("Cluster")),
            record_field(
                "nested",
                DataType::Option(Box::new(DataType::Record(ident("Meta")))),
            ),
        ],
    );
    let worker = child("Worker", DataType::Record(ident("Meta")));
    let schema = schema_with_records(vec![root("Cluster"), worker], vec![meta]);
    assert!(validate(&schema).is_empty());
}

#[test]
fn same_parent_across_events_passes() {
    // Req. 2 permits the parent reference on any number of events.
    let task = entity(
        "Task",
        vec![
            event("created", vec![field("a", tree_ref_to("Cluster"))]),
            event("moved", vec![field("b", tree_ref_to("Cluster"))]),
        ],
    );
    let schema = schema_with(vec![root("Cluster"), task]);
    assert!(validate(&schema).is_empty());
}

#[test]
fn type_erased_tree_ref_is_rejected() {
    // Req. 3: a tree-forming reference must be target-constrained.
    let schema = schema_with(vec![root("Cluster"), child("Worker", tree_ref())]);
    let errors = validate(&schema);
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0],
        RefTreeError::NotTargetConstrained { .. }
    ));
}

#[test]
fn type_erased_tree_ref_via_record_is_rejected() {
    // Req. 3 also reaches references hidden behind a record-typed field.
    let meta = record("Meta", vec![record_field("owner", tree_ref())]);
    let worker = child("Worker", DataType::Record(ident("Meta")));
    let schema = schema_with_records(vec![root("Cluster"), worker], vec![meta]);
    let errors = validate(&schema);
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0],
        RefTreeError::NotTargetConstrained { .. }
    ));
}

#[test]
fn conflicting_parents_is_rejected() {
    // Req. 2: a non-root declaring two distinct parent types across events.
    let task = entity(
        "Task",
        vec![
            event("created", vec![field("a", tree_ref_to("Worker"))]),
            event("moved", vec![field("b", tree_ref_to("Cluster"))]),
        ],
    );
    let schema = schema_with(vec![
        root("Cluster"),
        child("Worker", tree_ref_to("Cluster")),
        task,
    ]);
    let errors = validate(&schema);
    assert_eq!(errors.len(), 1);
    assert!(
        matches!(&errors[0], RefTreeError::ConflictingParents { entity, .. } if entity == "Task"),
    );
}

#[test]
fn two_tree_refs_in_one_event_is_rejected() {
    let task = entity(
        "Task",
        vec![event(
            "created",
            vec![
                field("a", tree_ref_to("Cluster")),
                field("b", tree_ref_to("Cluster")),
            ],
        )],
    );
    let schema = schema_with(vec![root("Cluster"), task]);
    let errors = validate(&schema);
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0],
        RefTreeError::MultiplePerEvent { count: 2, .. }
    ));
}

#[test]
fn no_root_is_rejected() {
    // Req. 1: every entity has a parent, so none is a root.
    let schema = schema_with(vec![
        child("A", tree_ref_to("B")),
        child("B", tree_ref_to("A")),
    ]);
    let errors = validate(&schema);
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0], RefTreeError::NoRoot));
}

#[test]
fn multiple_roots_is_rejected() {
    // Req. 1: two entities carry no tree-forming reference.
    let schema = schema_with(vec![
        root("Cluster"),
        root("Other"),
        child("Worker", tree_ref_to("Cluster")),
    ]);
    let errors = validate(&schema);
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0], RefTreeError::MultipleRoots { .. }));
}

#[test]
fn unknown_target_is_unreachable() {
    // Req. 4: a parent type naming no entity has no path to the root.
    let schema = schema_with(vec![root("Cluster"), child("A", tree_ref_to("Ghost"))]);
    let errors = validate(&schema);
    assert_eq!(errors.len(), 1);
    assert!(matches!(&errors[0], RefTreeError::Unreachable { entity } if entity == "A"));
}

#[test]
fn target_cycle_is_unreachable() {
    // Req. 4: A and B form a cycle with no path to the root.
    let schema = schema_with(vec![
        root("Cluster"),
        child("A", tree_ref_to("B")),
        child("B", tree_ref_to("A")),
    ]);
    let errors = validate(&schema);
    assert_eq!(errors.len(), 2);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, RefTreeError::Unreachable { entity } if entity == "A")),
    );
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, RefTreeError::Unreachable { entity } if entity == "B")),
    );
}
