// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_constraints::Constraint;
use quent_schema::builder::{AnnotationsBuilder, BuilderError, EntityBuilder, EventBuilder};
use quent_schema::{Cardinality, Entity, Field, Identifier};
use thiserror::Error;

use crate::{ExitStates, Fsm, FsmConstraint, Transition};

/// One state of an FSM entity: its name, the event payload it carries, and its
/// outgoing transitions and role flags.
pub struct StateDecl {
    /// State name, used verbatim as the state event's name.
    pub name: Identifier,
    /// Fields of the state event.
    pub attributes: Vec<Field>,
    /// States this state transitions to.
    pub to: Vec<Identifier>,
    /// Whether the FSM begins in this state.
    pub initial: bool,
    /// Whether the FSM may exit from this state.
    pub exit: bool,
}

/// Builds an FSM [`Entity`] from its states, so any frontend gets the same
/// state-to-event lowering.
///
/// Each state becomes one event whose cardinality is derived from the topology
/// (a state on a cycle is [`Cardinality::Multi`], otherwise [`Cardinality::Once`]).
/// The FSM topology is attached as the FSM constraint. Whether the topology is
/// valid (reachability, exits, event coverage) is checked separately by
/// [`crate::FsmConstraint`] during schema validation.
pub struct FsmEntityBuilder {
    id: Identifier,
    annotations: AnnotationsBuilder,
    states: Vec<StateDecl>,
}

/// A structural problem that prevents building an FSM entity at all, before any
/// topology validation.
#[derive(Debug, Error)]
pub enum FsmShapeError {
    /// No state was marked initial.
    #[error("no state is marked as the initial state")]
    NoInitialState,
    /// More than one state was marked initial.
    #[error("more than one state is marked as the initial state")]
    MultipleInitialStates(Vec<Identifier>),
    /// No state was marked as an exit.
    #[error("no state is marked as an exit state")]
    NoExitState,
    /// A duplicate attribute name within a state reached the schema builder.
    #[error(transparent)]
    Build(#[from] BuilderError),
    /// The FSM topology failed to serialize to its constraint payload.
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
}

impl FsmEntityBuilder {
    /// Begin an FSM entity named `id`, carrying `annotations` (to which the FSM
    /// constraint is added on [`Self::build`]).
    pub fn new(id: Identifier, annotations: AnnotationsBuilder) -> Self {
        Self {
            id,
            annotations,
            states: Vec::new(),
        }
    }

    /// Add a state.
    ///
    /// # Errors
    ///
    /// Errors if its name is already declared.
    pub fn try_insert_state(&mut self, state: StateDecl) -> Result<&mut Self, BuilderError> {
        if self.states.iter().any(|s| s.name == state.name) {
            return Err(BuilderError::DuplicateName(state.name.to_string()));
        }
        self.states.push(state);
        Ok(self)
    }

    /// Add a state, returning the builder for chaining.
    ///
    /// # Errors
    ///
    /// Errors if its name is already declared.
    pub fn try_with_state(mut self, state: StateDecl) -> Result<Self, BuilderError> {
        self.try_insert_state(state)?;
        Ok(self)
    }

    /// Add several states, returning the builder for chaining.
    ///
    /// # Errors
    ///
    /// Errors on the first duplicate name.
    pub fn try_with_states(
        mut self,
        states: impl IntoIterator<Item = StateDecl>,
    ) -> Result<Self, BuilderError> {
        for state in states {
            self.try_insert_state(state)?;
        }
        Ok(self)
    }

    /// Assemble the entity: derive each state event's cardinality from the
    /// topology, attach the events and the FSM constraint.
    ///
    /// # Errors
    ///
    /// Returns [`FsmShapeError`] if there is not exactly one initial state, no
    /// exit state, a state has a duplicate attribute name, or the topology fails
    /// to serialize.
    pub fn build(self) -> Result<Entity, FsmShapeError> {
        let Self {
            id,
            mut annotations,
            states,
        } = self;

        let initials: Vec<Identifier> = states
            .iter()
            .filter(|s| s.initial)
            .map(|s| s.name.clone())
            .collect();
        let initial = match initials.as_slice() {
            [one] => one.clone(),
            [] => return Err(FsmShapeError::NoInitialState),
            _ => return Err(FsmShapeError::MultipleInitialStates(initials)),
        };

        let exits: Vec<Identifier> = states
            .iter()
            .filter(|s| s.exit)
            .map(|s| s.name.clone())
            .collect();
        let Some((first_exit, other_exits)) = exits.split_first() else {
            return Err(FsmShapeError::NoExitState);
        };

        let transitions: Vec<Transition> = states
            .iter()
            .flat_map(|state| {
                let source = state.name.clone();
                state
                    .to
                    .iter()
                    .map(move |target| Transition::new(source.clone(), target.clone()))
            })
            .collect();
        let fsm = Fsm::new(
            initial,
            transitions,
            ExitStates::new(first_exit.clone(), other_exits.to_vec()),
        );

        let mut entity = EntityBuilder::new(id);
        for state in states {
            // An isolated state has no place on the topology; the FSM constraint
            // reports it, so `Once` here is only a stand-in.
            let cardinality = fsm.cardinality(&state.name).unwrap_or(Cardinality::Once);
            let event = EventBuilder::new(state.name, cardinality)
                .try_with_fields(state.attributes)?
                .build();
            entity = entity.try_with_event(event)?;
        }

        annotations.set_constraint(FsmConstraint::NAME, Some(fsm.constraint_data()?));
        Ok(entity.with_annotations(annotations.build()).build())
    }
}
