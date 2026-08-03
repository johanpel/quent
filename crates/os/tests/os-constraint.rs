// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_constraints::Constraint as _;
use quent_os::{
    Os, OsBuilder, OsConstraint, OsError, OsParts, PROCESS_ID_PATH, THREAD_ID_PATH,
    process_id_path, thread_id_path,
};
use quent_resource::{Resource, ResourceConstraint};
use quent_schema::{
    Annotations, Cardinality, DataType, Entity, Field, Record, Schema,
    builder::{AnnotationsBuilder, EntityBuilder, EventBuilder, RecordBuilder, SchemaBuilder},
    test_utils::{field, ident, path},
};

fn annotations(os: Os, resource: Option<serde_json::Value>) -> Annotations {
    let mut builder = AnnotationsBuilder::new()
        .with_constraint(OsConstraint::NAME, Some(os.constraint_data().unwrap()));
    if let Some(resource) = resource {
        builder = builder.with_constraint(Resource::NAME, Some(resource.to_string()));
    }
    builder.build().unwrap()
}

fn id_record(path: &str, fields: impl IntoIterator<Item = Field>) -> Record {
    RecordBuilder::new(path.parse::<quent_schema::Path>().unwrap())
        .with_fields(fields)
        .build()
        .unwrap()
}

fn entity_from_parts(
    name: &str,
    parts: OsParts,
    resource: Option<serde_json::Value>,
) -> (Entity, Record) {
    let entity = EntityBuilder::new(path(name))
        .with_event(
            EventBuilder::new(ident("created"), Cardinality::Once)
                .with_field(field("os_id", DataType::Record(parts.id.path().clone())))
                .build()
                .unwrap(),
        )
        .with_annotations(annotations(parts.definition, resource))
        .build()
        .unwrap();
    (entity, parts.id)
}

fn process_entity() -> (Entity, Record) {
    entity_from_parts("Process", OsBuilder::process().build().unwrap(), None)
}

fn thread_entity(resource: Option<serde_json::Value>) -> (Entity, Record) {
    entity_from_parts("Thread", OsBuilder::thread().build().unwrap(), resource)
}

fn unit_resource() -> Option<serde_json::Value> {
    Some(serde_json::json!({ "definition": {} }))
}

fn schema(
    entities: impl IntoIterator<Item = Entity>,
    records: impl IntoIterator<Item = Record>,
) -> Schema {
    SchemaBuilder::new(ident("App"))
        .with_entities(entities)
        .with_records(records)
        .build()
        .unwrap()
}

fn validate(schema: &Schema) -> Vec<OsError> {
    let report = quent_constraints::validate::<(OsConstraint, ResourceConstraint)>(schema);
    assert!(report.base_constraints.is_ok());
    assert!(report.unregistered_constraints.is_empty());
    match report.results.0 {
        Ok(()) => Vec::new(),
        Err(OsError::Multiple(errors)) => errors,
        Err(error) => vec![error],
    }
}

#[test]
fn valid_optional_platform_ids_pass() {
    let (process, process_id) = process_entity();
    let (thread, thread_id) = thread_entity(unit_resource());

    assert_eq!(process_id.path().to_string(), PROCESS_ID_PATH);
    assert_eq!(thread_id.path().to_string(), THREAD_ID_PATH);
    assert_eq!(
        process_id.field(&ident("linux_id")).unwrap().ty(),
        &DataType::Option(Box::new(DataType::I32))
    );
    assert_eq!(
        thread_id.field(&ident("macos_id")).unwrap().ty(),
        &DataType::Option(Box::new(DataType::U64))
    );
    assert!(validate(&schema([process, thread], [process_id, thread_id])).is_empty());
}

#[test]
fn canonical_id_record_shapes_are_validated() {
    let invalid_process_id = id_record(
        PROCESS_ID_PATH,
        [
            field("linux_id", DataType::Option(Box::new(DataType::U32))),
            field("macos_id", DataType::Option(Box::new(DataType::I32))),
            field("windows_id", DataType::Option(Box::new(DataType::U32))),
        ],
    );
    let invalid_thread_id = id_record(
        THREAD_ID_PATH,
        [
            field("linux_id", DataType::Option(Box::new(DataType::I32))),
            field("macos_id", DataType::Option(Box::new(DataType::U64))),
            field("windows_id", DataType::Option(Box::new(DataType::U32))),
        ],
    );
    let errors = validate(&schema([], [invalid_process_id, invalid_thread_id]));

    assert!(
        errors
            .iter()
            .any(|error| matches!(error, OsError::InvalidRecordFieldType { .. }))
    );
    assert!(errors.iter().any(
        |error| matches!(error, OsError::MissingRecordField { field, .. } if field == "process")
    ));
}

#[test]
fn id_records_must_be_used_by_matching_entities() {
    let process_id = OsBuilder::process().build().unwrap().id;
    let thread_id = OsBuilder::thread().build().unwrap().id;
    let process = EntityBuilder::new(path("Process"))
        .with_event(
            EventBuilder::new(ident("created"), Cardinality::Once)
                .with_field(field("wrong_id", DataType::Record(thread_id_path())))
                .build()
                .unwrap(),
        )
        .with_annotations(annotations(Os::Process, None))
        .build()
        .unwrap();
    let thread = EntityBuilder::new(path("Thread"))
        .with_event(
            EventBuilder::new(ident("created"), Cardinality::Once)
                .with_field(field("wrong_id", DataType::Record(process_id_path())))
                .build()
                .unwrap(),
        )
        .with_annotations(annotations(Os::Thread, unit_resource()))
        .build()
        .unwrap();
    let errors = validate(&schema([process, thread], [process_id, thread_id]));

    assert_eq!(
        errors
            .iter()
            .filter(|error| matches!(error, OsError::WrongNativeIdRecordOwner { .. }))
            .count(),
        2
    );
}

#[test]
fn thread_process_field_must_be_an_entity_reference() {
    let mut fields = OsBuilder::thread()
        .build()
        .unwrap()
        .id
        .fields()
        .filter(|field| field.name() != "process")
        .cloned()
        .collect::<Vec<_>>();
    fields.push(field("process", DataType::Uuid));
    let bad_id = id_record(THREAD_ID_PATH, fields);
    let thread = EntityBuilder::new(path("Thread"))
        .with_event(
            EventBuilder::new(ident("created"), Cardinality::Once)
                .with_field(field("os_id", DataType::Record(thread_id_path())))
                .build()
                .unwrap(),
        )
        .with_annotations(annotations(Os::Thread, unit_resource()))
        .build()
        .unwrap();

    assert!(
        validate(&schema([thread], [bad_id]))
            .iter()
            .any(|error| matches!(error, OsError::InvalidRecordFieldType { .. }))
    );
}

#[test]
fn thread_must_be_a_unit_resource() {
    let (thread, thread_id) = thread_entity(None);
    assert!(
        validate(&schema([thread], [thread_id]))
            .iter()
            .any(|error| matches!(error, OsError::ThreadNotUnitResource { .. }))
    );
}

#[test]
fn os_annotation_on_an_event_is_rejected() {
    let entity = EntityBuilder::new(path("Unmarked"))
        .with_event(
            EventBuilder::new(ident("created"), Cardinality::Once)
                .with_annotations(annotations(Os::Process, None))
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();

    assert!(
        validate(&schema([entity], []))
            .iter()
            .any(|error| matches!(error, OsError::MisplacedAnnotation { .. }))
    );
}
