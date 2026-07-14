// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Lowering from the deserialized [`Model`] to a [`Schema`].
//!
//! Each element lowers independently into a shared diagnostic sink, so one run
//! reports every problem. Constraint and metadata payloads are opaque: they
//! are converted to strings and attached, never interpreted.

use std::collections::HashMap;

use convert_case::{Boundary, Case, Casing};
use indexmap::IndexMap;
use quent_schema::builder::{
    AnnotationsBuilder, EntityBuilder, EventBuilder, RecordBuilder, SchemaBuilder,
};
use quent_schema::{Annotations, DataType, Entity, Field, Identifier, Record, Schema};
use serde_norway::Value;

use crate::ast::{self, Anns, Model, TypeSpec};
use crate::diag::Sink;
use crate::payload::payload;
use crate::types::{RESERVED_TYPE_NAMES, parse_type};

/// Lower `model` to a schema, reporting problems into `sink`.
pub(crate) fn lower(model: &Model, sink: &mut Sink) -> Schema {
    if model.quent != 1 {
        sink.error(
            "quent",
            format!("unsupported format version `{}`", model.quent),
            Some("this quent-yaml reads format 1; write `quent: 1`".to_string()),
        );
    }

    let name = ident(&model.model, "model", sink)
        .unwrap_or_else(|| Identifier::try_new("invalid").expect("placeholder is valid"));

    check_generated_type_collisions(model, sink);

    let records: Vec<Record> = model
        .records
        .iter()
        .map(|(name, record)| record_of(name, record, sink))
        .collect();
    let entities: Vec<Entity> = model
        .entities
        .iter()
        .map(|(name, entity)| entity_of(name, entity, sink))
        .collect();

    SchemaBuilder::new(name)
        .try_with_records(records)
        .expect("record names are unique (deserialized from a map)")
        .try_with_entities(entities)
        .expect("entity names are unique (deserialized from a map)")
        .with_annotations(annotations(
            &model.doc,
            &model.constraints,
            &model.metadata,
            "",
            sink,
        ))
        .build()
}

fn record_of(name: &str, record: &ast::Record, sink: &mut Sink) -> Record {
    let path = format!("records.{name}");
    let id = declared_ident(name, "records", sink);
    let fields = fields_of(&record.fields, &path, sink);
    RecordBuilder::new(id)
        .try_with_fields(fields)
        .expect("field names are unique")
        .with_annotations(annotations(
            &record.doc,
            &record.constraints,
            &record.metadata,
            &path,
            sink,
        ))
        .build()
}

fn entity_of(name: &str, entity: &ast::Entity, sink: &mut Sink) -> Entity {
    let path = format!("entities.{name}");
    let events_path = format!("{path}.events");
    let id = declared_ident(name, "entities", sink);
    let mut collisions = CollisionChecker::new(Case::Pascal);
    let events: Vec<_> = entity
        .events
        .iter()
        .map(|(event_name, event)| {
            collisions.check(event_name, &events_path, sink);
            event_of(event_name, event, &path, sink)
        })
        .collect();
    EntityBuilder::new(id)
        .try_with_events(events)
        .expect("event names are unique")
        .with_annotations(annotations(
            &entity.doc,
            &entity.constraints,
            &entity.metadata,
            &path,
            sink,
        ))
        .build()
}

fn event_of(
    name: &str,
    event: &ast::Event,
    entity_path: &str,
    sink: &mut Sink,
) -> quent_schema::Event {
    let events_path = format!("{entity_path}.events");
    let path = format!("{events_path}.{name}");
    let id = declared_ident(name, &events_path, sink);
    match event {
        ast::Event::OneLiner(card) => EventBuilder::new(id, (*card).into()).build(),
        ast::Event::Body(body) => {
            let (card, payload_key) = match (&body.once, &body.multi) {
                (Some(_), Some(_)) => {
                    sink.error(
                        &path,
                        "event declares both `once` and `multi`",
                        Some("keep exactly one".to_string()),
                    );
                    (ast::Cardinality::Once, "once")
                }
                (Some(_), None) => (ast::Cardinality::Once, "once"),
                (None, Some(_)) => (ast::Cardinality::Multi, "multi"),
                (None, None) => {
                    sink.error(
                        &path,
                        "event must declare a cardinality",
                        Some("add `once:` or `multi:`, or write `name: once`".to_string()),
                    );
                    (ast::Cardinality::Once, "once")
                }
            };
            let payload = match card {
                ast::Cardinality::Once => &body.once,
                ast::Cardinality::Multi => &body.multi,
            };
            let fields = match payload.as_ref().and_then(|p| p.as_ref()) {
                Some(map) => fields_of(map, &format!("{path}.{payload_key}"), sink),
                None => Vec::new(),
            };
            EventBuilder::new(id, card.into())
                .try_with_fields(fields)
                .expect("field names are unique")
                .with_annotations(annotations(
                    &body.doc,
                    &body.constraints,
                    &body.metadata,
                    &path,
                    sink,
                ))
                .build()
        }
    }
}

fn fields_of(fields: &IndexMap<String, ast::Field>, path: &str, sink: &mut Sink) -> Vec<Field> {
    let mut collisions = CollisionChecker::new(Case::Snake);
    fields
        .iter()
        .filter_map(|(name, field)| {
            let field_path = format!("{path}.{name}");
            let id = declared_ident(name, path, sink);
            collisions.check(name, path, sink);
            field_of(id, field, &field_path, sink)
        })
        .collect()
}

fn field_of(id: Identifier, field: &ast::Field, path: &str, sink: &mut Sink) -> Option<Field> {
    let (ty, ann) = match field {
        ast::Field::Short(text) => (type_expr(text, path, sink)?, Annotations::default()),
        ast::Field::Full(body) => {
            let ty = type_spec(&body.r#type, path, sink)?;
            let ann = annotations(&body.doc, &body.constraints, &body.metadata, path, sink);
            (ty, ann)
        }
    };
    Some(Field::new(id, ty, ann))
}

fn type_spec(spec: &TypeSpec, path: &str, sink: &mut Sink) -> Option<DataType> {
    match spec {
        TypeSpec::Expr(text) => type_expr(text, path, sink),
        TypeSpec::Ref(r) => {
            if !matches!(r.r#ref, Value::Null) {
                sink.error(
                    path,
                    "`ref` takes no value; reference targets are not supported yet",
                    Some("write `ref:` and leave it empty".to_string()),
                );
                return None;
            }
            let data = match &r.data {
                Some(inner) => Some(Box::new(type_spec(inner, path, sink)?)),
                None => None,
            };
            let mut builder = AnnotationsBuilder::new();
            add_anns(&mut builder, &r.constraints, &r.metadata, path, sink);
            Some(DataType::EntityRef {
                data,
                annotations: builder.build(),
            })
        }
    }
}

fn type_expr(text: &str, path: &str, sink: &mut Sink) -> Option<DataType> {
    match parse_type(text) {
        Ok(ty) => Some(ty),
        Err(reason) => {
            sink.error(path, format!("invalid type `{text}`: {reason}"), None);
            None
        }
    }
}

fn annotations(
    doc: &Option<String>,
    constraints: &Anns,
    metadata: &Anns,
    path: &str,
    sink: &mut Sink,
) -> Annotations {
    let mut builder = AnnotationsBuilder::new();
    if let Some(doc) = doc {
        builder.set_docs(doc.clone());
    }
    add_anns(&mut builder, constraints, metadata, path, sink);
    builder.build()
}

fn add_anns(
    builder: &mut AnnotationsBuilder,
    constraints: &Anns,
    metadata: &Anns,
    path: &str,
    sink: &mut Sink,
) {
    for (name, value) in constraints {
        if let Some(data) = ann_payload(name, value, "constraint", path, sink)
            && let Err(e) = builder.try_insert_constraint(name, data)
        {
            sink.error(path, e.to_string(), None);
        }
    }
    for (name, value) in metadata {
        if let Some(data) = ann_payload(name, value, "metadata", path, sink)
            && let Err(e) = builder.try_insert_metadata(name, data)
        {
            sink.error(path, e.to_string(), None);
        }
    }
}

/// Convert one annotation payload, reporting on failure. The outer `Option`
/// signals whether to attach it; the inner is the constraint/metadata data.
fn ann_payload(
    name: &str,
    value: &Value,
    kind: &str,
    path: &str,
    sink: &mut Sink,
) -> Option<Option<String>> {
    if name.is_empty() {
        sink.error(path, format!("{kind} name must not be empty"), None);
        return None;
    }
    match payload(value) {
        Ok(data) => Some(data),
        Err(reason) => {
            sink.error(path, format!("`{name}`: {reason}"), None);
            None
        }
    }
}

/// Validate a declared name, reporting under `path` and falling back to a
/// placeholder so lowering continues.
fn declared_ident(name: &str, path: &str, sink: &mut Sink) -> Identifier {
    if RESERVED_TYPE_NAMES.contains(&name) {
        sink.error(
            path,
            format!("`{name}` is a reserved type name"),
            Some("pick a different name".to_string()),
        );
    }
    ident(name, path, sink).unwrap_or_else(|| Identifier::try_new("invalid").expect("valid"))
}

fn ident(name: &str, path: &str, sink: &mut Sink) -> Option<Identifier> {
    match Identifier::try_new(name) {
        Ok(id) => Some(id),
        Err(e) => {
            sink.error(path, format!("invalid name `{name}`: {e}"), None);
            None
        }
    }
}

/// Records generate `Pascal(name)` and entities `Pascal(name)Event`; a clash
/// would emit two identical Rust types.
fn check_generated_type_collisions(model: &Model, sink: &mut Sink) {
    let record_types: HashMap<String, &String> = model
        .records
        .keys()
        .map(|name| (cased(name, Case::Pascal), name))
        .collect();
    for entity in model.entities.keys() {
        let generated = format!("{}Event", cased(entity, Case::Pascal));
        if let Some(record) = record_types.get(&generated) {
            sink.error(
                &format!("entities.{entity}"),
                format!(
                    "entity `{entity}` and record `{record}` both generate the type `{generated}`"
                ),
                Some("rename one of them".to_string()),
            );
        }
    }
    let mut records = CollisionChecker::new(Case::Pascal);
    for name in model.records.keys() {
        records.check(name, "records", sink);
    }
    let mut entities = CollisionChecker::new(Case::Pascal);
    for name in model.entities.keys() {
        entities.check(name, "entities", sink);
    }
}

/// Detects sibling names that converge under codegen's case conversions.
struct CollisionChecker {
    case: Case<'static>,
    seen: HashMap<String, String>,
}

impl CollisionChecker {
    fn new(case: Case<'static>) -> Self {
        Self {
            case,
            seen: HashMap::new(),
        }
    }

    fn check(&mut self, name: &str, path: &str, sink: &mut Sink) {
        let converted = cased(name, self.case);
        match self.seen.get(&converted) {
            Some(first) if first != name => sink.error(
                path,
                format!("`{name}` and `{first}` both generate the identifier `{converted}`"),
                Some("rename one of them".to_string()),
            ),
            Some(_) => {}
            None => {
                self.seen.insert(converted, name.to_string());
            }
        }
    }
}

/// Case-convert keeping digit boundaries together and suffixing un-rawable
/// keywords, as `quent-instrumentation-build` does.
fn cased(name: &str, case: Case) -> String {
    const KEEP_DIGITS: &[Boundary] = &[
        Boundary::LOWER_DIGIT,
        Boundary::UPPER_DIGIT,
        Boundary::DIGIT_LOWER,
        Boundary::DIGIT_UPPER,
    ];
    const NON_RAW: &[&str] = &["crate", "self", "super", "Self"];
    let mut out = name.without_boundaries(KEEP_DIGITS).to_case(case);
    if NON_RAW.contains(&out.as_str()) {
        out.push('_');
    }
    out
}
