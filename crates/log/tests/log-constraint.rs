// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_constraints::Constraint as _;
use quent_log::{
    LevelDecl, LogConstraint, LogDefinition, LogDefinitionError, LogEntityBuilder, LogError,
    MAX_LEVELS,
};
use quent_schema::{
    Annotations, Cardinality, DataType, Entity, Event, Field, Schema,
    builder::{AnnotationsBuilder, EntityBuilder, EventBuilder, SchemaBuilder},
    test_utils::{field, ident, path},
};

fn annotations(data: Option<String>) -> Annotations {
    AnnotationsBuilder::new()
        .with_constraint(LogConstraint::NAME, data)
        .build()
        .unwrap()
}

fn raw_definition(levels: &[&str]) -> String {
    serde_json::json!({ "levels": levels }).to_string()
}

fn event(name: &str, cardinality: Cardinality, fields: Vec<Field>) -> Event {
    EventBuilder::new(ident(name), cardinality)
        .with_fields(fields)
        .build()
        .unwrap()
}

fn required_fields() -> Vec<Field> {
    vec![field("message", DataType::String)]
}

fn schema(events: Vec<Event>, data: Option<String>) -> Schema {
    let entity = EntityBuilder::new(path("AppLog"))
        .with_events(events)
        .with_annotations(annotations(data))
        .build()
        .unwrap();
    SchemaBuilder::new(ident("Application"))
        .with_entity(entity)
        .build()
        .unwrap()
}

fn validate(schema: &Schema) -> Vec<LogError> {
    match quent_constraints::validate::<(LogConstraint,)>(schema)
        .results
        .0
    {
        Ok(()) => Vec::new(),
        Err(LogError::Multiple(errors)) => errors,
        Err(error) => vec![error],
    }
}

#[test]
fn builder_creates_ranked_repeatable_events() {
    let entity = LogEntityBuilder::new(path("AppLog"))
        .with_attributes([
            field("target", DataType::String),
            field("thread_name", DataType::Option(Box::new(DataType::String))),
        ])
        .with_levels(["trace", "debug", "info"].map(|name| LevelDecl {
            name: ident(name),
            annotations: Annotations::default(),
            attributes: Vec::new(),
        }))
        .build()
        .unwrap();

    let definition = LogDefinition::from_entity(&entity).unwrap().unwrap();
    let levels: Vec<_> = definition
        .levels()
        .map(|level| (level.name().to_string(), level.rank()))
        .collect();
    assert_eq!(
        levels,
        [("trace".into(), 0), ("debug".into(), 1), ("info".into(), 2)]
    );
    for event in entity.events() {
        assert_eq!(event.cardinality(), Cardinality::Multi);
        assert_eq!(
            event.field(&ident("message")).unwrap().ty(),
            &DataType::String
        );
        assert!(event.field(&ident("target")).is_some());
        assert!(event.field(&ident("thread_name")).is_some());
    }
}

#[test]
fn logging_context_fields_are_ordinary_additional_fields() {
    let schema = schema(
        vec![event(
            "warning",
            Cardinality::Multi,
            vec![
                field("message", DataType::String),
                field("target", DataType::U64),
                field("file", DataType::Bool),
                field("line", DataType::String),
                field("module", DataType::DynamicRecord),
            ],
        )],
        Some(raw_definition(&["warning"])),
    );
    assert!(validate(&schema).is_empty());
}

#[test]
fn malformed_level_definitions_are_rejected() {
    for (data, expected) in [
        (None, "missing"),
        (Some("not json".to_string()), "decode"),
        (
            Some(r#"{"levels":["info"],"unknown":true}"#.to_string()),
            "unknown field",
        ),
        (Some(raw_definition(&[])), "must not be empty"),
        (Some(raw_definition(&["info", "info"])), "more than once"),
        (Some(raw_definition(&["not-valid"])), "invalid name"),
    ] {
        let schema = schema(
            vec![event("info", Cardinality::Multi, required_fields())],
            data,
        );
        assert!(validate(&schema)[0].to_string().contains(expected));
    }
}

#[test]
fn level_limit_accepts_256_and_rejects_257() {
    let levels: Vec<_> = (0..MAX_LEVELS)
        .map(|rank| ident(&format!("level{rank}")))
        .collect();
    assert!(LogDefinition::new(levels.clone()).is_ok());
    let mut too_many = levels;
    too_many.push(ident("overflow"));
    assert!(matches!(
        LogDefinition::new(too_many),
        Err(LogDefinitionError::TooManyLevels { count: 257, .. })
    ));
}

#[test]
fn declared_levels_must_have_events() {
    let schema = schema(
        vec![event("info", Cardinality::Multi, required_fields())],
        Some(raw_definition(&["info", "error"])),
    );
    assert!(matches!(
        validate(&schema).as_slice(),
        [LogError::MissingLevelEvent { level, .. }] if level == "error"
    ));
}

#[test]
fn non_level_events_are_allowed() {
    let schema = schema(
        vec![
            event("initialized", Cardinality::Once, Vec::new()),
            event("info", Cardinality::Multi, required_fields()),
        ],
        Some(raw_definition(&["info"])),
    );
    assert!(validate(&schema).is_empty());
}

#[test]
fn level_events_must_be_repeatable() {
    let schema = schema(
        vec![event("info", Cardinality::Once, required_fields())],
        Some(raw_definition(&["info"])),
    );
    assert!(matches!(
        validate(&schema).as_slice(),
        [LogError::CardinalityMismatch { .. }]
    ));
}

#[test]
fn message_field_must_exist_with_exact_type() {
    let schema = schema(
        vec![
            event("info", Cardinality::Multi, Vec::new()),
            event(
                "error",
                Cardinality::Multi,
                vec![field("message", DataType::U32)],
            ),
        ],
        Some(raw_definition(&["info", "error"])),
    );
    let errors = validate(&schema);
    assert!(
        errors.iter().any(
            |error| matches!(error, LogError::MissingField { field, .. } if field == "message")
        )
    );
    assert!(errors.iter().any(
        |error| matches!(error, LogError::IncorrectFieldType { field, .. } if field == "message")
    ));
}

#[test]
fn misplaced_constraint_is_rejected() {
    let event = EventBuilder::new(ident("info"), Cardinality::Multi)
        .with_field(field("message", DataType::String))
        .with_annotations(annotations(Some(raw_definition(&["info"]))))
        .build()
        .unwrap();
    let entity = EntityBuilder::new(path("AppLog"))
        .with_event(event)
        .build()
        .unwrap();
    let schema = SchemaBuilder::new(ident("Application"))
        .with_entity(entity)
        .build()
        .unwrap();
    assert!(matches!(
        validate(&schema).as_slice(),
        [LogError::Misplaced { .. }]
    ));
}

#[test]
fn unannotated_entities_are_ignored() {
    let entity: Entity = EntityBuilder::new(path("Worker"))
        .with_event(event("worked", Cardinality::Once, Vec::new()))
        .build()
        .unwrap();
    let schema = SchemaBuilder::new(ident("Application"))
        .with_entity(entity)
        .build()
        .unwrap();
    assert!(validate(&schema).is_empty());
}
