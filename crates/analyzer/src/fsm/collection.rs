// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Collections of FSMs

use std::collections::HashSet;

use rustc_hash::FxHashMap as HashMap;
use uuid::Uuid;

use crate::{
    fsm::{Fsm, Transition},
    resource::{Usage, Using},
};

/// Trait for types that hold a collection of [`Fsm`]s of a single type.
///
/// An application with several FSM kinds unifies them under one [`Self::Fsm`]
/// type (e.g. an enum) so a single collection spans all of them.
pub trait FsmCollection {
    /// The FSM type held by this collection.
    type Fsm: Fsm;

    fn fsms(&self) -> impl Iterator<Item = &Self::Fsm>;
}

/// An in-memory collection of [`Fsm`]s.
pub struct InMemoryFsms<F, T>
where
    F: Fsm<TransitionType = T>,
    T: Transition,
{
    pub fsms: HashMap<Uuid, F>,
    pub fsm_type_names: HashSet<String>,
}

impl<F, T> FsmCollection for InMemoryFsms<F, T>
where
    F: Fsm<TransitionType = T>,
    T: Transition,
{
    type Fsm = F;

    fn fsms(&self) -> impl Iterator<Item = &F> {
        self.fsms.values()
    }
}

impl<F, T> Using for InMemoryFsms<F, T>
where
    F: Fsm<TransitionType = T> + Using,
    T: Transition,
{
    fn usages<'a>(&'a self) -> impl Iterator<Item = impl Usage<'a>> {
        self.fsms.values().flat_map(|fsm| fsm.usages())
    }
}

#[cfg(test)]
impl<F, T> InMemoryFsms<F, T>
where
    F: Fsm<TransitionType = T>,
    T: Transition,
{
    pub(crate) fn new() -> Self {
        Self {
            fsms: Default::default(),
            fsm_type_names: Default::default(),
        }
    }
    pub(crate) fn insert(&mut self, fsm: F) {
        self.fsm_type_names.insert(fsm.type_name().to_owned());
        self.fsms.insert(fsm.id(), fsm);
    }
}
