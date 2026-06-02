// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Quent built-in FSM constraint

use std::collections::{BTreeMap, HashSet};

use petgraph::{
    algo::tarjan_scc,
    graphmap::DiGraphMap,
    visit::{Bfs, Reversed, Walker},
};
use quent_constraints::Constraint;
use quent_schema::{Schema, event::Cardinality, identifier::Identifier};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A node in an [`Fsm`] topology.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum State {
    /// A special state that, when transitioned from, describes the FSM came into existence.
    Entry,
    /// A special state that, when transitioned into, describes an FSM going out of existence.
    Exit,
    /// A regular state, named after the event that transitions into it.
    Named(Identifier),
}

/// A directed transition between two [`State`]s in an [`Fsm`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Transition {
    pub source: State,
    pub target: State,
}

/// The state-transition topology of a finite state machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fsm {
    pub transitions: Vec<Transition>,
}

/// Constrains the order of an entity's events by a Finite-State-Machine
/// topology.
///
/// Through this constraint, the behavior of an entity can be modeled as an FSM.
/// The event order is restricted by a certain topology of "states" and
/// "transitions" between those states, where events represent the moment in
/// time the FSM transitions into a state with the name of the event.
///
/// Modeling entities as FSMs is useful to trace a specific restricted lifecycle
/// of the entity. Possible states include the special "entry" and "exit" states,
/// such that:
/// - there is exactly one transition from the "entry" state into some initial
///   state, and
/// - from each state, there exists a sequence of transitions to the "exit" state.
///
/// The moment an entity modeled as an FSM in the client code transitions
/// between states, the transition event is to be emitted. At that time, both
/// trigger conditions and state outputs can be captured in the event's
/// attributes. Where applicable, if users desire to capture changes to an FSM's
/// outputs as a function of its inputs without advancing to a different state,
/// this can be modeled as a self-transition that updates those attributes.
///
/// Be aware that Quent's concept of an FSM is a strict subset of what can
/// typically be expressed in full-fledged finite-state-automata theory.
///
/// ## Requirements
///
/// For every entity carrying this constraint:
///
/// 1. No event in the entity may be named `entry` or `exit` (case-insensitively).
/// 2. Exactly one transition has [`State::Entry`] as its source, and no
///    transition has [`State::Entry`] as its target.
/// 3. At least one transition has [`State::Exit`] as its target, and no
///    transition has [`State::Exit`] as its source.
/// 4. There is at least one [`State::Named`] state.
/// 5. Every [`State::Named`] state is reachable from [`State::Entry`].
/// 6. Every [`State::Named`] state can reach [`State::Exit`].
/// 7. Every [`State::Named`] state corresponds to an event in the entity.
/// 8. Every event in the entity appears as some [`State::Named`] state.
/// 9. A state on a cycle has [`Cardinality::Multi`], otherwise
///    [`Cardinality::Once`].
pub struct FsmConstraint;

impl Constraint for FsmConstraint {
    const NAME: &'static str = "quent.fsm.v1";

    fn validate(&self, schema: &Schema) -> Result<(), Box<dyn std::error::Error>> {
        let mut errors = Vec::new();
        for entity in &schema.entities {
            let Some(constraint) = entity
                .annotations
                .constraints
                .iter()
                .find(|c| c.name == Self::NAME)
            else {
                continue;
            };
            let raw = match constraint.data.as_deref() {
                Some(s) => s,
                None => {
                    errors.push(FsmError::InvalidData {
                        entity: entity.name.clone(),
                        message: "constraint data is missing".to_string(),
                    });
                    continue;
                }
            };
            let fsm = match serde_json::from_str::<Fsm>(raw) {
                Ok(f) => f,
                Err(e) => {
                    errors.push(FsmError::InvalidData {
                        entity: entity.name.clone(),
                        message: format!("failed to decode fsm: {e}"),
                    });
                    continue;
                }
            };
            check_entity(entity, &fsm, &mut errors);
        }

        match errors.len() {
            0 => Ok(()),
            1 => Err(errors.pop().unwrap().into()),
            _ => Err(FsmError::Multiple(errors).into()),
        }
    }
}

fn check_entity(entity: &quent_schema::entity::Entity, fsm: &Fsm, errors: &mut Vec<FsmError>) {
    // Requirement 1: no event may be named "entry" or "exit".
    for event in &entity.events {
        let lower = event.name.to_ascii_lowercase();
        if lower == "entry" || lower == "exit" {
            errors.push(FsmError::ReservedStateName {
                entity: entity.name.clone(),
                name: if lower == "entry" { "entry" } else { "exit" },
            });
        }
    }

    // Requirement 2: exactly one transition has Entry as its source.
    let entry_transitions = fsm
        .transitions
        .iter()
        .filter(|t| matches!(t.source, State::Entry))
        .count();
    if entry_transitions != 1 {
        errors.push(FsmError::WrongEntryCount {
            entity: entity.name.clone(),
            found: entry_transitions,
        });
    }
    // Entry is only ever transitioned from, never into.
    if fsm
        .transitions
        .iter()
        .any(|t| matches!(t.target, State::Entry))
    {
        errors.push(FsmError::TransitionIntoEntry {
            entity: entity.name.clone(),
        });
    }

    // Requirement 3: at least one transition has Exit as its target.
    let exit_transitions = fsm
        .transitions
        .iter()
        .filter(|t| matches!(t.target, State::Exit))
        .count();
    if exit_transitions == 0 {
        errors.push(FsmError::NoExit {
            entity: entity.name.clone(),
        });
    }
    // Exit is only ever transitioned into, never from.
    if fsm
        .transitions
        .iter()
        .any(|t| matches!(t.source, State::Exit))
    {
        errors.push(FsmError::TransitionOutOfExit {
            entity: entity.name.clone(),
        });
    }

    let event_names: HashSet<&Identifier> = entity.events.iter().map(|e| &e.name).collect();
    let cardinality_by_event: BTreeMap<&Identifier, Cardinality> = entity
        .events
        .iter()
        .map(|e| (&e.name, e.cardinality))
        .collect();

    let named_states: HashSet<&Identifier> = fsm
        .transitions
        .iter()
        .flat_map(|t| [&t.source, &t.target])
        .filter_map(|s| match s {
            State::Named(name) => Some(name),
            _ => None,
        })
        .collect();

    // Requirement 4: the FSM has at least one named state.
    if named_states.is_empty() {
        errors.push(FsmError::NoNamedStates {
            entity: entity.name.clone(),
        });
    }

    // Requirement 7: every named state corresponds to an entity event.
    for &state in &named_states {
        if !event_names.contains(state) {
            errors.push(FsmError::UnknownState {
                entity: entity.name.clone(),
                state: state.clone(),
            });
        }
    }

    // Requirement 8: every entity event appears as a named state.
    for event in &entity.events {
        if !named_states.contains(&event.name) {
            errors.push(FsmError::UncoveredEvent {
                entity: entity.name.clone(),
                event: event.name.clone(),
            });
        }
    }

    let graph: DiGraphMap<GraphNode, ()> = fsm
        .transitions
        .iter()
        .map(|t| (GraphNode::from(&t.source), GraphNode::from(&t.target), ()))
        .collect();

    // Requirement 5: every named state is reachable from Entry
    if entry_transitions != 0 {
        let reachable_from_entry: HashSet<GraphNode> =
            Bfs::new(&graph, GraphNode::Entry).iter(&graph).collect();
        for &name in &named_states {
            if !reachable_from_entry.contains(&GraphNode::Named(name)) {
                errors.push(FsmError::UnreachableFromEntry {
                    entity: entity.name.clone(),
                    state: name.clone(),
                });
            }
        }
    }

    // Requirement 6: every named state can reach Exit.
    if exit_transitions != 0 {
        let reversed = Reversed(&graph);
        let reaches_exit: HashSet<GraphNode> =
            Bfs::new(reversed, GraphNode::Exit).iter(reversed).collect();
        for &name in &named_states {
            if !reaches_exit.contains(&GraphNode::Named(name)) {
                errors.push(FsmError::CannotReachExit {
                    entity: entity.name.clone(),
                    state: name.clone(),
                });
            }
        }
    }

    // Requirement 9: a state on a cycle is Multi, otherwise Once.
    let on_cycle = find_cyclic(&graph, fsm);
    for &name in &named_states {
        let expected_cardinality = if on_cycle.contains(name) {
            Cardinality::Multi
        } else {
            Cardinality::Once
        };
        let Some(actual) = cardinality_by_event.get(name) else {
            continue;
        };
        if *actual != expected_cardinality {
            errors.push(FsmError::CardinalityMismatch {
                entity: entity.name.clone(),
                state: name.clone(),
                expected: expected_cardinality,
                found: *actual,
            });
        }
    }
}

/// Compute the states that lie on a cycle in the transition graph.
fn find_cyclic<'a>(graph: &DiGraphMap<GraphNode<'a>, ()>, fsm: &'a Fsm) -> HashSet<&'a Identifier> {
    let mut on_cycle = HashSet::new();
    // A node is on a cycle if it sits in a strongly connected component of more
    // than one node.
    for scc in tarjan_scc(graph) {
        if scc.len() > 1 {
            for node in scc {
                if let GraphNode::Named(name) = node {
                    on_cycle.insert(name);
                }
            }
        }
    }
    // It also sits on a cycle if it has a self-loop, but `tarjan_scc` does not
    // report those as cyclic, so do them separately:
    for t in &fsm.transitions {
        if t.source == t.target
            && let State::Named(n) = &t.source
        {
            on_cycle.insert(n);
        }
    }
    on_cycle
}

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
enum GraphNode<'a> {
    Entry,
    Exit,
    Named(&'a Identifier),
}

impl<'a> From<&'a State> for GraphNode<'a> {
    fn from(state: &'a State) -> Self {
        match state {
            State::Entry => GraphNode::Entry,
            State::Exit => GraphNode::Exit,
            State::Named(name) => GraphNode::Named(name),
        }
    }
}

#[derive(Debug, Error)]
pub enum FsmError {
    #[error("entity \"{entity}\" fsm: {message}")]
    InvalidData { entity: Identifier, message: String },
    #[error("entity \"{entity}\" fsm: \"{name}\" is a reserved state name")]
    ReservedStateName {
        entity: Identifier,
        name: &'static str,
    },
    #[error("entity \"{entity}\" fsm: expected exactly one transition from entry, found {found}")]
    WrongEntryCount { entity: Identifier, found: usize },
    #[error("entity \"{entity}\" fsm: no transition reaches exit")]
    NoExit { entity: Identifier },
    #[error("entity \"{entity}\" fsm: a transition targets the entry state")]
    TransitionIntoEntry { entity: Identifier },
    #[error("entity \"{entity}\" fsm: a transition originates from the exit state")]
    TransitionOutOfExit { entity: Identifier },
    #[error("entity \"{entity}\" fsm: has no named states")]
    NoNamedStates { entity: Identifier },
    #[error("entity \"{entity}\" fsm: state \"{state}\" is unreachable from entry")]
    UnreachableFromEntry {
        entity: Identifier,
        state: Identifier,
    },
    #[error("entity \"{entity}\" fsm: state \"{state}\" cannot reach exit")]
    CannotReachExit {
        entity: Identifier,
        state: Identifier,
    },
    #[error("entity \"{entity}\" fsm: state \"{state}\" does not match any event")]
    UnknownState {
        entity: Identifier,
        state: Identifier,
    },
    #[error("entity \"{entity}\" fsm: event \"{event}\" is not covered by any transition")]
    UncoveredEvent {
        entity: Identifier,
        event: Identifier,
    },
    #[error(
        "entity \"{entity}\" fsm: state \"{state}\" expects cardinality {expected:?}, but event has {found:?}"
    )]
    CardinalityMismatch {
        entity: Identifier,
        state: Identifier,
        expected: Cardinality,
        found: Cardinality,
    },
    #[error("multiple fsm violations:\n{}", join_errors(.0))]
    Multiple(Vec<FsmError>),
}

fn join_errors(errors: &[FsmError]) -> String {
    errors
        .iter()
        .map(|e| format!("  - {e}"))
        .collect::<Vec<_>>()
        .join("\n")
}
