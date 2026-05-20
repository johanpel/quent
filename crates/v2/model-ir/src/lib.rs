//! Intermediate Representation of an application model.
//!
//! This module holds definitions for an Intermediate Representation (IR) of
//! what an application model looks like in-memory. This module also holds
//! traits that types can implement to emit their IR. This is useful for
//! validation in proc macros as well as generating wrappers for other target
//! language bindings.
//!
//! Validation involves checking certain requirements, e.g.:
//! - Identifiers are accepted by the specified identifier grammar (see docs).
//! - All FSM states are reachable from the entry transition and an exit
//!   transition is reachable from all states.
//! - Resource groups have an event declaring their parent.
//!
//! Rust's powerful type system is used to enforce as many requirements as
//! possible, as long as the model declaration stays compact and straightforward
//! for users. Some constraints could in principle be encoded in types too, but
//! they would be very awkward to encode (e.g. FSM state reachability). Since
//! the IR is necessary for cross-language code generation, the IR validator
//! checks these requirements instead, even in a pure Rust flow (see below).
//! This is an opinionated trade-off between Rust-purity, ease of encoding the
//! modeling requirements, and ease of declaring an application model.
//! Constraints may migrate to the type system over time, because in an ideal
//! world, the Rust compiler would validate everything.
//!
//! # IR usage overview
//!
//! ## A. Rust model source, Rust target
//!
//! 1. Rust source with Rust types + derive macros (`Entity`, etc.).
//! 2. Derive macros construct the IR in memory and validates the IR.
//! 3. Derive macros generate instrumentation library.
//! 4. Rust application source uses the instrumentation library directly (e.g. FooObserver, etc.).
//!
//! ## B. Rust model source, Non-Rust target language (TODO)
//!
//! E.g. a C++/Python target through CXX/PyO3.
//!
//! 1. Rust source with Rust types + derive macros (`Entity`, etc.).
//! 2. Derive macros construct the IR in memory and validate the IR.
//! 3. Derive macros generate instrumentation library.
//! 4. Derive macros generate IR trait impls for entities with functions that return the IR in memory.
//! 5. A build.rs script obtains the IR through the traits, and generates target language codegen compatible wrapper code.
//! 6. The build.rs script runs target language codegen.
//! 7. The build.rs artifacts are used in the downstream target lang toolchains.
//!
//! ## C. Non-Rust model source, Rust target (TODO)
//!
//! E.g. a YAML-based DSL or a JSON-serialized IR.
//!
//! 1. The Rust source invokes a function-like macro, e.g. `quent_dsl!(include_str!("model.sourcetype"))`.
//! 2. The macro parses to IR, emits Rust source (type decls + derives).
//! 3. Recursive macro expansion, see flow A.
//!
//! ## D. Non-Rust model source, Non-Rust target (TODO)
//!
//! 1. A build.rs in a Rust bridge crate parses the Non-Rust source into IR and validates it.
//! 2. The build.rs emits Rust source (entity decls + derives) into `OUT_DIR`.
//! 3. The bridge crate compiles via rustc (see flow A).
//! 4. The build.rs also emits target-language source from the IR.
//! 5. The build.rs emits glue between the Rust instrumentation and the target language.
//! 6. The user's target-language app uses the emitted glue + links the Rust bridge crate as a static library.
use crate::identifier::{Identifier, IdentifierError};

use self::{attributes::Attributes, entity::Entity};

pub mod attributes;
pub mod entity;
pub mod event;
pub mod identifier;
// pub mod proc;
pub mod qualifications;
pub mod validator;
pub mod value_type;

/// IR of an application model.
pub struct Model {
    /// The name of the model.
    pub name: Identifier,
    /// The [`Entity`]s of the model.
    pub entities: Vec<Entity>,
    /// The [`Attributes`] sets of the model.
    pub attributes: Vec<Attributes>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IrError {
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(#[from] IdentifierError),
}

pub type Result<T> = std::result::Result<T, IrError>;
