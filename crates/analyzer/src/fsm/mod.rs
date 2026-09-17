// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! FSM analysis interfaces and storage implementations.

use quent_time::{Timestamp, span::SpanUnixNanoSec};

use crate::{Entity, resource::Usage};

pub mod collection;
pub mod native;
pub mod runtime;

/// Trait for types that represent an [`Fsm`] state transition event.
pub trait Transition: Timestamp {
    /// Return the unique name of the state this transition leads to.
    fn name(&self) -> &str;

    /// Returns the per-entity ordering key for equal timestamps.
    fn sequence(&self) -> u16;

    /// Returns whether this transition ends the FSM's dynamic lifetime.
    fn is_final(&self) -> bool;
}

/// Trait for types that represent a Finite State Machine (FSM).
///
/// An FSM is modeled as a sequence of transitions between uniquely named
/// states. Each FSM must have at least two transition, some entry transition
/// and an exit transition. The number of states is always one less than the
/// number of transitions.
pub trait Fsm: Entity {
    /// The type of transitions stored by this FSM.
    ///
    /// This associated type enables dyn-free access to underlying transition
    /// data.
    type TransitionType: Transition;

    /// Return the number of states in this FSM.
    ///
    /// Each state spans two consecutive transitions, so this is always one less
    /// than the number of transitions.
    fn len(&self) -> usize;

    /// Return true if this FSM has no states (meaning the model of whatever it
    /// represents is incomplete).
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return a reference to the transition at the given index.
    ///
    /// Returns `None` if the index is out of bounds.
    fn transition(&self, index: usize) -> Option<&Self::TransitionType>;

    /// Return a reference to the state at the given index.
    ///
    /// The state spans from transition `index` to transition `index + 1`.
    /// Returns `None` if the index is out of bounds.
    fn state<'a>(&'a self, index: usize) -> Option<FsmStateRef<'a, Self, Self::TransitionType>> {
        (self.len() > index).then_some(FsmStateRef { fsm: self, index })
    }

    /// Return an iterator over all states in this FSM.
    fn states<'a>(
        &'a self,
    ) -> impl ExactSizeIterator<Item = FsmStateRef<'a, Self, Self::TransitionType>> {
        (0..self.len()).map(|index| self.state(index).unwrap())
    }

    /// Return the first state, if the FSM is not empty.
    fn first<'a>(&'a self) -> Option<FsmStateRef<'a, Self, Self::TransitionType>> {
        self.state(0)
    }

    /// Return the last state, if the FSM is not empty.
    fn last<'a>(&'a self) -> Option<FsmStateRef<'a, Self, Self::TransitionType>> {
        self.state(self.len() - 1)
    }
}

/// Trait for FSMs that have resource usages associated with their states.
pub trait FsmUsages<'a>: Fsm {
    /// Return an iterator over all usages with their associated state names.
    fn usages_with_state_names(&'a self) -> impl Iterator<Item = (&'a str, impl Usage<'a>)>;
}

#[derive(Clone)]
pub struct FsmStateRef<'a, F, T>
where
    F: Fsm<TransitionType = T> + ?Sized,
    T: Transition,
{
    fsm: &'a F,
    index: usize,
}

impl<'a, F, T> FsmStateRef<'a, F, T>
where
    F: Fsm<TransitionType = T>,
    T: Transition,
{
    pub fn name(&self) -> &str {
        self.fsm.transition(self.index).unwrap().name()
    }

    pub fn span(&self) -> SpanUnixNanoSec {
        let start = self.fsm.transition(self.index).unwrap().timestamp();
        let end = self.fsm.transition(self.index + 1).unwrap().timestamp();
        SpanUnixNanoSec::try_new(start, end).unwrap()
    }
}
