// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! # Constraint trait and validation for [`Schema`]s.

use std::collections::{HashMap, hash_map::Entry};

use quent_schema::{Schema, constraint::Constraint as SchemaConstraint, identifier::Identifier};
use thiserror::Error;

/// A trait for types that implement a "constraint".
///
/// A constraint is a rule imposed on an application event model. It is conveyed
/// through opaque data attached to the constituents of a [`Schema`] as
/// [`SchemaConstraint`]s.
///
/// By applying the constraint to a model, the model gains properties that need
/// to be validated against the entire schema, which is the main purpose of this
/// trait.
///
/// Constraints are leveraged for a wide variety of purposes. For more details,
/// see [`quent_schema`].
///
/// The canonical validation flow is orchestrated by the [`Validator`].
pub trait Constraint {
    /// A unique name for this constraint.
    ///
    /// While no restrictions are imposed on constraint names (other than that
    /// they are valid UTF-8 strings) it is recommended to follow the
    /// human-readable dot-separated pattern `project.constraint.version`. For
    /// example: `quent.fsm.v1`. This reduces the probability of name clashes
    /// between dependencies, and provides a means of easily detecting breaking
    /// changes to the constraint's own schema.
    const NAME: &'static str;

    /// Validate this constraint against `schema`.
    fn validate(&self, schema: &Schema) -> Result<(), Vec<Error>>;
}

/// Errors of [`Constraint`] and [`Validator`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    #[error("duplicate registration of constraint: \"{0}\"")]
    DuplicateConstraint(&'static str),
    #[error(
        "element \"{owner}\" has constraint \"{constraint}\" requiring validation, but it is not registered"
    )]
    UnregisteredConstraint {
        owner: Identifier,
        constraint: String,
    },
    #[error("\"{constraint}\" failed to validate with: {message}")]
    Validation { constraint: String, message: String },
}

type ConstraintFn = Box<dyn Fn(&Schema) -> Result<(), Vec<Error>>>;

/// Validates registered [`Constraint`]s.
///
/// Validation will fail when:
/// - a constraint used by the schema isn't registered by the validator, or
/// - the rule of a registered constraint is unmet
///
/// # Example: validate in a `build.rs`
///
/// ```ignore
/// let validator = Validator::default()
///     .try_with(MyConstraint)?; // register a constraint implemented elsewhere
///
/// if let Err(errors) = validator.validate(&schema) {
///     panic!("schema validation failed: {errors:?}");
/// }
/// ```
#[derive(Default)]
pub struct Validator {
    constraints: HashMap<&'static str, ConstraintFn>,
}

impl Validator {
    /// Register a [`Constraint`] to be validated.
    pub fn try_with<C: Constraint + 'static>(mut self, constraint: C) -> Result<Self, Error> {
        match self.constraints.entry(C::NAME) {
            Entry::Occupied(_) => Err(Error::DuplicateConstraint(C::NAME)),
            Entry::Vacant(entry) => {
                entry.insert(Box::new(move |schema: &Schema| constraint.validate(schema)));
                Ok(self)
            }
        }
    }

    /// Run validation of all registered constraints against `schema`.
    pub fn validate(&self, schema: &Schema) -> Result<(), Vec<Error>> {
        let mut errors = Vec::new();
        // First, walk the entire schema to figure out if it uses any
        // unregistered constraints.
        check_constraints(
            &schema.annotations.constraints,
            &schema.name,
            &self.constraints,
            &mut errors,
        );
        for entity in &schema.entities {
            check_constraints(
                &entity.annotations.constraints,
                &entity.name,
                &self.constraints,
                &mut errors,
            );
            for event in &entity.events {
                check_constraints(
                    &event.annotations.constraints,
                    &entity.name,
                    &self.constraints,
                    &mut errors,
                );
                for field in &event.payload {
                    check_constraints(
                        &field.annotations.constraints,
                        &entity.name,
                        &self.constraints,
                        &mut errors,
                    );
                }
            }
        }
        for record in &schema.records {
            check_constraints(
                &record.annotations.constraints,
                &record.name,
                &self.constraints,
                &mut errors,
            );
            for field in &record.fields {
                check_constraints(
                    &field.annotations.constraints,
                    &record.name,
                    &self.constraints,
                    &mut errors,
                );
            }
        }

        // Second, validate
        for validate in self.constraints.values() {
            if let Err(e) = validate(schema) {
                errors.extend(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn check_constraints(
    constraints: &[SchemaConstraint],
    owner: &Identifier,
    validators: &HashMap<&'static str, ConstraintFn>,
    errors: &mut Vec<Error>,
) {
    for constraint in constraints {
        if !validators.contains_key(constraint.name.as_str()) {
            errors.push(Error::UnregisteredConstraint {
                owner: owner.clone(),
                constraint: constraint.name.clone(),
            });
        }
    }
}
