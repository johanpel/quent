// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! # Convention trait and validation for [`Schema`]s.

use std::collections::{HashMap, hash_map::Entry};

use quent_schema::{Schema, convention::Convention as SchemaConvention, identifier::Identifier};
use thiserror::Error;

/// A trait for types that implement a "convention".
///
/// A convention is a set of properties that can be added to an application
/// event model. It is conveyed through opaque data attached to the constituents
/// of a [`Schema`].
///
/// By applying the convention to a model, it may obtain properties that need to
/// be validated against the entire schema, which is the main purpose of this
/// trait.
///
/// Conventions are leveraged for a wide variety of purposes. For more details,
/// see [`quent_schema`].
///
/// The canonical validation flow is orchestrated by the [`Validator`].
pub trait Convention {
    /// A unique name for this convention.
    ///
    /// While no restrictions are imposed on convention names (other than that
    /// they are valid UTF-8 strings) it is recommended to follow the
    /// human-readable dot-separated pattern `project.convention.version`. For
    /// example: `quent.fsm.v1`. This reduces the probability of name clashes
    /// between dependencies, and provides a means of easily detecting breaking
    /// changes to the convention's own schema.
    fn name(&self) -> &'static str;

    /// Validate this convention against `schema`.
    fn validate(&self, schema: &Schema) -> Result<(), Vec<Error>>;
}

/// Errors of [`Convention`] and [`Validator`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    #[error("duplicate registration of convention: \"{0}\"")]
    DuplicateConvention(&'static str),
    #[error(
        "element \"{owner}\" has convention \"{convention}\" requiring validation, but it is not registered"
    )]
    UnregisteredConvention {
        owner: Identifier,
        convention: String,
    },
    #[error("\"{convention}\" failed to validate with: {message}")]
    Validation { convention: String, message: String },
}

/// Validates registered [`Convention`]s.
///
/// Validation will fail when:
/// - a convention used by the schema requiring validation isn't registered by
///   the validator, or
/// - the constraints of the registered convention are unmet
///
/// # Example: validate in a `build.rs`
///
/// ```ignore
/// let validator = Validator::default()
///     .try_with(ConventionImplementing)?; // register a convention implemented elsewhere
///
/// if let Err(errors) = validator.run(&schema) {
///     panic!("schema validation failed: {errors:?}");
/// }
/// ```
#[derive(Default)]
pub struct Validator {
    conventions: HashMap<&'static str, Box<dyn Convention>>,
}

impl Validator {
    /// Register a [`Convention`] to be validated.
    pub fn try_with(mut self, convention: impl Convention + 'static) -> Result<Self, Error> {
        match self.conventions.entry(convention.name()) {
            Entry::Occupied(_) => Err(Error::DuplicateConvention(convention.name())),
            Entry::Vacant(entry) => {
                entry.insert(Box::new(convention));
                Ok(self)
            }
        }
    }

    /// Run validation of all registered conventions against `schema`.
    pub fn run(&self, schema: &Schema) -> Result<(), Vec<Error>> {
        let mut errors = Vec::new();
        // First, walk the entire schema to figure out if it uses any
        // unregistered conventions.
        check_conventions(
            &schema.conventions,
            &schema.name,
            &self.conventions,
            &mut errors,
        );
        for entity in &schema.entities {
            check_conventions(
                &entity.conventions,
                &entity.name,
                &self.conventions,
                &mut errors,
            );
            for event in &entity.events {
                check_conventions(
                    &event.conventions,
                    &entity.name,
                    &self.conventions,
                    &mut errors,
                );
                for field in &event.payload {
                    check_conventions(
                        &field.conventions,
                        &entity.name,
                        &self.conventions,
                        &mut errors,
                    );
                }
            }
        }
        for record in &schema.records {
            check_conventions(
                &record.conventions,
                &record.name,
                &self.conventions,
                &mut errors,
            );
            for field in &record.fields {
                check_conventions(
                    &field.conventions,
                    &record.name,
                    &self.conventions,
                    &mut errors,
                );
            }
        }

        // Second, validate
        for v in self.conventions.values() {
            if let Err(e) = v.validate(schema) {
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

fn check_conventions(
    conventions: &[SchemaConvention],
    owner: &Identifier,
    validators: &HashMap<&'static str, Box<dyn Convention>>,
    errors: &mut Vec<Error>,
) {
    for conv in conventions {
        if !conv.validated {
            continue;
        }
        if !validators.contains_key(conv.name.as_str()) {
            errors.push(Error::UnregisteredConvention {
                owner: owner.clone(),
                convention: conv.name.clone(),
            });
        }
    }
}
