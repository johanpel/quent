// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Constraint marking an entity reference as tree-forming.

use std::collections::{HashMap, VecDeque};

use quent_constraints::Constraint;
use quent_ref_target::{RefTarget, RefTargetConstraint};
use quent_schema::{
    Schema, annotations::Annotations, data_type::DataType, identifier::Identifier, record::Record,
};
use thiserror::Error;

/// Constraint to express a tree connecting all entities within a graph where
/// vertices represent entities and edges represent entity references.
///
/// This constraint can be used for arbitrary purposes. Its canonical purpose
/// is to provide some "preferred" way of traversing entities and their events
/// from a single starting point (the root entity), e.g. such that some user
/// interface can help a human traverse the trace in this preferred way.
///
/// References annotated with this constraint are typically used (but not
/// limited) to express:
/// - Causal relations (e.g. entity Y was produced by entity X)
/// - Hierarchical relations (e.g. entity Y is part of / owned by / scoped by X)
///
/// In order for instrumentation libraries to provide strong guarantees
/// (typically compile-time) that this constraint is met, the tree must be fully
/// defined at "schema-time". Therefore, type-erased entity references cannot
/// carry an annotation with this constraint, as this would allow forming entity
/// graphs that are not trees (i.e. multiple instances of an entity of type A
/// would be able to emit events that refer to both an entity of type B and of
/// type C). For this reason, this constraint depends on the constraint provided
/// by the [`quent_ref_target`] crate.
///
/// ## Requirements
///
/// 1. The schema has exactly one entity (a.k.a. the root entity) that does not
///    carry an entity reference annotated with this constraint in any of its
///    events.
/// 2. Every non-root entity has at least one event carrying an entity
///    reference annotated with this constraint to declare it refers to exactly
///    one type of parent entity in the tree (a.k.a. a parent entity reference).
/// 3. Every parent entity reference must be target-constrained (carry a
///    [`quent_ref_target`] annotation). A type-erased reference may not carry
///    this constraint (implied by requirement 2).
/// 4. There is exactly one path from every non-root entity type to the root
///    entity type through parent entity references.
///
/// ## Note on possible parent ambiguity (req. 2)
///
/// Parent ambiguity at run-time can exist through multiple parent-declaring
/// events, which is allowed by requirement 2.
///
/// Since client code can have branching behavior where certain events are
/// conditionally emitted, this constraint permits the parent reference to be
/// placed (once) on any number of events, even though logically speaking, it
/// can only have one parent, and it would ideally emit its parent reference
/// exactly once. It is the responsibility of the client code to ensure it
/// produces an unambiguous event stream with regards to this tree-forming
/// constraint.
///
/// This constraint intentionally defers any potential resolution to the problem
/// of clients producing ambiguous event streams to schema producer / consumer
/// implementations.
///
/// For example, a modeling API or DSL _could_ decide to enforce FSM entities to
/// always declare their parent in the initial state. An instrumentation library
/// _could_ error on emitting a second parent-declaring event if it changes the
/// reference value. An analysis library _could_ produce an error when an event
/// stream is ingested exhibiting this ambiguity.
pub struct RefTreeConstraint;

impl Constraint for RefTreeConstraint {
    const NAME: &'static str = "quent.ref-tree.v1";

    fn validate(&self, schema: &Schema) -> Result<(), Box<dyn std::error::Error>> {
        let mut errors = Vec::new();
        check_tree(schema, &mut errors);
        match errors.len() {
            0 => Ok(()),
            1 => Err(errors.pop().unwrap().into()),
            _ => Err(RefTreeError::Multiple(errors).into()),
        }
    }
}

/// A tree-forming reference gathered from an entity's events.
enum TreeRef {
    /// Target-constrained: the parent type from the co-located
    /// `quent.ref-target.v1`.
    Targeted { target: Identifier },
    /// Type-erased: no decodable target (a req. 3 violation). The location is
    /// rendered here because this is the only path that consumes it.
    TypeErased { location: String },
}

/// Index into [`Schema::entities`]; never interchanged with a name or a count.
#[derive(Clone, Copy)]
struct EntityIdx(usize);

/// A non-root entity paired with the single parent entity its references
/// resolve to. There is no representable non-root without a parent edge.
struct NonRoot {
    entity: EntityIdx,
    parent: EntityIdx,
}

fn check_tree(schema: &Schema, errors: &mut Vec<RefTreeError>) {
    let n = schema.entities.len();

    // Gather, per entity, the tree-forming references its events declare,
    // descending through Option/List, reference payloads and record fields. At
    // most one is allowed per event (req. 2's parenthetical).
    let mut refs_by_entity: Vec<Vec<TreeRef>> = (0..n).map(|_| Vec::new()).collect();
    for (i, entity) in schema.entities.iter().enumerate() {
        for event in &entity.events {
            let mut in_event = Vec::new();
            for field in &event.payload {
                let loc = Loc::Field {
                    entity: &entity.name,
                    event: &event.name,
                    field: &field.name,
                };
                collect_refs(&field.ty, &loc, &schema.records, &mut in_event);
            }
            if in_event.len() > 1 {
                errors.push(RefTreeError::MultiplePerEvent {
                    entity: entity.name.clone(),
                    event: event.name.clone(),
                    count: in_event.len(),
                });
            }
            refs_by_entity[i].append(&mut in_event);
        }
    }

    // The constraint only forms a tree when at least one reference uses it.
    if refs_by_entity.iter().all(|r| r.is_empty()) {
        return;
    }

    // Req. 3: every parent reference must be target-constrained.
    for refs in &refs_by_entity {
        for r in refs {
            if let TreeRef::TypeErased { location } = r {
                errors.push(RefTreeError::NotTargetConstrained {
                    location: location.clone(),
                });
            }
        }
    }

    // Req. 1: exactly one root (an entity carrying no tree-forming reference).
    let roots: Vec<EntityIdx> = refs_by_entity
        .iter()
        .enumerate()
        .filter(|(_, r)| r.is_empty())
        .map(|(i, _)| EntityIdx(i))
        .collect();
    let root = match roots.as_slice() {
        [] => {
            errors.push(RefTreeError::NoRoot);
            return;
        }
        [root] => *root,
        _ => {
            let mut names: Vec<Identifier> = roots
                .iter()
                .map(|idx| schema.entities[idx.0].name.clone())
                .collect();
            names.sort();
            errors.push(RefTreeError::MultipleRoots { roots: names });
            return;
        }
    };

    let index: HashMap<&Identifier, EntityIdx> = schema
        .entities
        .iter()
        .enumerate()
        .map(|(i, e)| (&e.name, EntityIdx(i)))
        .collect();

    // Req. 2: each non-root must declare exactly one parent type. A non-root
    // carrying a type-erased reference is already reported (req. 3) and dropped.
    // A resolved non-root carries its parent edge by construction; a target
    // naming no entity has no path to the root (req. 4), reported directly.
    let mut graph: Vec<NonRoot> = Vec::new();
    for (i, refs) in refs_by_entity.iter().enumerate() {
        if refs.is_empty() || refs.iter().any(|r| matches!(r, TreeRef::TypeErased { .. })) {
            continue;
        }
        let mut parents: Vec<&Identifier> = refs
            .iter()
            .filter_map(|r| match r {
                TreeRef::Targeted { target } => Some(target),
                TreeRef::TypeErased { .. } => None,
            })
            .collect();
        parents.sort();
        parents.dedup();
        match parents.as_slice() {
            [parent] => match index.get(*parent) {
                Some(&parent) => graph.push(NonRoot {
                    entity: EntityIdx(i),
                    parent,
                }),
                None => errors.push(RefTreeError::Unreachable {
                    entity: schema.entities[i].name.clone(),
                }),
            },
            _ => errors.push(RefTreeError::ConflictingParents {
                entity: schema.entities[i].name.clone(),
                parents: parents.into_iter().cloned().collect(),
            }),
        }
    }

    // Req. 4: a walk from the unique root over parent -> child edges reaches
    // every non-root on a path to it. Any left unvisited sits on a cycle.
    let mut children: Vec<Vec<EntityIdx>> = (0..n).map(|_| Vec::new()).collect();
    for node in &graph {
        children[node.parent.0].push(node.entity);
    }
    let mut visited = vec![false; n];
    visited[root.0] = true;
    let mut queue = VecDeque::from([root]);
    while let Some(parent) = queue.pop_front() {
        for &child in &children[parent.0] {
            if !visited[child.0] {
                visited[child.0] = true;
                queue.push_back(child);
            }
        }
    }
    for node in &graph {
        if !visited[node.entity.0] {
            errors.push(RefTreeError::Unreachable {
                entity: schema.entities[node.entity.0].name.clone(),
            });
        }
    }
}

/// Gather every tree-forming reference reachable in `ty`, classifying each as
/// targeted or type-erased. Descends `Option`/`List`, reference payloads and —
/// resolving names against `records` — record fields. `loc` tracks the path so
/// a violation can name its site without allocating on the valid path.
fn collect_refs<'a>(
    ty: &'a DataType,
    loc: &'a Loc<'a>,
    records: &'a [Record],
    out: &mut Vec<TreeRef>,
) {
    match ty {
        DataType::Option(inner) | DataType::List(inner) => collect_refs(inner, loc, records, out),
        DataType::EntityRef { data, annotations } => {
            if has_tree(annotations) {
                out.push(match tree_ref_target(annotations) {
                    Some(target) => TreeRef::Targeted { target },
                    None => TreeRef::TypeErased {
                        location: loc.render(),
                    },
                });
            }
            if let Some(inner) = data {
                collect_refs(inner, loc, records, out);
            }
        }
        DataType::Record(name) => {
            // Guard against record definitions that nest cyclically.
            if loc.descends_record(name) {
                return;
            }
            if let Some(record) = records.iter().find(|r| &r.name == name) {
                for field in &record.fields {
                    let nested = Loc::Nested {
                        outer: loc,
                        record: name,
                        field: &field.name,
                    };
                    collect_refs(&field.ty, &nested, records, out);
                }
            }
        }
        _ => {}
    }
}

/// Borrowed path to a tree-forming reference, rendered to a string only when a
/// violation is reported.
enum Loc<'a> {
    Field {
        entity: &'a Identifier,
        event: &'a Identifier,
        field: &'a Identifier,
    },
    Nested {
        outer: &'a Loc<'a>,
        record: &'a Identifier,
        field: &'a Identifier,
    },
}

impl Loc<'_> {
    fn render(&self) -> String {
        match self {
            Loc::Field {
                entity,
                event,
                field,
            } => format!("entity \"{entity}\" event \"{event}\" field \"{field}\""),
            Loc::Nested {
                outer,
                record,
                field,
            } => format!(
                "{} -> record \"{record}\" field \"{field}\"",
                outer.render()
            ),
        }
    }

    /// Whether the path already descends through record `name` (a nesting cycle).
    fn descends_record(&self, name: &Identifier) -> bool {
        match self {
            Loc::Field { .. } => false,
            Loc::Nested { outer, record, .. } => *record == name || outer.descends_record(name),
        }
    }
}

/// The target entity type of a co-located `quent.ref-target.v1`, if present and
/// decodable.
fn tree_ref_target(annotations: &Annotations) -> Option<Identifier> {
    let constraint = annotations
        .constraints
        .iter()
        .find(|c| c.name == RefTargetConstraint::NAME)?;
    let raw = constraint.data.as_deref()?;
    serde_json::from_str::<RefTarget>(raw)
        .ok()
        .map(|rt| rt.target)
}

fn has_tree(annotations: &Annotations) -> bool {
    annotations
        .constraints
        .iter()
        .any(|c| c.name == RefTreeConstraint::NAME)
}

#[derive(Debug, Error)]
pub enum RefTreeError {
    #[error(
        "{location}: a tree-forming reference must be target-constrained (carry a quent.ref-target.v1 annotation)"
    )]
    NotTargetConstrained { location: String },
    #[error(
        "entity \"{entity}\" event \"{event}\": {count} tree-forming references, at most one is allowed"
    )]
    MultiplePerEvent {
        entity: Identifier,
        event: Identifier,
        count: usize,
    },
    #[error(
        "tree-forming references are used but no entity is a root (every entity has a parent); a tree needs exactly one root"
    )]
    NoRoot,
    #[error("more than one root entity (no tree-forming reference): {}", join_idents(.roots))]
    MultipleRoots { roots: Vec<Identifier> },
    #[error("entity \"{entity}\" declares more than one parent type: {}", join_idents(.parents))]
    ConflictingParents {
        entity: Identifier,
        parents: Vec<Identifier>,
    },
    #[error("entity \"{entity}\" has no path to the root through tree-forming references")]
    Unreachable { entity: Identifier },
    #[error("multiple ref-tree violations:\n{}", join_errors(.0))]
    Multiple(Vec<RefTreeError>),
}

fn join_idents(ids: &[Identifier]) -> String {
    ids.iter()
        .map(|i| format!("\"{i}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

fn join_errors(errors: &[RefTreeError]) -> String {
    errors
        .iter()
        .map(|e| format!("  - {e}"))
        .collect::<Vec<_>>()
        .join("\n")
}
