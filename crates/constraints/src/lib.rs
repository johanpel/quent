// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! # Constraint trait and validation for [`Schema`]s.

pub mod utils;

use std::collections::{BTreeSet, HashSet};

use quent_schema::{
    Schema,
    visitor::{Cursor, Element, IndexedSchema, Visitor},
};

/// A trait for types that implement a "constraint" of an application event
/// model.
///
/// A constraint is a rule imposed on an application event model. It is conveyed
/// through opaque data attached to the constituents of a [`Schema`] as
/// [`quent_schema::constraint::Constraint`]s.
///
/// By applying the constraint to a model, the model gains properties that need
/// to be validated against the entire schema, which is the main purpose of this
/// trait.
///
/// Constraints are leveraged for a wide variety of purposes. For more details,
/// see [`quent_schema`].
///
/// The canonical validation flow is orchestrated by [`validate`].
pub trait Constraint: Visitor + Default {
    /// A unique name for this constraint.
    ///
    /// While no restrictions are imposed on constraint names (other than that
    /// they are valid UTF-8 strings) it is recommended to follow the
    /// human-readable dot-separated pattern `project.constraint.version`. For
    /// example: `quent.fsm.v1`. This reduces the probability of name clashes
    /// between dependencies, and provides a means of easily detecting breaking
    /// changes to the constraint's own schema.
    const NAME: &'static str;
}

/// The outcome of [`validate`].
pub struct Report<R> {
    /// Constraint names referenced by the schema that no validated constraint
    /// handles.
    pub unregistered: BTreeSet<String>,
    /// Each constraint's own result, in tuple order matching the validated
    /// constraints.
    pub results: R,
}

/// Validates (a tuple of) [`Constraint`]s against `schema`.
///
/// ```ignore
/// let { unregistered, (some_result, other_result) } = validate::<(SomeConstraint, OtherConstraint)>(&schema);
/// some_result?;
/// other_result?;
/// assert!(report.unregistered.is_empty());
/// ```
pub fn validate<C: Constraints + Default>(schema: &Schema) -> Report<C::Output> {
    let registered: HashSet<&'static str> = C::names().into_iter().collect();
    let (unregistered, results) = schema.walk(ConstraintScan {
        registered: &registered,
        unregistered: BTreeSet::new(),
        inner: C::default(),
    });
    Report {
        unregistered,
        results,
    }
}

/// A tuple of [`Constraint`]s that can be validated together in one walk.
///
/// Implemented for tuples of constraints; [`names`](Constraints::names) collects
/// their [`Constraint::NAME`]s so [`validate`] can tell which constraints the
/// schema references but no member handles.
pub trait Constraints: Visitor {
    /// The [`Constraint::NAME`] of every constraint in the tuple.
    fn names() -> Vec<&'static str>;
}

macro_rules! constraints_impls {
    ($($T:ident),+) => {
        impl<$($T: Constraint),+> Constraints for ($($T,)+) {
            fn names() -> Vec<&'static str> {
                vec![$($T::NAME),+]
            }
        }
    };
}
constraints_impls!(A);
constraints_impls!(A, B);
constraints_impls!(A, B, C);
constraints_impls!(A, B, C, D);
constraints_impls!(A, B, C, D, E);
constraints_impls!(A, B, C, D, E, F);
constraints_impls!(A, B, C, D, E, F, G);
constraints_impls!(A, B, C, D, E, F, G, H);
constraints_impls!(A, B, C, D, E, F, G, H, I);
constraints_impls!(A, B, C, D, E, F, G, H, I, J);
constraints_impls!(A, B, C, D, E, F, G, H, I, J, K);
constraints_impls!(A, B, C, D, E, F, G, H, I, J, K, L);

/// Utility visitor wrapping around constraints to collect any unregistered
/// constraint names.
struct ConstraintScan<'a, C> {
    registered: &'a HashSet<&'static str>,
    unregistered: BTreeSet<String>,
    inner: C,
}

impl<C: Visitor> Visitor for ConstraintScan<'_, C> {
    type Output = (BTreeSet<String>, C::Output);
    fn visit(&mut self, cursor: &Cursor, index: &IndexedSchema) {
        if let Element::Annotations(annotations) = cursor.current() {
            for constraint in &annotations.constraints {
                if !self.registered.contains(constraint.name.as_str()) {
                    self.unregistered.insert(constraint.name.clone());
                }
            }
        }
        self.inner.visit(cursor, index);
    }
    fn finish(self) -> Self::Output {
        (self.unregistered, self.inner.finish())
    }
}
