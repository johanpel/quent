//! Intermediate Representation of an application model.
//!
//! This module holds definitions for an Intermediate Representation (IR) of
//! what an application model looks like in-memory during code generation.
//!
//! An IR is populated through one of two paths:
//!
//! # 1. From Rust source with derive macros
//!
//! When an application model is declared by writing Rust types with the model
//! derive macros (`Attributes`, `Entity`, etc.), the derives emit `Model*`
//! trait implementations on the user's types. Calling those trait methods at
//! build time yields the IR. The generated instrumentation API references the
//! original types wherever possible.
//!
//! # 2. From an external source via a parser
//!
//! When an application model is declared elsewhere (e.g. from a serialized form
//! of the IR or a DSL source file) a parser populates the IR directly. From
//! that IR, the necessary Rust types annotated with derive macros can be
//! generated and compiled. From here, goto path 1.
//!
//! Consumers of the IR:
//! - the Rust instrumentation API generator,
//! - cross-language bridge generators for C++ and Python
//!
use std::collections::HashMap;

pub mod attributes;
pub mod entity;
pub mod event;
pub mod qualifications;
pub mod value_type;

pub struct Model {
    pub name: String,
    pub entities: HashMap<String, entity::Entity>,
    pub attributes: HashMap<String, attributes::Attributes>,
}
