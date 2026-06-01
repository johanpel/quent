// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_constraints::{Constraint as _, Error as ValidatorError, Validator};
use quent_fsm::{Fsm, FsmConstraint, FsmError, State, Transition};
use quent_schema::{
    Schema,
    annotations::Annotations,
    constraint::Constraint,
    entity::Entity,
    event::{Cardinality, Event},
    identifier::Identifier,
};

fn ident(s: &str) -> Identifier {
    Identifier::try_new(s).unwrap()
}

fn event(name: &str, cardinality: Cardinality) -> Event {
    Event {
        name: ident(name),
        cardinality,
        payload: vec![],
        annotations: Annotations::default(),
    }
}

fn fsm_constraint(fsm: &Fsm) -> Constraint {
    Constraint {
        name: FsmConstraint::NAME.to_string(),
        data: Some(serde_json::to_string(fsm).unwrap()),
    }
}

fn entity_with(name: &str, events: Vec<Event>, fsm: &Fsm) -> Entity {
    Entity {
        name: ident(name),
        events,
        annotations: Annotations {
            constraints: vec![fsm_constraint(fsm)],
            ..Default::default()
        },
    }
}

fn schema_with(entity: Entity) -> Schema {
    Schema {
        name: ident("S"),
        entities: vec![entity],
        records: vec![],
        annotations: Annotations::default(),
    }
}

fn validate(schema: &Schema) -> Vec<FsmError> {
    match Validator::default()
        .try_with(FsmConstraint)
        .unwrap()
        .validate(schema)
    {
        Ok(()) => Vec::new(),
        Err(ValidatorError::Invalid { failures, .. }) => {
            let (_, source) = failures.into_iter().next().unwrap();
            match *source.downcast::<FsmError>().unwrap() {
                FsmError::Multiple(errors) => errors,
                single => vec![single],
            }
        }
        Err(_) => unreachable!(),
    }
}

fn linear_transitions(named: &[&str]) -> Vec<Transition> {
    let mut ts = Vec::new();
    let mut prev = State::Entry;
    for n in named {
        let curr = State::Named(ident(n));
        ts.push(Transition {
            source: prev,
            target: curr.clone(),
        });
        prev = curr;
    }
    ts.push(Transition {
        source: prev,
        target: State::Exit,
    });
    ts
}

#[test]
fn well_formed_linear_fsm_passes() {
    let fsm = Fsm {
        transitions: linear_transitions(&["a", "b"]),
    };
    let entity = entity_with(
        "E",
        vec![event("a", Cardinality::Once), event("b", Cardinality::Once)],
        &fsm,
    );
    assert!(validate(&schema_with(entity)).is_empty());
}

#[test]
fn well_formed_self_loop_fsm_passes() {
    let fsm = Fsm {
        transitions: vec![
            Transition {
                source: State::Entry,
                target: State::Named(ident("a")),
            },
            Transition {
                source: State::Named(ident("a")),
                target: State::Named(ident("a")),
            },
            Transition {
                source: State::Named(ident("a")),
                target: State::Exit,
            },
        ],
    };
    let entity = entity_with("E", vec![event("a", Cardinality::Multi)], &fsm);
    assert!(validate(&schema_with(entity)).is_empty());
}

#[test]
fn missing_data_is_rejected() {
    let entity = Entity {
        name: ident("E"),
        events: vec![event("a", Cardinality::Once)],
        annotations: Annotations {
            constraints: vec![Constraint {
                name: FsmConstraint::NAME.to_string(),
                data: None,
            }],
            ..Default::default()
        },
    };
    let errors = validate(&schema_with(entity));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, FsmError::InvalidData { .. }))
    );
}

#[test]
fn invalid_json_is_rejected() {
    let entity = Entity {
        name: ident("E"),
        events: vec![event("a", Cardinality::Once)],
        annotations: Annotations {
            constraints: vec![Constraint {
                name: FsmConstraint::NAME.to_string(),
                data: Some("{ trash".to_string()),
            }],
            ..Default::default()
        },
    };
    let errors = validate(&schema_with(entity));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, FsmError::InvalidData { .. })),
    );
}

#[test]
fn fsm_with_no_named_states_is_rejected() {
    // this would appear in an event stream as some freak kind of stateless
    // single-event FSM, so reject it.
    let fsm = Fsm {
        transitions: vec![Transition {
            source: State::Entry,
            target: State::Exit,
        }],
    };
    let entity = entity_with("E", vec![], &fsm);
    let errors = validate(&schema_with(entity));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, FsmError::NoNamedStates { .. })),
    );
}

#[test]
fn reserved_name_entry_is_rejected() {
    let fsm = Fsm {
        transitions: linear_transitions(&["a"]),
    };
    let entity = entity_with(
        "E",
        vec![
            event("Entry", Cardinality::Once),
            event("a", Cardinality::Once),
        ],
        &fsm,
    );
    let errors = validate(&schema_with(entity));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, FsmError::ReservedStateName { name: "entry", .. })),
    );
}

#[test]
fn reserved_name_exit_is_rejected() {
    let fsm = Fsm {
        transitions: linear_transitions(&["a"]),
    };
    let entity = entity_with(
        "E",
        vec![
            event("EXIT", Cardinality::Once),
            event("a", Cardinality::Once),
        ],
        &fsm,
    );
    let errors = validate(&schema_with(entity));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, FsmError::ReservedStateName { name: "exit", .. })),
    );
}

#[test]
fn no_entry_transition_is_rejected() {
    let fsm = Fsm {
        transitions: vec![Transition {
            source: State::Named(ident("a")),
            target: State::Exit,
        }],
    };
    let entity = entity_with("E", vec![event("a", Cardinality::Once)], &fsm);
    let errors = validate(&schema_with(entity));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, FsmError::WrongEntryCount { found: 0, .. })),
    );
    // The missing entry transition must not cascade into per-state unreachability.
    assert!(
        !errors
            .iter()
            .any(|e| matches!(e, FsmError::UnreachableFromEntry { .. })),
    );
}

#[test]
fn two_entry_transitions_are_rejected() {
    let fsm = Fsm {
        transitions: vec![
            Transition {
                source: State::Entry,
                target: State::Named(ident("a")),
            },
            Transition {
                source: State::Entry,
                target: State::Named(ident("b")),
            },
            Transition {
                source: State::Named(ident("a")),
                target: State::Exit,
            },
            Transition {
                source: State::Named(ident("b")),
                target: State::Exit,
            },
        ],
    };
    let entity = entity_with(
        "E",
        vec![event("a", Cardinality::Once), event("b", Cardinality::Once)],
        &fsm,
    );
    let errors = validate(&schema_with(entity));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, FsmError::WrongEntryCount { found: 2, .. })),
    );
}

#[test]
fn no_exit_transition_is_rejected() {
    let fsm = Fsm {
        transitions: vec![Transition {
            source: State::Entry,
            target: State::Named(ident("a")),
        }],
    };
    let entity = entity_with("E", vec![event("a", Cardinality::Once)], &fsm);
    let errors = validate(&schema_with(entity));
    assert!(errors.iter().any(|e| matches!(e, FsmError::NoExit { .. })),);
    // if exit is missing just report that
    assert!(
        !errors
            .iter()
            .any(|e| matches!(e, FsmError::CannotReachExit { .. })),
    );
}

#[test]
fn transition_into_entry_is_rejected() {
    let fsm = Fsm {
        transitions: vec![
            Transition {
                source: State::Entry,
                target: State::Named(ident("a")),
            },
            Transition {
                source: State::Named(ident("a")),
                target: State::Entry,
            },
            Transition {
                source: State::Named(ident("a")),
                target: State::Exit,
            },
        ],
    };
    let entity = entity_with("E", vec![event("a", Cardinality::Multi)], &fsm);
    let errors = validate(&schema_with(entity));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, FsmError::TransitionIntoEntry { .. })),
    );
}

#[test]
fn transition_out_of_exit_is_rejected() {
    let fsm = Fsm {
        transitions: vec![
            Transition {
                source: State::Entry,
                target: State::Named(ident("a")),
            },
            Transition {
                source: State::Named(ident("a")),
                target: State::Exit,
            },
            Transition {
                source: State::Exit,
                target: State::Named(ident("a")),
            },
        ],
    };
    let entity = entity_with("E", vec![event("a", Cardinality::Multi)], &fsm);
    let errors = validate(&schema_with(entity));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, FsmError::TransitionOutOfExit { .. })),
    );
}

#[test]
fn state_unreachable_from_entry_is_rejected() {
    // b is named in transitions but only connects to itself and exit.
    let fsm = Fsm {
        transitions: vec![
            Transition {
                source: State::Entry,
                target: State::Named(ident("a")),
            },
            Transition {
                source: State::Named(ident("a")),
                target: State::Exit,
            },
            Transition {
                source: State::Named(ident("b")),
                target: State::Exit,
            },
        ],
    };
    let entity = entity_with(
        "E",
        vec![event("a", Cardinality::Once), event("b", Cardinality::Once)],
        &fsm,
    );
    let errors = validate(&schema_with(entity));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, FsmError::UnreachableFromEntry { state, .. } if state == "b")),
    );
}

#[test]
fn state_cannot_reach_exit_is_rejected() {
    let fsm = Fsm {
        transitions: vec![
            Transition {
                source: State::Entry,
                target: State::Named(ident("a")),
            },
            Transition {
                source: State::Named(ident("a")),
                target: State::Named(ident("b")),
            },
            // b never reaches exit
            Transition {
                source: State::Named(ident("a")),
                target: State::Exit,
            },
        ],
    };
    let entity = entity_with(
        "E",
        vec![event("a", Cardinality::Once), event("b", Cardinality::Once)],
        &fsm,
    );
    let errors = validate(&schema_with(entity));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, FsmError::CannotReachExit { state, .. } if state == "b")),
    );
}

#[test]
fn fsm_state_not_in_events_is_rejected() {
    let fsm = Fsm {
        transitions: linear_transitions(&["phantom"]),
    };
    let entity = entity_with("E", vec![event("a", Cardinality::Once)], &fsm);
    let errors = validate(&schema_with(entity));
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, FsmError::UnknownState { state, .. } if
            state == "phantom")),
    );
}

// TODO(johanpel): consider allowing FSMs to have freestanding events
#[test]
fn event_not_covered_by_fsm_is_rejected() {
    let fsm = Fsm {
        transitions: linear_transitions(&["a"]),
    };
    // dead is declared but never referenced by any transition.
    let entity = entity_with(
        "E",
        vec![
            event("a", Cardinality::Once),
            event("dead", Cardinality::Once),
        ],
        &fsm,
    );
    let errors = validate(&schema_with(entity));
    assert!(errors.iter().any(|e| matches!(e,
    FsmError::UncoveredEvent { event, .. } if event == "dead")),);
}

#[test]
fn cycle_requires_multi_cardinality() {
    let fsm = Fsm {
        transitions: vec![
            Transition {
                source: State::Entry,
                target: State::Named(ident("a")),
            },
            Transition {
                source: State::Named(ident("a")),
                target: State::Named(ident("a")),
            },
            Transition {
                source: State::Named(ident("a")),
                target: State::Exit,
            },
        ],
    };
    let entity = entity_with("E", vec![event("a", Cardinality::Once)], &fsm);
    let errors = validate(&schema_with(entity));
    assert!(errors.iter().any(|e| matches!(
        e,
        FsmError::CardinalityMismatch {
            expected: Cardinality::Multi,
            found: Cardinality::Once,
            ..
        }
    )),);
}

#[test]
fn acyclic_requires_once_cardinality() {
    let fsm = Fsm {
        transitions: linear_transitions(&["a"]),
    };
    let entity = entity_with("E", vec![event("a", Cardinality::Multi)], &fsm);
    let errors = validate(&schema_with(entity));
    assert!(errors.iter().any(|e| matches!(
        e,
        FsmError::CardinalityMismatch {
            expected: Cardinality::Once,
            found: Cardinality::Multi,
            ..
        }
    )),);
}

#[test]
fn scc_of_size_two_requires_multi_for_both_states() {
    let fsm = Fsm {
        transitions: vec![
            Transition {
                source: State::Entry,
                target: State::Named(ident("a")),
            },
            Transition {
                source: State::Named(ident("a")),
                target: State::Named(ident("b")),
            },
            Transition {
                source: State::Named(ident("b")),
                target: State::Named(ident("a")),
            },
            Transition {
                source: State::Named(ident("b")),
                target: State::Exit,
            },
        ],
    };

    // a and b should actually be multi, so this should not validate
    let entity = entity_with(
        "E",
        vec![event("a", Cardinality::Once), event("b", Cardinality::Once)],
        &fsm,
    );
    let errors = validate(&schema_with(entity));
    assert!(errors.iter().any(|e| matches!(
        e,
        FsmError::CardinalityMismatch { state, expected: Cardinality::Multi, .. }
            if state == "a"
    )),);
    assert!(errors.iter().any(|e| matches!(
        e,
        FsmError::CardinalityMismatch { state, expected: Cardinality::Multi, .. }
            if state == "b"
    )),);

    // make them multi to make it pass
    let entity = entity_with(
        "E",
        vec![
            event("a", Cardinality::Multi),
            event("b", Cardinality::Multi),
        ],
        &fsm,
    );
    assert!(validate(&schema_with(entity)).is_empty());
}

#[test]
fn entity_without_fsm_constraint_is_ignored() {
    let entity = Entity {
        name: ident("E"),
        events: vec![event("a", Cardinality::Once)],
        annotations: Annotations::default(),
    };
    assert!(validate(&schema_with(entity)).is_empty());
}
