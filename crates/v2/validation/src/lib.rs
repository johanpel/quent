// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Validation for the Quent v2 IR.
//!
//! Provides the [`Convention`] trait, [`ValidatorRegistry`], and the
//! [`ValidationError`] enum. Core IR rules (FSM topology, identifier grammar)
//! are always installed; opt-in conventions (e.g. `Resource`) register
//! through `ValidatorRegistry::with::<C>()`.

use std::fmt;

use quent_v2_model_ir::{Model, convention::Convention as IrConvention, identifier::Identifier};

pub mod fsm;

/// A convention is an opt-in, named cross-cutting validation rule whose data
/// is attached as opaque per-element JSON strings under
/// `conventions[Self::NAME]` on entities, events, event fields, records, or
/// fields. Implementations parse their own data and emit
/// [`ValidationError`]s.
pub trait Convention {
    const NAME: &'static str;
    fn validate(model: &Model) -> Result<(), Vec<ValidationError>>;
}

/// Signature of a validator registered in a [`ValidatorRegistry`].
type ValidatorFn = fn(&Model) -> Result<(), Vec<ValidationError>>;

/// Registry of convention validators. Always includes the core validator
/// (FSM topology + identifier grammar). Additional conventions are added via
/// [`ValidatorRegistry::with`].
pub struct ValidatorRegistry {
    known: Vec<&'static str>,
    validators: Vec<ValidatorFn>,
}

impl Default for ValidatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidatorRegistry {
    pub fn new() -> Self {
        Self {
            known: vec![],
            validators: vec![core_validate],
        }
    }

    pub fn with<C: Convention>(mut self) -> Self {
        if self.known.contains(&C::NAME) {
            panic!("convention '{}' already registered", C::NAME);
        }
        self.known.push(C::NAME);
        self.validators.push(C::validate);
        self
    }

    pub fn run(self, model: &Model) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Every Validated convention key present in the model must have a
        // registered validator. Metadata entries are skipped.
        check_conventions(&model.conventions, &model.name, &self.known, &mut errors);
        for entity in &model.entities {
            check_conventions(&entity.conventions, &entity.name, &self.known, &mut errors);
            for event in &entity.events {
                check_conventions(&event.conventions, &entity.name, &self.known, &mut errors);
                for field in &event.payload {
                    check_conventions(&field.conventions, &entity.name, &self.known, &mut errors);
                }
            }
        }
        for record in &model.records {
            check_conventions(&record.conventions, &record.name, &self.known, &mut errors);
            for field in &record.fields {
                check_conventions(&field.conventions, &record.name, &self.known, &mut errors);
            }
        }

        for v in &self.validators {
            if let Err(e) = v(model) {
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
    conventions: &[IrConvention],
    owner: &Identifier,
    known: &[&'static str],
    errors: &mut Vec<ValidationError>,
) {
    for conv in conventions {
        if !conv.validated {
            continue;
        }
        if !known.contains(&conv.name.as_str()) {
            errors.push(ValidationError::UnregisteredConvention {
                owner: owner.clone(),
                convention: conv.name.clone(),
            });
        }
    }
}

/// Core validator: runs identifier-grammar-free, topology-only checks on
/// every entity that declares an FSM.
fn core_validate(model: &Model) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    for entity in &model.entities {
        if entity.fsm.is_some()
            && let Err(msgs) = fsm::validate(entity)
        {
            for m in msgs {
                errors.push(ValidationError::Other(m));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Errors produced by [`ValidatorRegistry::run`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Some element in the model is tagged with a convention name that has
    /// no validator registered in the [`ValidatorRegistry`].
    UnregisteredConvention {
        owner: Identifier,
        convention: String,
    },
    /// A convention validator produced an error.
    ConventionError {
        convention: Identifier,
        message: String,
    },
    /// A core (built-in) validation error: e.g. FSM topology, identifier
    /// grammar.
    Other(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::UnregisteredConvention { owner, convention } => write!(
                f,
                "element '{owner}' uses unregistered convention '{convention}'",
            ),
            ValidationError::ConventionError {
                convention,
                message,
            } => write!(f, "[{convention}] {message}"),
            ValidationError::Other(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for ValidationError {}
