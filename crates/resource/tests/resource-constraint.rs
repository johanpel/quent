// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_constraints::Constraint as _;
use quent_fsm::FsmConstraint;
use quent_resource::{ResourceConstraint, ResourceError};
use quent_schema::{
    Annotations, Cardinality, DataType, Entity, Event, Field, Record, Schema,
    builder::{AnnotationsBuilder, EntityBuilder, EventBuilder, RecordBuilder, SchemaBuilder},
    test_utils::{field, ident, schema},
};

// Constraint data is built as JSON directly so a test can build it in an
// invalid way (see `capacity_name_must_equal_its_key`).

/// A `definition` role over capacities given as `(name, kind, bounded)`.
fn definition(capacities: &[(&str, &str, bool)]) -> String {
    let capacities: serde_json::Map<String, serde_json::Value> = capacities
        .iter()
        .map(|&(name, kind, bounded)| {
            (
                name.to_string(),
                serde_json::json!({ "name": name, "kind": kind, "bounded": bounded }),
            )
        })
        .collect();
    serde_json::json!({ "definition": serde_json::Value::Object(capacities) }).to_string()
}

fn usage_data(resource: &str) -> String {
    serde_json::json!({ "usage": { "resource": resource } }).to_string()
}

fn bounds_data(resource: &str) -> String {
    serde_json::json!({ "bounds": { "resource": resource } }).to_string()
}

/// Annotations carrying the resource constraint with `data`.
fn resource_annotations(data: String) -> Annotations {
    AnnotationsBuilder::new()
        .try_with_constraint(ResourceConstraint::NAME, Some(data))
        .unwrap()
        .build()
}

fn fsm_annotations() -> Annotations {
    AnnotationsBuilder::new()
        .try_with_constraint(FsmConstraint::NAME, None)
        .unwrap()
        .build()
}

/// A record carrying resource `data`, with a `U64` field for each name.
fn role_record(name: &str, data: String, fields: &[&str]) -> Record {
    let mut builder = RecordBuilder::new(ident(name)).with_annotations(resource_annotations(data));
    for &f in fields {
        builder = builder.try_with_field(field(f, DataType::U64)).unwrap();
    }
    builder.build()
}

fn usage_record(name: &str, resource: &str, claims: &[&str]) -> Record {
    role_record(name, usage_data(resource), claims)
}

fn bounds_record(name: &str, resource: &str, fields: &[&str]) -> Record {
    role_record(name, bounds_data(resource), fields)
}

fn event(name: &str, field_name: &str, ty: DataType) -> Event {
    quent_schema::test_utils::event(name, [field(field_name, ty)])
}

/// A resource entity carrying `definition`, with a bounds event referencing the
/// `bounds` record when given.
fn resource_entity(name: &str, definition: String, bounds: Option<&str>) -> Entity {
    let mut builder =
        EntityBuilder::new(ident(name)).with_annotations(resource_annotations(definition));
    if let Some(bounds) = bounds {
        builder = builder
            .try_with_event(event(
                "operating",
                "bounds",
                DataType::Record(ident(bounds)),
            ))
            .unwrap();
    }
    builder.build()
}

/// An entity with one event referencing `record`. It is an FSM iff `fsm`, and
/// the reference rides on an entity reference iff `on_ref`.
fn user(name: &str, fsm: bool, record: &str, on_ref: bool) -> Entity {
    let record = DataType::Record(ident(record));
    let ty = if on_ref {
        DataType::EntityRef {
            data: Some(Box::new(record)),
            annotations: Annotations::default(),
        }
    } else {
        record
    };
    let mut builder = EntityBuilder::new(ident(name))
        .try_with_event(event("using", "claim", ty))
        .unwrap();
    if fsm {
        builder = builder.with_annotations(fsm_annotations());
    }
    builder.build()
}

fn validate(schema: &Schema) -> Vec<ResourceError> {
    match quent_constraints::validate::<(ResourceConstraint,)>(schema)
        .results
        .0
    {
        Ok(()) => Vec::new(),
        Err(ResourceError::Multiple(errors)) => errors,
        Err(single) => vec![single],
    }
}

/// A resource, its bounds and usage records, and an FSM user of it.
#[test]
fn valid_resource_passes() {
    let memory = resource_entity(
        "Memory",
        definition(&[("bytes", "occupancy", true)]),
        Some("MemoryBounds"),
    );
    let worker = user("Worker", true, "MemoryUsage", true);
    let bounds = bounds_record("MemoryBounds", "Memory", &["bytes"]);
    let usage = usage_record("MemoryUsage", "Memory", &["bytes"]);
    assert!(validate(&schema("App", vec![memory, worker], vec![bounds, usage])).is_empty());
}

/// Requirement 1: a resource has at least one capacity.
#[test]
fn resource_without_capacity_is_rejected() {
    let memory = resource_entity("Memory", definition(&[]), None);
    let errors = validate(&schema("App", vec![memory], vec![]));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ResourceError::NoCapacities { .. }))
    );
}

/// Requirement 2: a capacity's identifier is unique within a resource. Two
/// capacities sharing a name can only be keyed distinctly, so the duplicate
/// surfaces as a name that is not its key.
#[test]
fn duplicate_capacity_names_are_rejected() {
    let definition = serde_json::json!({
        "definition": {
            "bytes": { "name": "bytes", "kind": "occupancy", "bounded": false },
            "octets": { "name": "bytes", "kind": "occupancy", "bounded": false }
        }
    })
    .to_string();
    let memory = resource_entity("Memory", definition, None);
    let errors = validate(&schema("App", vec![memory], vec![]));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ResourceError::MismatchedCapacityName { .. }))
    );
}

/// Requirement 3: a bounded resource has a bounds record covering exactly its
/// bounded capacities.
#[test]
fn bounds_must_match_the_bounded_capacities() {
    // Memory is bounded but has no bounds record.
    let memory = resource_entity("Memory", definition(&[("bytes", "occupancy", true)]), None);
    // Disk's bounds omit the bounded `blocks` and bound the unbounded `watts`.
    let disk = resource_entity(
        "Disk",
        definition(&[
            ("blocks", "occupancy", true),
            ("sectors", "occupancy", true),
            ("watts", "rate", false),
        ]),
        None,
    );
    let disk_bounds = bounds_record("DiskBounds", "Disk", &["sectors", "watts"]);
    let errors = validate(&schema("App", vec![memory, disk], vec![disk_bounds]));
    assert!(
        errors.iter().any(
            |e| matches!(e, ResourceError::MissingBounds { resource } if resource == "Memory")
        )
    );
    assert!(errors.iter().any(
        |e| matches!(e, ResourceError::UncoveredCapacity { capacity, .. } if capacity == "blocks")
    ));
    assert!(errors.iter().any(
        |e| matches!(e, ResourceError::UnboundedCapacity { capacity, .. } if capacity == "watts")
    ));
}

#[test]
fn bounded_resource_requires_a_bounds_event() {
    let memory = resource_entity("Memory", definition(&[("bytes", "occupancy", true)]), None);
    let bounds = bounds_record("MemoryBounds", "Memory", &["bytes"]);
    let errors = validate(&schema("App", vec![memory], vec![bounds]));
    assert!(
        errors.iter().any(
            |e| matches!(e, ResourceError::MissingBounds { resource } if resource == "Memory")
        )
    );
}

#[test]
fn unbounded_resource_rejects_bounds() {
    let memory = resource_entity(
        "Memory",
        definition(&[("bytes", "occupancy", false)]),
        Some("MemoryBounds"),
    );
    let bounds = bounds_record("MemoryBounds", "Memory", &[]);
    let errors = validate(&schema("App", vec![memory], vec![bounds]));
    assert!(errors.iter().any(
        |e| matches!(e, ResourceError::UnexpectedBounds { resource, .. } if resource == "Memory")
    ));
}

/// Requirement 4: only an FSM entity may use a resource.
#[test]
fn non_fsm_user_is_rejected() {
    let memory = resource_entity("Memory", definition(&[("bytes", "occupancy", false)]), None);
    let worker = user("Worker", false, "MemoryUsage", true);
    let usage = usage_record("MemoryUsage", "Memory", &["bytes"]);
    let errors = validate(&schema("App", vec![memory, worker], vec![usage]));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ResourceError::NonFsmUser { entity, .. } if entity == "Worker"))
    );
}

/// Requirement 5: a usage names a declared resource.
#[test]
fn usage_of_undeclared_resource_is_rejected() {
    let usage = usage_record("GhostUsage", "Ghost", &[]);
    let errors = validate(&schema("App", vec![], vec![usage]));
    assert!(errors.iter().any(
        |e| matches!(e, ResourceError::UnknownResource { resource, .. } if resource == "Ghost")
    ));
}

/// Requirement 6: a usage claims only its resource's capacities.
#[test]
fn usage_claiming_undeclared_capacity_is_rejected() {
    let memory = resource_entity("Memory", definition(&[("bytes", "occupancy", false)]), None);
    let usage = usage_record("MemoryUsage", "Memory", &["watts"]);
    let errors = validate(&schema("App", vec![memory], vec![usage]));
    assert!(errors.iter().any(
        |e| matches!(e, ResourceError::UndeclaredCapacity { capacity, .. } if capacity == "watts")
    ));
}

/// Requirement 7: a usage record rides on an entity reference.
#[test]
fn usage_record_off_an_entity_reference_is_rejected() {
    let memory = resource_entity("Memory", definition(&[("bytes", "occupancy", false)]), None);
    let worker = user("Worker", true, "MemoryUsage", false);
    let usage = usage_record("MemoryUsage", "Memory", &["bytes"]);
    let errors = validate(&schema("App", vec![memory, worker], vec![usage]));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ResourceError::UsageNotOnReference { .. }))
    );
}

/// Requirement 8: a bounds record appears only on its own resource's events.
#[test]
fn bounds_record_used_by_a_foreign_entity_is_rejected() {
    let memory = resource_entity(
        "Memory",
        definition(&[("bytes", "occupancy", true)]),
        Some("MemoryBounds"),
    );
    let bounds = bounds_record("MemoryBounds", "Memory", &["bytes"]);
    let intruder = user("Intruder", false, "MemoryBounds", false);
    let errors = validate(&schema("App", vec![memory, intruder], vec![bounds]));
    assert!(errors.iter().any(
        |e| matches!(e, ResourceError::ForeignBounds { resource, .. } if resource == "Memory")
    ));
}

fn assert_misplaced(schema: &Schema) {
    assert!(
        validate(schema)
            .iter()
            .any(|e| matches!(e, ResourceError::MisplacedRole { .. }))
    );
}

/// A resource role placed on the wrong element kind is rejected.
#[test]
fn misplaced_role_is_rejected() {
    // A definition on a record.
    let bad_record = role_record("Memory", definition(&[("bytes", "occupancy", false)]), &[]);
    assert_misplaced(&schema("App", vec![], vec![bad_record]));

    // A usage on an entity.
    let bad_entity = EntityBuilder::new(ident("Worker"))
        .with_annotations(resource_annotations(usage_data("Memory")))
        .build();
    assert_misplaced(&schema("App", vec![bad_entity], vec![]));
}

#[test]
fn roles_on_other_annotated_elements_are_rejected() {
    let bad_schema = SchemaBuilder::new(ident("App"))
        .with_annotations(resource_annotations(usage_data("Memory")))
        .build();
    assert_misplaced(&bad_schema);

    let bad_event = EventBuilder::new(ident("bad"), Cardinality::Once)
        .with_annotations(resource_annotations(usage_data("Memory")))
        .build();
    let entity = EntityBuilder::new(ident("Worker"))
        .try_with_event(bad_event)
        .unwrap()
        .build();
    assert_misplaced(&schema("App", vec![entity], vec![]));

    let bad_field = Field::new(
        ident("bad"),
        DataType::U64,
        resource_annotations(usage_data("Memory")),
    );
    let field_event = EventBuilder::new(ident("using"), Cardinality::Once)
        .try_with_field(bad_field)
        .unwrap()
        .build();
    let entity = EntityBuilder::new(ident("Worker"))
        .try_with_event(field_event)
        .unwrap()
        .build();
    assert_misplaced(&schema("App", vec![entity], vec![]));

    let bad_ref = DataType::EntityRef {
        data: None,
        annotations: resource_annotations(usage_data("Memory")),
    };
    let entity = EntityBuilder::new(ident("Worker"))
        .try_with_event(event("using", "claim", bad_ref))
        .unwrap()
        .build();
    assert_misplaced(&schema("App", vec![entity], vec![]));
}

/// A usage carried by an entity reference with no enclosing entity is rejected.
#[test]
fn usage_without_enclosing_entity_is_rejected() {
    let memory = resource_entity("Memory", definition(&[("bytes", "occupancy", false)]), None);
    let usage = usage_record("MemoryUsage", "Memory", &["bytes"]);
    // A record, not an entity, carries the usage on an entity-ref field.
    let carrier = DataType::EntityRef {
        data: Some(Box::new(DataType::Record(ident("MemoryUsage")))),
        annotations: Annotations::default(),
    };
    let wrapper = RecordBuilder::new(ident("Wrapper"))
        .try_with_field(field("carried", carrier))
        .unwrap()
        .build();
    assert_misplaced(&schema("App", vec![memory], vec![usage, wrapper]));
}
