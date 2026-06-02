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

/// A directed transition between two named states in an [`Fsm`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Transition {
    /// The name of the source state.
    source: Identifier,
    /// The name of the target state.
    target: Identifier,
}

/// The state-transition topology of a finite state machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fsm {
    /// The name of the initial state this FSM transitions into when it comes
    /// into existence.
    initial_state: Identifier,
    /// The possible transitions of this FSM.
    transitions: Vec<Transition>,
    /// The names of states from which this FSM can exit to go out of existence.
    exit_from_states: Vec<Identifier>,
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
/// of the entity. The lifecycle has a single initial state where it comes into
/// existence and a set of exit states from which it may go out of existence.
/// The topology must be formed such that every state is reachable from the
/// initial state, and from every state there exists a sequence of transitions to
/// some exit state.
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
/// 1. No event in the entity may be named `exit` (case-insensitively). This
///    is reserved such that instrumentation APIs can unambiguously provide an
///    event-emitting function marking the FSM's transition out of existence
///    without name clashes with possible user-defined events named "exit".
/// 2. There is at least one exit transition.
/// 3. Every state named by the FSM corresponds to an event name in the entity.
/// 4. Every event in the entity appears as a state in the FSM.
/// 5. Every state is reachable from the initial state.
/// 6. The exit state is reachable from every other state.
/// 7. A state on a cycle has [`Cardinality::Multi`], otherwise
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
    // Requirement 1: no event may be named "exit".
    for event in &entity.events {
        if event.name.to_ascii_lowercase() == "exit" {
            errors.push(FsmError::ReservedStateName {
                entity: entity.name.clone(),
                name: "exit",
            });
        }
    }

    // Requirement 2: there is at least one exit transition.
    if fsm.exit_from_states.is_empty() {
        errors.push(FsmError::NoExit {
            entity: entity.name.clone(),
        });
    }

    let event_names: HashSet<&Identifier> = entity.events.iter().map(|e| &e.name).collect();
    let cardinality_by_event: BTreeMap<&Identifier, Cardinality> = entity
        .events
        .iter()
        .map(|e| (&e.name, e.cardinality))
        .collect();

    // Gather every state named
    let states: HashSet<&Identifier> = std::iter::once(&fsm.initial_state)
        .chain(fsm.transitions.iter().flat_map(|t| [&t.source, &t.target]))
        .chain(&fsm.exit_from_states)
        .collect();

    // Requirement 3: every state name corresponds to an entity event name.
    for &state in &states {
        if !event_names.contains(state) {
            errors.push(FsmError::UnknownState {
                entity: entity.name.clone(),
                state: state.clone(),
            });
        }
    }

    // Requirement 4: every entity event appears as a state.
    for event in &entity.events {
        if !states.contains(&event.name) {
            errors.push(FsmError::UncoveredEvent {
                entity: entity.name.clone(),
                event: event.name.clone(),
            });
        }
    }

    // Graph of states + transitions
    let graph: DiGraphMap<GraphNode, ()> =
        std::iter::once((GraphNode::Init, GraphNode::Named(&fsm.initial_state), ()))
            .chain(
                fsm.transitions
                    .iter()
                    .map(|t| (GraphNode::Named(&t.source), GraphNode::Named(&t.target), ())),
            )
            .chain(
                fsm.exit_from_states
                    .iter()
                    .map(|x| (GraphNode::Named(x), GraphNode::Exit, ())),
            )
            .collect();

    // Requirement 5: every state is reachable from the initial state.
    let reachable_from_init: HashSet<GraphNode> =
        Bfs::new(&graph, GraphNode::Init).iter(&graph).collect();
    for &name in &states {
        if !reachable_from_init.contains(&GraphNode::Named(name)) {
            errors.push(FsmError::UnreachableFromInit {
                entity: entity.name.clone(),
                state: name.clone(),
            });
        }
    }

    // Requirement 6: every state can reach exit.
    // Skipped when there is no exit transition: req 2 already reports that, and
    // the check would otherwise flag every state.
    if !fsm.exit_from_states.is_empty() {
        let reversed = Reversed(&graph);
        let reaches_exit: HashSet<GraphNode> =
            Bfs::new(reversed, GraphNode::Exit).iter(reversed).collect();
        for &name in &states {
            if !reaches_exit.contains(&GraphNode::Named(name)) {
                errors.push(FsmError::CannotReachExit {
                    entity: entity.name.clone(),
                    state: name.clone(),
                });
            }
        }
    }

    // Requirement 7: a state on a cycle is Multi, otherwise Once.
    let on_cycle = find_cyclic(&graph, fsm);
    for &name in &states {
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
        if t.source == t.target {
            on_cycle.insert(&t.source);
        }
    }
    on_cycle
}

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
enum GraphNode<'a> {
    Init,
    Exit,
    Named(&'a Identifier),
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
    #[error("entity \"{entity}\" fsm: has no exit states")]
    NoExit { entity: Identifier },
    #[error("entity \"{entity}\" fsm: state \"{state}\" is unreachable from the intial state")]
    UnreachableFromInit {
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
    #[error("entity \"{entity}\" fsm: event \"{event}\" does not appear as a state")]
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
