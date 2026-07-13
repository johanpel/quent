// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Lowering from the spanned YAML tree to a [`Schema`].
//!
//! Every element lowers independently and reports into one shared sink, so a
//! single run surfaces all problems. Cross-declaration checks (record
//! references, entity reference targets) run deferred once every declaration
//! is known.

use std::collections::HashMap;

use convert_case::{Boundary, Case, Casing};
use quent_schema::builder::{
    AnnotationsBuilder, EntityBuilder, EventBuilder, RecordBuilder, SchemaBuilder,
};
use quent_schema::{
    Annotations, Cardinality, DataType, Entity, Event, Field, Identifier, Record, Schema,
};
use saphyr_parser::Span;

use crate::diag::{Sink, suggest};
use crate::json;
use crate::tree::{Kind, Node};
use crate::types::{self, DeferredRefs, MAX_TYPE_DEPTH, RESERVED_TYPE_NAMES};
use crate::walk::{MapView, expect_name, expect_string};

/// Spans of schema elements, for locating validation-stage errors.
pub(crate) struct SourceMap {
    /// Span of the `model` value.
    pub(crate) model_span: Span,
    /// Record name to the span of its declaration key.
    pub(crate) record_spans: HashMap<String, Span>,
    /// First occurrence of each constraint name.
    pub(crate) constraint_first: HashMap<String, Span>,
}

/// Lower the document root to a schema.
///
/// Returns `None` only for a non-mapping root; all other failures keep
/// lowering and report through the sink (the schema is then discarded by the
/// caller).
pub(crate) fn lower(root: &Node, sink: &mut Sink) -> Option<(Schema, SourceMap)> {
    if !matches!(root.kind, Kind::Map(_)) {
        sink.error(
            root.span,
            "",
            "expected a mapping at the document root",
            None,
        );
        return None;
    }
    let mut lower = Lower {
        sink,
        refs: DeferredRefs::default(),
        map: SourceMap {
            model_span: root.span,
            record_spans: HashMap::new(),
            constraint_first: HashMap::new(),
        },
        record_names: Vec::new(),
        entity_names: Vec::new(),
    };
    let schema = lower.document(root);
    lower.check_deferred_refs();
    let map = lower.map;
    Some((schema, map))
}

struct Lower<'s> {
    sink: &'s mut Sink,
    refs: DeferredRefs,
    map: SourceMap,
    record_names: Vec<String>,
    entity_names: Vec<String>,
}

impl Lower<'_> {
    fn document(&mut self, root: &Node) -> Schema {
        let mut view = MapView::new(root, self.sink, "");

        match view.take("quent") {
            None => self.sink.error(
                root.span,
                "",
                "missing the `quent` format version key",
                Some("add `quent: 1` at the top of the file".to_string()),
            ),
            Some((node, _)) => self.format_version(node),
        }

        let name = match view.take("model") {
            Some((node, _)) => {
                self.map.model_span = node.span;
                expect_name(node, self.sink, "model")
                    .and_then(|name| self.identifier(&name, node.span, "model"))
            }
            None => {
                self.sink.error(
                    root.span,
                    "",
                    "missing the `model` key naming the model",
                    None,
                );
                None
            }
        };
        // A placeholder keeps lowering going when `model` is broken; the
        // schema is discarded because the sink already holds an error.
        let name = name.unwrap_or_else(|| {
            Identifier::try_new("invalid").expect("placeholder is a valid identifier")
        });

        let annotations = self.annotations(&mut view, "");

        // Names must all be known before deferred reference checks, so
        // collect the declarations before lowering bodies.
        let records: Vec<(String, Span, &Node)> = match view.take("records") {
            Some((node, _)) => self.declarations(node, "records"),
            None => Vec::new(),
        };
        let entities: Vec<(String, Span, &Node)> = match view.take("entities") {
            Some((node, _)) => self.declarations(node, "entities"),
            None => Vec::new(),
        };
        self.record_names = records.iter().map(|(n, ..)| n.clone()).collect();
        self.entity_names = entities.iter().map(|(n, ..)| n.clone()).collect();

        // Cross-collection codegen collision: records generate
        // `Pascal(name)`, entities `Pascal(name)Event`.
        let record_types: HashMap<String, (&String, Span)> = records
            .iter()
            .map(|(name, span, _)| (cased(name, Case::Pascal), (name, *span)))
            .collect();
        for (name, span, _) in &entities {
            let generated = format!("{}Event", cased(name, Case::Pascal));
            if let Some((record, record_span)) = record_types.get(&generated) {
                self.sink.error(
                    *span,
                    &format!("entities.{name}"),
                    format!(
                        "entity `{name}` and record `{record}` (line {}) both generate the type `{generated}`",
                        record_span.start.line()
                    ),
                    Some("rename one of them".to_string()),
                );
            }
        }

        view.finish_strict(self.sink, "");

        let records: Vec<Record> = records
            .into_iter()
            .map(|(name, span, node)| self.record(&name, span, node))
            .collect();
        let entities: Vec<Entity> = entities
            .into_iter()
            .map(|(name, _, node)| self.entity(&name, node))
            .collect();

        // Duplicates cannot reach the builder: the map views deduplicate
        // sibling names, so these errors are unreachable.
        SchemaBuilder::new(name)
            .try_with_records(records)
            .expect("record names are deduplicated")
            .try_with_entities(entities)
            .expect("entity names are deduplicated")
            .with_annotations(annotations)
            .build()
    }

    fn format_version(&mut self, node: &Node) {
        match node.scalar() {
            // Style is deliberately not checked: `quent: '1'` means the same.
            Some(("1", _)) => {}
            Some((text, _)) => self.sink.error(
                node.span,
                "quent",
                format!("unsupported format version `{text}`"),
                Some("this quent-yaml reads format 1; write `quent: 1`".to_string()),
            ),
            None => self
                .sink
                .error(node.span, "quent", "expected the format version `1`", None),
        }
    }

    /// Read a name-keyed declaration collection, checking names and
    /// generated-code collisions but not lowering bodies yet.
    fn declarations<'t>(&mut self, node: &'t Node, path: &str) -> Vec<(String, Span, &'t Node)> {
        let view = MapView::new(node, self.sink, path);
        let mut collisions = CollisionChecker::new(Case::Pascal);
        let mut out = Vec::new();
        for (name, key_span, value) in view.into_entries() {
            let item_path = format!("{path}.{name}");
            if self.identifier(name, key_span, &item_path).is_none() {
                continue;
            }
            if RESERVED_TYPE_NAMES.contains(&name) {
                self.sink.error(
                    key_span,
                    &item_path,
                    format!("`{name}` is a reserved type name"),
                    Some(
                        "pick a different name; this one has a fixed meaning in type expressions"
                            .to_string(),
                    ),
                );
                continue;
            }
            collisions.check(name, key_span, &item_path, self.sink);
            out.push((name.to_string(), key_span, value));
        }
        out
    }

    fn record(&mut self, name: &str, key_span: Span, node: &Node) -> Record {
        let path = format!("records.{name}");
        self.map.record_spans.insert(name.to_string(), key_span);
        let mut view = MapView::new(node, self.sink, &path);
        let annotations = self.annotations(&mut view, &path);
        let fields = match view.take("fields") {
            Some((fields_node, _)) => self.fields(fields_node, &path),
            None => Vec::new(),
        };
        view.finish_strict(self.sink, &path);
        let ident = Identifier::try_new(name).expect("declaration names are pre-checked");
        RecordBuilder::new(ident)
            .try_with_fields(fields)
            .expect("field names are deduplicated")
            .with_annotations(annotations)
            .build()
    }

    fn entity(&mut self, name: &str, node: &Node) -> Entity {
        let path = format!("entities.{name}");
        let mut view = MapView::new(node, self.sink, &path);
        let annotations = self.annotations(&mut view, &path);
        let events = match view.take("events") {
            Some((events_node, _)) => self.events(events_node, &path),
            None => Vec::new(),
        };
        view.finish_strict(self.sink, &path);
        let ident = Identifier::try_new(name).expect("declaration names are pre-checked");
        EntityBuilder::new(ident)
            .try_with_events(events)
            .expect("event names are deduplicated")
            .with_annotations(annotations)
            .build()
    }

    fn events(&mut self, node: &Node, path: &str) -> Vec<Event> {
        let events_path = format!("{path}.events");
        let view = MapView::new(node, self.sink, &events_path);
        let mut collisions = CollisionChecker::new(Case::Pascal);
        let mut out = Vec::new();
        for (name, key_span, value) in view.into_entries() {
            let event_path = format!("{events_path}.{name}");
            let Some(ident) = self.identifier(name, key_span, &event_path) else {
                continue;
            };
            collisions.check(name, key_span, &event_path, self.sink);
            if let Some(event) = self.event(ident, value, &event_path) {
                out.push(event);
            }
        }
        out
    }

    fn event(&mut self, name: Identifier, node: &Node, path: &str) -> Option<Event> {
        const CARDINALITY_HELP: &str =
            "declare the payload under `once:` or `multi:`, or write the one-liner `name: once`";
        if node.is_null() {
            self.sink.error(
                node.span,
                path,
                "event must declare a cardinality",
                Some(CARDINALITY_HELP.to_string()),
            );
            return None;
        }
        if let Some((text, _)) = node.scalar() {
            let cardinality = match text {
                "once" => Cardinality::Once,
                "multi" => Cardinality::Multi,
                _ => {
                    self.sink.error(
                        node.span,
                        path,
                        format!("expected `once` or `multi`, found `{text}`"),
                        Some(CARDINALITY_HELP.to_string()),
                    );
                    return None;
                }
            };
            return Some(EventBuilder::new(name, cardinality).build());
        }

        let mut view = MapView::new(node, self.sink, path);
        let annotations = self.annotations(&mut view, path);
        let once = view.take("once");
        let multi = view.take("multi");
        view.finish_strict(self.sink, path);
        let (cardinality, payload_key, payload) = match (once, multi) {
            (Some(_), Some((_, span))) => {
                self.sink.error(
                    span,
                    path,
                    "event declares both `once` and `multi`",
                    Some("keep exactly one".to_string()),
                );
                return None;
            }
            (Some((node, _)), None) => (Cardinality::Once, "once", node),
            (None, Some((node, _))) => (Cardinality::Multi, "multi", node),
            (None, None) => {
                self.sink.error(
                    node.span,
                    path,
                    "event must declare a cardinality",
                    Some(CARDINALITY_HELP.to_string()),
                );
                return None;
            }
        };
        let fields = self.fields(payload, &format!("{path}.{payload_key}"));
        Some(
            EventBuilder::new(name, cardinality)
                .try_with_fields(fields)
                .expect("field names are deduplicated")
                .with_annotations(annotations)
                .build(),
        )
    }

    /// Lower a `fields:` mapping or event payload mapping.
    fn fields(&mut self, node: &Node, path: &str) -> Vec<Field> {
        let view = MapView::new(node, self.sink, path);
        let mut collisions = CollisionChecker::new(Case::Snake);
        let mut out = Vec::new();
        for (name, key_span, value) in view.into_entries() {
            let field_path = format!("{path}.{name}");
            let Some(ident) = self.identifier(name, key_span, &field_path) else {
                continue;
            };
            collisions.check(name, key_span, &field_path, self.sink);
            if let Some(field) = self.field(ident, value, &field_path) {
                if types::wrapper_depth(field.ty()) > MAX_TYPE_DEPTH {
                    self.sink.error(
                        value.span,
                        &field_path,
                        format!("type nests deeper than {MAX_TYPE_DEPTH} wrappers"),
                        None,
                    );
                    continue;
                }
                out.push(field);
            }
        }
        out
    }

    fn field(&mut self, name: Identifier, node: &Node, path: &str) -> Option<Field> {
        if let Some((text, _)) = node.scalar()
            && !node.is_null()
        {
            let ty = types::parse_type_expr(text, node.span, path, self.sink, &mut self.refs)?;
            return Some(Field::new(name, ty, Annotations::default()));
        }
        if matches!(node.kind, Kind::Map(_)) {
            let mut view = MapView::new(node, self.sink, path);
            let ty = match view.take("type") {
                Some((type_node, _)) => self.type_value(type_node, path),
                None => {
                    self.sink
                        .error(node.span, path, "field needs a `type`", None);
                    None
                }
            };
            let annotations = self.annotations(&mut view, path);
            view.finish_strict(self.sink, path);
            return Some(Field::new(name, ty?, annotations));
        }
        self.sink.error(
            node.span,
            path,
            "expected a type expression or a field mapping",
            Some("write `name: <type>` or `name: { type: <type>, doc: ... }`".to_string()),
        );
        None
    }

    /// The value of a `type:` or `data:` key: a type expression or a
    /// structured reference mapping.
    fn type_value(&mut self, node: &Node, path: &str) -> Option<DataType> {
        if let Some((text, _)) = node.scalar()
            && !node.is_null()
        {
            return types::parse_type_expr(text, node.span, path, self.sink, &mut self.refs);
        }
        if matches!(node.kind, Kind::Map(_)) {
            return self.ref_map(node, path);
        }
        self.sink.error(
            node.span,
            path,
            "expected a type expression or a `{ ref: ... }` mapping",
            None,
        );
        None
    }

    /// The structured entity reference form:
    /// `{ ref: , data: <type>, constraints: ..., metadata: ... }`.
    ///
    /// `ref:` is the form's marker and must be empty: its value is kept free
    /// for later syntax extensions (reference targets).
    fn ref_map(&mut self, node: &Node, path: &str) -> Option<DataType> {
        let mut view = MapView::new(node, self.sink, path);

        match view.take("ref") {
            Some((ref_node, _)) if ref_node.is_null() => {}
            Some((ref_node, _)) => {
                self.sink.error(
                    ref_node.span,
                    path,
                    "`ref` takes no value; reference targets are not supported yet",
                    Some("write `ref:` and leave it empty".to_string()),
                );
                return None;
            }
            None => {
                self.sink.error(
                    node.span,
                    path,
                    "reference mapping needs a `ref` key",
                    Some("write `ref:` to mark an entity reference".to_string()),
                );
                return None;
            }
        }

        let data = match view.take("data") {
            Some((data_node, _)) => Some(Box::new(self.type_value(data_node, path)?)),
            None => None,
        };

        let mut builder = AnnotationsBuilder::new();
        if let Some((constraints_node, _)) = view.take("constraints") {
            let constraints = MapView::new(constraints_node, self.sink, path);
            for (name, key_span, value) in constraints.into_entries() {
                let Some(data) = json::payload(value, self.sink, path) else {
                    continue;
                };
                self.add_constraint(&mut builder, name, data, key_span, path);
            }
        }
        if let Some((metadata_node, _)) = view.take("metadata") {
            self.metadata_entries(&mut builder, metadata_node, path);
        }
        view.finish_strict(self.sink, path);

        Some(DataType::EntityRef {
            data,
            annotations: builder.build(),
        })
    }

    /// Lower `doc`/`constraints`/`metadata` keys from `view`.
    fn annotations(&mut self, view: &mut MapView<'_>, path: &str) -> Annotations {
        let mut builder = AnnotationsBuilder::new();
        if let Some((doc_node, _)) = view.take("doc")
            && !doc_node.is_null()
            && let Some(doc) = expect_string(doc_node, self.sink, path)
        {
            builder.set_docs(doc);
        }
        if let Some((constraints_node, _)) = view.take("constraints") {
            let constraints = MapView::new(constraints_node, self.sink, path);
            for (name, key_span, value) in constraints.into_entries() {
                let Some(data) = json::payload(value, self.sink, path) else {
                    continue;
                };
                self.add_constraint(&mut builder, name, data, key_span, path);
            }
        }
        if let Some((metadata_node, _)) = view.take("metadata") {
            self.metadata_entries(&mut builder, metadata_node, path);
        }
        builder.build()
    }

    fn add_constraint(
        &mut self,
        builder: &mut AnnotationsBuilder,
        name: &str,
        data: Option<String>,
        key_span: Span,
        path: &str,
    ) {
        if name.is_empty() {
            self.sink
                .error(key_span, path, "constraint name must not be empty", None);
            return;
        }
        self.map
            .constraint_first
            .entry(name.to_string())
            .or_insert(key_span);
        // Duplicates cannot occur (the map view deduplicates names), but the
        // builder keeps its state on error, so map it instead of expecting.
        if let Err(e) = builder.try_insert_constraint(name, data) {
            self.sink.error(key_span, path, e.to_string(), None);
        }
    }

    fn metadata_entries(&mut self, builder: &mut AnnotationsBuilder, node: &Node, path: &str) {
        let view = MapView::new(node, self.sink, path);
        for (name, key_span, value) in view.into_entries() {
            if name.is_empty() {
                self.sink
                    .error(key_span, path, "metadata name must not be empty", None);
                continue;
            }
            let Some(data) = json::payload(value, self.sink, path) else {
                continue;
            };
            if let Err(e) = builder.try_insert_metadata(name, data) {
                self.sink.error(key_span, path, e.to_string(), None);
            }
        }
    }

    fn identifier(&mut self, name: &str, span: Span, path: &str) -> Option<Identifier> {
        match Identifier::try_new(name) {
            Ok(ident) => Some(ident),
            Err(e) => {
                self.sink
                    .error(span, path, format!("invalid name `{name}`: {e}"), None);
                None
            }
        }
    }

    /// Check collected record references against the declared names.
    fn check_deferred_refs(&mut self) {
        let refs = std::mem::take(&mut self.refs);
        // Membership sets; the ordered Vecs stay the suggestion candidates so
        // did-you-mean results are deterministic.
        let records: std::collections::HashSet<&str> =
            self.record_names.iter().map(String::as_str).collect();
        let entities: std::collections::HashSet<&str> =
            self.entity_names.iter().map(String::as_str).collect();
        for (name, span, path) in refs.records {
            if records.contains(name.as_str()) {
                continue;
            }
            if entities.contains(name.as_str()) {
                self.sink.error(
                    span,
                    &path,
                    format!("`{name}` is an entity, not a record"),
                    Some("write `Ref` to reference an entity".to_string()),
                );
                continue;
            }
            let candidates = self
                .record_names
                .iter()
                .map(String::as_str)
                .chain(RESERVED_TYPE_NAMES.iter().copied());
            let help = suggest(&name, candidates).map(|s| format!("did you mean `{s}`?"));
            self.sink.error(
                span,
                &path,
                format!("unknown type or record `{name}`"),
                help,
            );
        }
    }
}

/// Detects sibling names that converge under the case conversions
/// `quent-instrumentation-build` applies, which would generate colliding Rust
/// identifiers.
struct CollisionChecker {
    case: Case<'static>,
    seen: HashMap<String, (String, Span)>,
}

impl CollisionChecker {
    fn new(case: Case<'static>) -> Self {
        Self {
            case,
            seen: HashMap::new(),
        }
    }

    fn check(&mut self, name: &str, span: Span, path: &str, sink: &mut Sink) {
        let converted = cased(name, self.case);
        match self.seen.get(&converted) {
            Some((first, first_span)) if first != name => {
                sink.error(
                    span,
                    path,
                    format!(
                        "`{name}` and `{first}` (line {}) both generate the identifier `{converted}`",
                        first_span.start.line()
                    ),
                    Some("rename one of them".to_string()),
                );
            }
            Some(_) => {}
            None => {
                self.seen.insert(converted, (name.to_string(), span));
            }
        }
    }
}

/// Case-convert keeping digit boundaries together, as
/// `quent-instrumentation-build` does.
pub(crate) fn to_case_digits(name: &str, case: Case) -> String {
    const KEEP_DIGITS: &[Boundary] = &[
        Boundary::LOWER_DIGIT,
        Boundary::UPPER_DIGIT,
        Boundary::DIGIT_LOWER,
        Boundary::DIGIT_UPPER,
    ];
    name.without_boundaries(KEEP_DIGITS).to_case(case)
}

/// The identifier `quent-instrumentation-build` generates for `name`:
/// case-converted, with un-rawable keywords suffixed with `_`.
fn cased(name: &str, case: Case) -> String {
    const NON_RAW: &[&str] = &["crate", "self", "super", "Self"];
    let mut out = to_case_digits(name, case);
    if NON_RAW.contains(&out.as_str()) {
        out.push('_');
    }
    out
}
