// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use petgraph::{
    graphmap::DiGraphMap,
    visit::{Bfs, Reversed, Walker},
};

use quent_v2_model_ir::{entity::Entity, fsm::State};

/// Validate the FSM topology of `entity`. Returns one message per violation.
/// Caller is expected to only invoke this when `entity.fsm` is `Some`.
pub fn validate(entity: &Entity) -> Result<(), Vec<String>> {
    let Some(fsm) = &entity.fsm else {
        return Err(vec![format!(
            "Entity {} does not qualify as Fsm: no FSM declared",
            entity.name
        )]);
    };

    let mut violations: Vec<String> = vec![];
    let mut violation = |reason: &str| {
        violations.push(format!(
            "Entity {} does not qualify as Fsm. {}",
            entity.name, reason
        ))
    };

    // state name must not be "entry"
    if entity
        .events
        .iter()
        .any(|e| e.name.eq_ignore_ascii_case("entry"))
    {
        violation("\"entry\" is a reserved state name")
    }
    // state name must not be "exit"
    if entity
        .events
        .iter()
        .any(|e| e.name.eq_ignore_ascii_case("exit"))
    {
        violation("\"exit\" is a reserved state name")
    }

    // exactly one transition into the entry state
    let num_entry_transitions = fsm
        .transitions
        .iter()
        .filter(|t| matches!(t.source, State::Entry))
        .count();
    if num_entry_transitions != 1 {
        violation(format!("entry transitions: {num_entry_transitions}, expected: 1").as_str());
    }

    // at least one exit transition
    let num_exit_transitions = fsm
        .transitions
        .iter()
        .filter(|t| matches!(t.target, State::Exit))
        .count();
    if num_exit_transitions == 0 {
        violation(format!("exit transitions: {num_exit_transitions}, expected: >= 1",).as_str());
    }

    let graph: DiGraphMap<FsmGraphNode, ()> = fsm
        .transitions
        .iter()
        .map(|t| {
            (
                FsmGraphNode::from_state(&t.source),
                FsmGraphNode::from_state(&t.target),
                (),
            )
        })
        .collect();
    let named_states: HashSet<&str> = fsm
        .transitions
        .iter()
        .flat_map(|t| [&t.source, &t.target])
        .filter_map(|s| match s {
            State::State(name) => Some(name.as_str()),
            _ => None,
        })
        .collect();

    let reachable_from_entry: HashSet<FsmGraphNode> =
        Bfs::new(&graph, FsmGraphNode::Entry).iter(&graph).collect();
    for name in &named_states {
        if !reachable_from_entry.contains(&FsmGraphNode::Named(name)) {
            violation(format!("state '{name}' is unreachable from entry").as_str());
        }
    }

    let reversed = Reversed(&graph);
    let states_reaching_exit: HashSet<FsmGraphNode> = Bfs::new(reversed, FsmGraphNode::Exit)
        .iter(reversed)
        .collect();
    for name in &named_states {
        if !states_reaching_exit.contains(&FsmGraphNode::Named(name)) {
            violation(format!("exit is unreachable from state '{name}'").as_str());
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum FsmGraphNode<'a> {
    Entry,
    Exit,
    Named(&'a str),
}

impl<'a> FsmGraphNode<'a> {
    fn from_state(state: &'a State) -> Self {
        match state {
            State::Entry => FsmGraphNode::Entry,
            State::Exit => FsmGraphNode::Exit,
            State::State(name) => FsmGraphNode::Named(name.as_str()),
        }
    }
}
