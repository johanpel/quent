use rustc_hash::FxHashMap as HashMap;

use crate::{Annotations, DataType, Entity, Event, Field, Identifier, Record, Schema};

/// Reference to a schema element.
#[derive(Clone, Copy)]
pub enum Element<'s> {
    Schema(&'s Schema),
    Annotations(&'s Annotations),
    Entity(&'s Entity),
    Event(&'s Event),
    Field(&'s Field),
    Record(&'s Record),
    DataType(&'s DataType),
}

/// Path from the schema root down to the element currently being visited.
pub struct Cursor<'s>(Vec<Element<'s>>);

impl<'s> Cursor<'s> {
    pub fn new(schema: &'s Schema) -> Self {
        // Capacity hint, not a bound: 5 is a typical leaf depth, e.g.
        // Schema -> Entity -> Event -> Field -> Annotations. Nested data types
        // can go deeper and the vec grows as needed.
        let mut path = Vec::with_capacity(5);
        path.push(Element::Schema(schema));
        Self(path)
    }
    /// The element currently being visited.
    pub fn current(&self) -> Element<'s> {
        // unwrap ok: inner vec can't be constructed without a schema
        self.0.last().copied().unwrap()
    }
    /// The parent of the current element, if any.
    pub fn previous(&self) -> Option<Element<'s>> {
        self.0.iter().rev().nth(1).copied()
    }
    /// The schema being walked.
    pub fn root(&self) -> &'s Schema {
        match self.0.first() {
            Some(Element::Schema(schema)) => schema,
            _ => unreachable!("cursor is constructed with the schema as its root"),
        }
    }
    /// The full path, from the schema root to the current element.
    pub fn elements(&self) -> &[Element<'s>] {
        &self.0
    }
    fn enter(&mut self, element: Element<'s>) {
        self.0.push(element);
    }
    fn leave(&mut self) {
        self.0.pop();
    }
}

pub trait Visitor {
    type Output;
    fn visit(&mut self, cursor: &Cursor, index: &SchemaIndex);
    fn finish(self) -> Self::Output;
}

/// Reason a [`SchemaIndex`] lookup failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupError {
    /// No element is declared under the queried name.
    Missing(Identifier),
    /// The name is declared more than once in its scope, so the lookup is
    /// ambiguous. Reported only for the duplicated name; other names resolve.
    Duplicate(Identifier),
}

impl std::fmt::Display for LookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "no element named \"{name}\""),
            Self::Duplicate(name) => write!(f, "name \"{name}\" is declared more than once"),
        }
    }
}

impl std::error::Error for LookupError {}

/// Lookups into a [`Schema`] by name.
pub struct SchemaIndex<'s> {
    entities: HashMap<&'s Identifier, Result<IndexedEntity<'s>, LookupError>>,
    records: HashMap<&'s Identifier, Result<IndexedRecord<'s>, LookupError>>,
}

struct IndexedEntity<'s> {
    entity: &'s Entity,
    events: HashMap<&'s Identifier, Result<IndexedEvent<'s>, LookupError>>,
}

struct IndexedEvent<'s> {
    event: &'s Event,
    fields: HashMap<&'s Identifier, Result<&'s Field, LookupError>>,
}

struct IndexedRecord<'s> {
    record: &'s Record,
    fields: HashMap<&'s Identifier, Result<&'s Field, LookupError>>,
}

impl<'s> SchemaIndex<'s> {
    /// Indexes every named element of `schema`.
    pub fn new(schema: &'s Schema) -> Self {
        Self {
            entities: index_by(
                &schema.entities,
                |e| &e.name,
                |entity| IndexedEntity {
                    entity,
                    events: index_by(
                        &entity.events,
                        |e| &e.name,
                        |event| IndexedEvent {
                            event,
                            fields: index_by(&event.payload, |f| &f.name, |field| field),
                        },
                    ),
                },
            ),
            records: index_by(
                &schema.records,
                |r| &r.name,
                |record| IndexedRecord {
                    record,
                    fields: index_by(&record.fields, |f| &f.name, |field| field),
                },
            ),
        }
    }

    /// The entity declared under `name`.
    pub fn entity(&self, name: &Identifier) -> Result<&'s Entity, LookupError> {
        get(&self.entities, name).map(|e| e.entity)
    }

    /// The record declared under `name`.
    pub fn record(&self, name: &Identifier) -> Result<&'s Record, LookupError> {
        get(&self.records, name).map(|r| r.record)
    }

    /// The event named `event` on entity `entity`.
    pub fn event(&self, entity: &Identifier, event: &Identifier) -> Result<&'s Event, LookupError> {
        get(&get(&self.entities, entity)?.events, event).map(|e| e.event)
    }

    /// The field named `field` of event `event` on entity `entity`.
    pub fn event_field(
        &self,
        entity: &Identifier,
        event: &Identifier,
        field: &Identifier,
    ) -> Result<&'s Field, LookupError> {
        let entity = get(&self.entities, entity)?;
        let event = get(&entity.events, event)?;
        get(&event.fields, field).copied()
    }

    /// The field named `field` of record `record`.
    pub fn record_field(
        &self,
        record: &Identifier,
        field: &Identifier,
    ) -> Result<&'s Field, LookupError> {
        let record = get(&self.records, record)?;
        get(&record.fields, field).copied()
    }
}

/// Reads `name` from an index map: a unique name yields its value, a repeated name
/// its stored [`LookupError::Duplicate`], an absent name [`LookupError::Missing`].
fn get<'a, V>(
    map: &'a HashMap<&Identifier, Result<V, LookupError>>,
    name: &Identifier,
) -> Result<&'a V, LookupError> {
    match map.get(name) {
        Some(Ok(value)) => Ok(value),
        Some(Err(error)) => Err(error.clone()),
        None => Err(LookupError::Missing(name.clone())),
    }
}

/// Indexes `items` by the name `key` returns. A name declared more than once maps
/// to [`LookupError::Duplicate`]; a unique name maps to `value(item)`.
fn index_by<'s, T, V>(
    items: &'s [T],
    key: impl Fn(&'s T) -> &'s Identifier,
    value: impl Fn(&'s T) -> V,
) -> HashMap<&'s Identifier, Result<V, LookupError>> {
    let mut map = HashMap::default();
    for item in items {
        let name = key(item);
        map.entry(name)
            .and_modify(|slot| *slot = Err(LookupError::Duplicate(name.clone())))
            .or_insert_with(|| Ok(value(item)));
    }
    map
}

impl Schema {
    /// Walks this schema in pre-order with `visitor`, returning its
    /// [`Visitor::Output`]. Every element is visited exactly once. At each node,
    /// in order:
    ///
    /// 1. the node itself, before any of its children;
    /// 2. its own [`Annotations`], first among its children;
    /// 3. its structural children, in declaration order.
    ///
    /// A field's [`DataType`] is one of its children, recursed by variant:
    ///
    /// 1. `Option` and `List` descend into their inner type;
    /// 2. `EntityRef` surfaces its own [`Annotations`], then recurses into any carried data;
    /// 3. `Record(name)` is a leaf: it is not followed (resolve via [`SchemaIndex::record`]).
    ///
    /// Every record is visited once under [`Schema::records`].
    pub fn walk<T: Visitor>(&self, mut visitor: T) -> T::Output {
        let index = SchemaIndex::new(self);
        let mut cursor = Cursor::new(self);

        visitor.visit(&cursor, &index);
        walk_annotations(&mut cursor, &index, &mut visitor, &self.annotations);
        for entity in &self.entities {
            walk_entity(&mut cursor, &index, &mut visitor, entity);
        }
        for record in &self.records {
            walk_record(&mut cursor, &index, &mut visitor, record);
        }

        visitor.finish()
    }
}

fn walk_annotations<'s, V: Visitor>(
    cursor: &mut Cursor<'s>,
    index: &SchemaIndex<'s>,
    visitor: &mut V,
    annotations: &'s Annotations,
) {
    cursor.enter(Element::Annotations(annotations));
    visitor.visit(cursor, index);
    cursor.leave();
}

fn walk_entity<'s, V: Visitor>(
    cursor: &mut Cursor<'s>,
    index: &SchemaIndex<'s>,
    visitor: &mut V,
    entity: &'s Entity,
) {
    cursor.enter(Element::Entity(entity));
    visitor.visit(cursor, index);
    walk_annotations(cursor, index, visitor, &entity.annotations);
    for event in &entity.events {
        walk_event(cursor, index, visitor, event);
    }
    cursor.leave();
}

fn walk_event<'s, V: Visitor>(
    cursor: &mut Cursor<'s>,
    index: &SchemaIndex<'s>,
    visitor: &mut V,
    event: &'s Event,
) {
    cursor.enter(Element::Event(event));
    visitor.visit(cursor, index);
    walk_annotations(cursor, index, visitor, &event.annotations);
    for field in &event.payload {
        walk_field(cursor, index, visitor, field);
    }
    cursor.leave();
}

fn walk_record<'s, V: Visitor>(
    cursor: &mut Cursor<'s>,
    index: &SchemaIndex<'s>,
    visitor: &mut V,
    record: &'s Record,
) {
    cursor.enter(Element::Record(record));
    visitor.visit(cursor, index);
    walk_annotations(cursor, index, visitor, &record.annotations);
    for field in &record.fields {
        walk_field(cursor, index, visitor, field);
    }
    cursor.leave();
}

fn walk_field<'s, V: Visitor>(
    cursor: &mut Cursor<'s>,
    index: &SchemaIndex<'s>,
    visitor: &mut V,
    field: &'s Field,
) {
    cursor.enter(Element::Field(field));
    visitor.visit(cursor, index);
    walk_annotations(cursor, index, visitor, &field.annotations);
    walk_data_type(cursor, index, visitor, &field.ty);
    cursor.leave();
}

fn walk_data_type<'s, V: Visitor>(
    cursor: &mut Cursor<'s>,
    index: &SchemaIndex<'s>,
    visitor: &mut V,
    ty: &'s DataType,
) {
    cursor.enter(Element::DataType(ty));
    visitor.visit(cursor, index);
    match ty {
        DataType::Option(inner) | DataType::List(inner) => {
            walk_data_type(cursor, index, visitor, inner);
        }
        DataType::EntityRef { data, annotations } => {
            walk_annotations(cursor, index, visitor, annotations);
            if let Some(inner) = data {
                walk_data_type(cursor, index, visitor, inner);
            }
        }
        _ => {}
    }
    cursor.leave();
}

// Macro to create impls for tuples of visitors, so output can be collected
// without having to upcast.
macro_rules! tuple_impls {
    ($($T:ident => $idx:tt),+) => {
        impl<$($T: Visitor),+> Visitor for ($($T,)+) {
            type Output = ($($T::Output,)+);
            fn visit(&mut self, cursor: &Cursor, index: &SchemaIndex) {
                $( self.$idx.visit(cursor, index); )+
            }
            fn finish(self) -> Self::Output {
                ($( self.$idx.finish(), )+)
            }
        }
    };
}
tuple_impls!(A => 0, B => 1);
tuple_impls!(A => 0, B => 1, C => 2);
tuple_impls!(A => 0, B => 1, C => 2, D => 3);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7, I => 8);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7, I => 8, J => 9);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7, I => 8, J => 9, K => 10);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7, I => 8, J => 9, K => 10, L => 11);

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Constraint, schema::event::Cardinality};

    fn ident(s: &str) -> Identifier {
        Identifier::try_new(s).unwrap()
    }

    // Counts how often each kind of element is visited.
    #[derive(Default)]
    struct ElementCounter {
        schemas: usize,
        annotations: usize,
        entities: usize,
        events: usize,
        fields: usize,
        records: usize,
        data_types: usize,
    }

    impl Visitor for ElementCounter {
        type Output = ElementCounter;
        fn visit(&mut self, cursor: &Cursor, _index: &SchemaIndex) {
            match cursor.current() {
                Element::Schema(_) => self.schemas += 1,
                Element::Annotations(_) => self.annotations += 1,
                Element::Entity(_) => self.entities += 1,
                Element::Event(_) => self.events += 1,
                Element::Field(_) => self.fields += 1,
                Element::Record(_) => self.records += 1,
                Element::DataType(_) => self.data_types += 1,
            }
        }
        fn finish(self) -> ElementCounter {
            self
        }
    }

    // Stateful visitor that drinks a beer at every element.
    #[derive(Default)]
    struct BarVisitor {
        beers: u8,
    }
    impl Visitor for BarVisitor {
        type Output = u8;
        fn visit(&mut self, _cursor: &Cursor, _index: &SchemaIndex) {
            self.beers += 1;
        }
        fn finish(self) -> u8 {
            self.beers
        }
    }

    fn sample_schema() -> Schema {
        Schema {
            name: ident("S"),
            annotations: Annotations::default(),
            entities: vec![Entity {
                name: ident("E"),
                annotations: Annotations::default(),
                events: vec![Event {
                    name: ident("Ev"),
                    cardinality: Cardinality::Once,
                    annotations: Annotations::default(),
                    payload: vec![Field {
                        name: ident("f"),
                        ty: DataType::U64,
                        annotations: Annotations::default(),
                    }],
                }],
            }],
            records: vec![Record {
                name: ident("R"),
                annotations: Annotations::default(),
                fields: vec![Field {
                    name: ident("rf"),
                    ty: DataType::U64,
                    annotations: Annotations::default(),
                }],
            }],
        }
    }

    #[test]
    fn visits_every_element() {
        let counter = sample_schema().walk(ElementCounter::default());
        assert_eq!(counter.schemas, 1);
        assert_eq!(counter.entities, 1);
        assert_eq!(counter.events, 1);
        assert_eq!(counter.fields, 2);
        assert_eq!(counter.records, 1);
        assert_eq!(counter.data_types, 2);
        assert_eq!(counter.annotations, 6);
    }

    #[test]
    fn recurses_data_types_and_entity_ref_annotations() {
        let entity_ref = DataType::EntityRef {
            data: None,
            annotations: Annotations {
                constraints: vec![Constraint {
                    name: "my.constraint.v1".to_string(),
                    data: None,
                }],
                ..Default::default()
            },
        };
        let schema = Schema {
            name: ident("S"),
            annotations: Annotations::default(),
            entities: vec![Entity {
                name: ident("E"),
                annotations: Annotations::default(),
                events: vec![Event {
                    name: ident("Ev"),
                    cardinality: Cardinality::Once,
                    annotations: Annotations::default(),
                    payload: vec![Field {
                        name: ident("x"),
                        ty: DataType::Option(Box::new(DataType::List(Box::new(entity_ref)))),
                        annotations: Annotations::default(),
                    }],
                }],
            }],
            records: vec![],
        };

        let census = schema.walk(ElementCounter::default());
        // Option -> List -> EntityRef.
        assert_eq!(census.data_types, 3);
        // Schema, Entity, Event, the Field, and the EntityRef's own annotations.
        assert_eq!(census.annotations, 5);
    }

    #[test]
    fn tuple_visitor_collects_each_output() {
        let (counter, beers) =
            sample_schema().walk((ElementCounter::default(), BarVisitor::default()));
        let total = counter.schemas
            + counter.annotations
            + counter.entities
            + counter.events
            + counter.fields
            + counter.records
            + counter.data_types;
        assert_eq!(total as u8, beers);
    }

    #[test]
    fn index_resolves_names_and_fields() {
        let schema = sample_schema();
        let index = SchemaIndex::new(&schema);

        assert!(index.entity(&ident("E")).is_ok());
        assert!(index.record(&ident("R")).is_ok());
        assert!(
            index
                .event_field(&ident("E"), &ident("Ev"), &ident("f"))
                .is_ok()
        );
        assert!(index.record_field(&ident("R"), &ident("rf")).is_ok());
    }

    #[test]
    fn index_reports_missing() {
        let schema = sample_schema();
        let index = SchemaIndex::new(&schema);

        assert_eq!(
            index.entity(&ident("nope")),
            Err(LookupError::Missing(ident("nope")))
        );
        // The entity exists but the field does not.
        assert_eq!(
            index.event_field(&ident("E"), &ident("Ev"), &ident("nope")),
            Err(LookupError::Missing(ident("nope")))
        );
        // The entity itself is absent.
        assert_eq!(
            index.event_field(&ident("nope"), &ident("Ev"), &ident("f")),
            Err(LookupError::Missing(ident("nope")))
        );
    }

    #[test]
    fn index_reports_duplicate_entity() {
        let entity = |name: &str| Entity {
            name: ident(name),
            annotations: Annotations::default(),
            events: vec![],
        };
        let schema = Schema {
            name: ident("S"),
            annotations: Annotations::default(),
            entities: vec![entity("Dup"), entity("Dup"), entity("Unique")],
            records: vec![],
        };
        let index = SchemaIndex::new(&schema);

        // Only the duplicated name errors; other names resolve as usual.
        assert_eq!(
            index.entity(&ident("Dup")),
            Err(LookupError::Duplicate(ident("Dup")))
        );
        assert!(index.entity(&ident("Unique")).is_ok());
        assert_eq!(
            index.entity(&ident("Other")),
            Err(LookupError::Missing(ident("Other")))
        );
    }

    #[test]
    fn index_reports_duplicate_event_field() {
        let field = || Field {
            name: ident("f"),
            ty: DataType::U64,
            annotations: Annotations::default(),
        };
        let schema = Schema {
            name: ident("S"),
            annotations: Annotations::default(),
            entities: vec![Entity {
                name: ident("E"),
                annotations: Annotations::default(),
                events: vec![Event {
                    name: ident("Ev"),
                    cardinality: Cardinality::Once,
                    annotations: Annotations::default(),
                    payload: vec![field(), field()],
                }],
            }],
            records: vec![],
        };
        let index = SchemaIndex::new(&schema);

        assert_eq!(
            index.event_field(&ident("E"), &ident("Ev"), &ident("f")),
            Err(LookupError::Duplicate(ident("f")))
        );
    }

    #[test]
    fn index_reports_duplicate_event() {
        let event = |name: &str| Event {
            name: ident(name),
            cardinality: Cardinality::Once,
            annotations: Annotations::default(),
            payload: vec![],
        };
        let schema = Schema {
            name: ident("S"),
            annotations: Annotations::default(),
            entities: vec![Entity {
                name: ident("E"),
                annotations: Annotations::default(),
                events: vec![event("Ev"), event("Ev")],
            }],
            records: vec![],
        };
        let index = SchemaIndex::new(&schema);

        assert_eq!(
            index.event(&ident("E"), &ident("Ev")),
            Err(LookupError::Duplicate(ident("Ev")))
        );
        // The duplicate event propagates to a field lookup through it.
        assert_eq!(
            index.event_field(&ident("E"), &ident("Ev"), &ident("f")),
            Err(LookupError::Duplicate(ident("Ev")))
        );
    }

    #[test]
    fn index_reports_duplicate_record() {
        let record = |name: &str| Record {
            name: ident(name),
            annotations: Annotations::default(),
            fields: vec![],
        };
        let schema = Schema {
            name: ident("S"),
            annotations: Annotations::default(),
            entities: vec![],
            records: vec![record("Dup"), record("Dup")],
        };
        let index = SchemaIndex::new(&schema);

        assert_eq!(
            index.record(&ident("Dup")),
            Err(LookupError::Duplicate(ident("Dup")))
        );
    }

    #[test]
    fn index_reports_duplicate_record_field() {
        let field = || Field {
            name: ident("f"),
            ty: DataType::U64,
            annotations: Annotations::default(),
        };
        let schema = Schema {
            name: ident("S"),
            annotations: Annotations::default(),
            entities: vec![],
            records: vec![Record {
                name: ident("R"),
                annotations: Annotations::default(),
                fields: vec![field(), field()],
            }],
        };
        let index = SchemaIndex::new(&schema);

        assert_eq!(
            index.record_field(&ident("R"), &ident("f")),
            Err(LookupError::Duplicate(ident("f")))
        );
    }

    #[test]
    fn lookup_error_displays() {
        assert_eq!(
            LookupError::Missing(ident("x")).to_string(),
            "no element named \"x\""
        );
        assert_eq!(
            LookupError::Duplicate(ident("x")).to_string(),
            "name \"x\" is declared more than once"
        );
    }

    #[test]
    fn cursor_and_index_are_available_during_visit() {
        struct Probe;
        impl Visitor for Probe {
            type Output = ();
            fn visit(&mut self, cursor: &Cursor, index: &SchemaIndex) {
                // The root is always the schema being walked.
                assert_eq!(cursor.root().name, ident("S"));
                // The path begins at the schema root.
                assert!(matches!(
                    cursor.elements().first(),
                    Some(Element::Schema(_))
                ));
                // A field's parent is the event or record that declares it.
                if let Element::Field(_) = cursor.current() {
                    assert!(matches!(
                        cursor.previous(),
                        Some(Element::Event(_) | Element::Record(_))
                    ));
                }
                // Declared names resolve through the index mid-walk.
                if let Element::Schema(_) = cursor.current() {
                    assert!(index.entity(&ident("E")).is_ok());
                    assert!(index.record(&ident("R")).is_ok());
                    assert!(index.entity(&ident("nope")).is_err());
                }
            }
            fn finish(self) {}
        }

        sample_schema().walk(Probe);
    }

    #[test]
    fn record_reference_is_a_leaf() {
        let schema = Schema {
            name: ident("S"),
            annotations: Annotations::default(),
            entities: vec![Entity {
                name: ident("E"),
                annotations: Annotations::default(),
                events: vec![Event {
                    name: ident("Ev"),
                    cardinality: Cardinality::Once,
                    annotations: Annotations::default(),
                    payload: vec![Field {
                        name: ident("f"),
                        ty: DataType::Record(ident("R")),
                        annotations: Annotations::default(),
                    }],
                }],
            }],
            records: vec![Record {
                name: ident("R"),
                annotations: Annotations::default(),
                fields: vec![Field {
                    name: ident("rf"),
                    ty: DataType::U64,
                    annotations: Annotations::default(),
                }],
            }],
        };

        let counter = schema.walk(ElementCounter::default());
        // The `Record(R)` reference is one DataType leaf; R's field is visited
        // only under records, not inlined at the reference.
        assert_eq!(counter.fields, 2);
        assert_eq!(counter.data_types, 2);
        assert_eq!(counter.records, 1);
    }

    #[test]
    fn walks_empty_schema() {
        let schema = Schema {
            name: ident("S"),
            annotations: Annotations::default(),
            entities: vec![],
            records: vec![],
        };

        let counter = schema.walk(ElementCounter::default());
        // Only the schema node and its annotations are visited.
        assert_eq!(counter.schemas, 1);
        assert_eq!(counter.annotations, 1);
        assert_eq!(counter.entities, 0);
        assert_eq!(counter.events, 0);
        assert_eq!(counter.fields, 0);
        assert_eq!(counter.records, 0);
        assert_eq!(counter.data_types, 0);
    }

    #[test]
    fn annotations_are_walked_first() {
        #[derive(Default)]
        struct Trace(Vec<&'static str>);
        impl Visitor for Trace {
            type Output = Vec<&'static str>;
            fn visit(&mut self, cursor: &Cursor, _index: &SchemaIndex) {
                self.0.push(match cursor.current() {
                    Element::Schema(_) => "schema",
                    Element::Entity(_) => "entity",
                    Element::Event(_) => "event",
                    Element::Field(_) => "field",
                    Element::Record(_) => "record",
                    Element::DataType(_) => "data_type",
                    Element::Annotations(_) => "annotations",
                });
            }
            fn finish(self) -> Vec<&'static str> {
                self.0
            }
        }

        // Each node's annotations come before its structural children.
        let trace = sample_schema().walk(Trace::default());
        assert_eq!(
            trace,
            [
                "schema",
                "annotations", // schema
                "entity",
                "annotations", // entity E
                "event",
                "annotations", // event Ev
                "field",
                "annotations", // field f
                "data_type",
                "record",
                "annotations", // record R
                "field",
                "annotations", // field rf
                "data_type",
            ]
        );
    }
}
