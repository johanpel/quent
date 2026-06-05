// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Visitors that check a [`Schema`] for internal consistency.

use quent_schema::{
    DataType,
    visitor::{Cursor, Element, IndexedSchema, LookupError, Visitor},
};

/// Reports every name declared more than once in its scope.
#[derive(Default)]
pub struct DuplicateNames {
    found: Vec<String>,
}
impl Visitor for DuplicateNames {
    type Output = Vec<String>;
    fn visit(&mut self, cursor: &Cursor, index: &IndexedSchema) {
        // The index already knows every duplicate.
        // Read them once, at the schema root (visited first, exactly once).
        if let Element::Schema(_) = cursor.current() {
            self.found = index
                .duplicate_paths()
                .iter()
                .map(ToString::to_string)
                .collect();
        }
    }
    fn finish(self) -> Self::Output {
        self.found
    }
}

/// Reports every internal reference that does not resolve.
///
/// Note that constraints adding internal references are responsible for
/// validating those internal references themselves.
#[derive(Default)]
pub struct UnresolvedReferences {
    found: Vec<String>,
}
impl Visitor for UnresolvedReferences {
    type Output = Vec<String>;
    fn visit(&mut self, cursor: &Cursor, index: &IndexedSchema) {
        if let Element::DataType(DataType::Record(name)) = cursor.current() {
            // a duplicate record still "exists", albeit in an ambiguous state
            // but this is checked by the DuplicateNames visitor already.
            if matches!(index.record(name), Err(LookupError::Missing(_))) {
                self.found.push(format!("{cursor}: {name}"));
            }
        }
    }
    fn finish(self) -> Self::Output {
        self.found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quent_schema::{
        Annotations, Cardinality, Constraint, Entity, Event, Field, Identifier, Metadata, Record,
        Schema,
    };

    fn ident(s: &str) -> Identifier {
        Identifier::try_new(s).unwrap()
    }

    fn field(name: &str, ty: DataType) -> Field {
        Field {
            name: ident(name),
            ty,
            annotations: Annotations::default(),
        }
    }

    fn event(name: &str, payload: Vec<Field>) -> Event {
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

    fn record(name: &str, fields: Vec<Field>) -> Record {
        Record {
            name: ident(name),
            fields,
            annotations: Annotations::default(),
        }
    }

    fn schema(entities: Vec<Entity>, records: Vec<Record>) -> Schema {
        Schema {
            name: ident("S"),
            entities,
            records,
            annotations: Annotations::default(),
        }
    }

    fn duplicates(schema: &Schema) -> Vec<String> {
        schema.walk(DuplicateNames::default())
    }

    fn unresolved(schema: &Schema) -> Vec<String> {
        schema.walk(UnresolvedReferences::default())
    }

    #[test]
    fn consistent_schema_passes() {
        let s = schema(
            vec![entity(
                "E",
                vec![event("Ev", vec![field("f", DataType::U64)])],
            )],
            vec![record("R", vec![field("rf", DataType::Record(ident("R")))])],
        );
        assert!(duplicates(&s).is_empty());
        assert!(unresolved(&s).is_empty());
    }

    #[test]
    fn duplicate_entity_is_reported() {
        let s = schema(vec![entity("E", vec![]), entity("E", vec![])], vec![]);
        assert_eq!(duplicates(&s), vec!["S.E".to_string()]);
    }

    #[test]
    fn duplicate_event_field_is_reported() {
        let s = schema(
            vec![entity(
                "E",
                vec![event(
                    "Ev",
                    vec![field("f", DataType::U64), field("f", DataType::Bool)],
                )],
            )],
            vec![],
        );
        assert!(duplicates(&s).contains(&"S.E.Ev.f".to_string()));
    }

    #[test]
    fn duplicate_constraint_name_is_reported() {
        let dup = Annotations {
            constraints: vec![
                Constraint {
                    name: "c".to_string(),
                    data: None,
                },
                Constraint {
                    name: "c".to_string(),
                    data: None,
                },
            ],
            ..Default::default()
        };
        let s = Schema {
            name: ident("S"),
            entities: vec![],
            records: vec![],
            annotations: dup,
        };
        assert_eq!(duplicates(&s), vec!["S.Annotations.c".to_string()]);
    }

    #[test]
    fn entity_and_record_may_share_a_name() {
        // distinct namespaces, not a duplicate.
        let s = schema(vec![entity("X", vec![])], vec![record("X", vec![])]);
        assert!(duplicates(&s).is_empty());
    }

    #[test]
    fn metadata_named_like_a_constraint_is_not_a_duplicate() {
        let annotations = Annotations {
            constraints: vec![Constraint {
                name: "x".to_string(),
                data: None,
            }],
            metadata: vec![Metadata {
                name: "x".to_string(),
                data: None,
            }],
            ..Default::default()
        };
        let s = Schema {
            name: ident("S"),
            entities: vec![],
            records: vec![],
            annotations,
        };
        assert!(duplicates(&s).is_empty());
    }

    #[test]
    fn unresolved_record_reference_is_reported() {
        let s = schema(
            vec![entity(
                "E",
                vec![event(
                    "Ev",
                    vec![field("f", DataType::Record(ident("ghost")))],
                )],
            )],
            vec![],
        );
        assert_eq!(
            unresolved(&s),
            vec!["S.E.Ev.f.Record(ghost): ghost".to_string()]
        );
    }

    #[test]
    fn resolved_record_reference_passes() {
        let s = schema(
            vec![entity(
                "E",
                vec![event("Ev", vec![field("f", DataType::Record(ident("R")))])],
            )],
            vec![record("R", vec![])],
        );
        assert!(unresolved(&s).is_empty());
    }

    #[test]
    fn references_into_nested_data_types_are_checked() {
        // Option<List<Record(ghost)>>
        let ty = DataType::Option(Box::new(DataType::List(Box::new(DataType::Record(ident(
            "ghost",
        ))))));
        let s = schema(
            vec![entity("E", vec![event("Ev", vec![field("f", ty)])])],
            vec![],
        );
        assert!(unresolved(&s).iter().any(|r| r.contains("ghost")));
    }
}
