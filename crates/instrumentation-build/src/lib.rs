// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generates a Rust instrumentation library source from a
//! [`quent_schema::Schema`].
//!
//! The usual workflow is build-time generation:
//!
//! 1. From your crate's build script, call [`generate`] with `out_dir` set to
//!    the directory Cargo provides via the `OUT_DIR` environment variable; it
//!    writes the generated source there.
//! 2. Pull that file into your crate's source at compile time with the
//!    `include!` macro.
//!
//! # Example
//!
//! In your crate's `build.rs`:
//!
//! ```ignore
//! use quent_instrumentation_build::{Options, generate};
//!
//! let schema = todo!();
//! let opts = Options {
//!     // Exporters serialize events, so a `Serialize` derive is required.
//!     event_derives: &["Debug", "::serde::Serialize"],
//!     record_derives: &["Debug", "::serde::Serialize"],
//!     out_dir: std::env::var("OUT_DIR")?.into(),
//!     file_name: None, // defaults to `<schema name>.rs`
//! };
//! generate(&schema, &opts)?;
//! ```
//!
//! Then, anywhere in your crate's source:
//!
//! ```ignore
//! pub mod demo {
//!     include!(concat!(env!("OUT_DIR"), "/demo.rs"));
//! }
//! ```
//!
//! # Restrictions
//!
//! The schema does not limit how many events an entity declares, but this
//! generator caps once-cardinality
//! ([`Cardinality::Once`](quent_schema::Cardinality::Once)) events at 64 per
//! entity; beyond that, generation fails with
//! [`GenerateError::TooManyOnceEvents`].
//!
//! Building an exporter requires the event type to be `Serialize`, so
//! [`Options::event_derives`] (and [`Options::record_derives`], for events
//! carrying records or entity refs) must include a `Serialize`-providing
//! derive; otherwise the generated code will not compile.

mod any_event;
mod common;
mod data_type;
mod events;
mod namespace;
mod records;
mod runtime;

use std::path::PathBuf;

use quent_constraints::{BaseConstraintsError, Report, validate};
use quent_schema::{Path, Schema};
use quote::quote;

/// Options controlling instrumentation library generation.
pub struct Options {
    /// Derives applied to every generated event payload enum.
    ///
    /// Must include a `Serialize`-providing derive (e.g. `"::serde::Serialize"`):
    /// the generated context builds exporters, which require it.
    // TODO(johanpel): derives are kept as simple as possible for now, but
    // eventually some built-in options for built-in exporters (e.g. serde-based
    // or Narrow) will surface here as simpler type-safe options.
    pub event_derives: &'static [&'static str],

    /// Derives applied to every generated record struct.
    ///
    /// Records embedded in events must also be `Serialize`, so include a
    /// `Serialize`-providing derive (e.g. `"::serde::Serialize"`).
    pub record_derives: &'static [&'static str],

    /// Directory the generated file is written into.
    pub out_dir: PathBuf,

    /// File name to write; defaults to `<schema name>.rs` (lowercased) when
    /// `None`.
    pub file_name: Option<String>,

    /// Emit root and namespace-local `AnyEvent` enums that decode type-erased
    /// events. Each enum carries [`Self::event_derives`].
    ///
    /// No aggregate is emitted for a namespace without events.
    pub any_event: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            event_derives: Default::default(),
            record_derives: Default::default(),
            out_dir: PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()),
            file_name: None,
            any_event: false,
        }
    }
}

/// An error from generating instrumentation source.
#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    #[error("base schema validation failed: {0}")]
    InvalidSchema(#[from] BaseConstraintsError),
    #[error("invalid derive path {derive:?}")]
    InvalidDerive {
        /// The offending derive entry.
        derive: String,
        /// The underlying parse error.
        source: syn::Error,
    },
    #[error("generated code did not form a valid Rust file")]
    InvalidGeneratedCode(#[source] syn::Error),
    #[error(
        "entity `{entity}` declares {count} once-events, exceeding the maximum of {max}",
        max = crate::runtime::MAX_ONCE_EVENTS
    )]
    TooManyOnceEvents {
        /// The offending entity.
        entity: Path,
        /// The number of once-cardinality events the entity declares.
        count: usize,
    },
    #[error("generated observer type `{generated}` conflicts with schema type `{schema_path}`")]
    GeneratedTypeCollision {
        /// The generated Rust type name.
        generated: String,
        /// The schema type whose generated name conflicts.
        schema_path: Path,
    },
    #[error("failed to write generated file")]
    Io(#[from] std::io::Error),
}

pub struct GenerateInfo {
    pub path: PathBuf,
    pub warnings: Vec<String>,
}

/// Generate the full instrumentation source for `schema` with `opts`.
pub fn generate(schema: &Schema, opts: &Options) -> Result<GenerateInfo, GenerateError> {
    let Report {
        base_constraints,
        unregistered_constraints,
        results: _, // unused for now, but built-in constraints go here later
                    // and will add to either errors or warnings.
    } = validate::<()>(schema);

    let warnings = unregistered_constraints;

    // Fail if base constraints aren't met.
    base_constraints?;

    let file_name = opts
        .file_name
        .clone()
        .unwrap_or_else(|| format!("{}.rs", schema.name().to_string().to_lowercase()));
    let path = opts.out_dir.join(file_name);
    std::fs::write(&path, generate_str(schema, opts)?)?;
    Ok(GenerateInfo { path, warnings })
}

/// Return the full instrumentation source for `schema`.
///
/// # Errors
///
/// Returns [`GenerateError`] if a generated observer type conflicts with a
/// schema type, a derive entry is not a parseable Rust path, or the generated
/// code is not a valid Rust file.
pub fn generate_str(schema: &Schema, opts: &Options) -> Result<String, GenerateError> {
    let namespaces = namespace::Namespace::root(schema);

    let reexports = runtime::reexports();
    let entity_types = runtime::entity_types(schema);
    let types = generate_namespace(schema, opts, &namespaces, false)?;
    let model = runtime::generate_model(schema, &namespaces);
    let any_event = if opts.any_event {
        any_event::generate_any_event(&namespaces, opts)?
    } else {
        quote! {}
    };
    let file = syn::parse2::<syn::File>(quote! {
        #reexports
        #entity_types
        #types
        #model
        #any_event
    })
    .map_err(GenerateError::InvalidGeneratedCode)?;
    Ok(prettyplease::unparse(&file))
}

fn generate_namespace(
    schema: &Schema,
    opts: &Options,
    namespace: &namespace::Namespace<'_>,
    include_any_event: bool,
) -> Result<proc_macro2::TokenStream, GenerateError> {
    let records = namespace
        .records()
        .iter()
        .map(|record| records::record_struct(record, opts))
        .collect::<Result<Vec<_>, _>>()?;
    let events = namespace
        .entities()
        .iter()
        .map(|entity| events::entity_event_enum(entity, opts))
        .collect::<Result<Vec<_>, _>>()?;
    let runtime = namespace
        .entities()
        .iter()
        .map(|entity| runtime::entity_runtime_types(schema, entity))
        .collect::<Result<Vec<_>, _>>()?;
    let children = namespace
        .children()
        .iter()
        .map(|child| {
            let segment = child
                .path()
                .last()
                .expect("child namespaces extend their parent");
            let module = common::module_ident(segment);
            let contents = generate_namespace(schema, opts, child, true)?;
            Ok::<_, GenerateError>(quote! {
                pub mod #module {
                    #contents
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let any_event = if include_any_event && opts.any_event && namespace.has_entities() {
        any_event::generate_any_event(namespace, opts)?
    } else {
        quote! {}
    };
    let observer_storage = runtime::observer_storage(schema, namespace)?;
    Ok(quote! {
        #(#records)*
        #(#events)*
        #(#runtime)*
        #(#children)*
        #observer_storage
        #any_event
    })
}

#[cfg(test)]
mod path_tests {
    use super::*;
    use quent_constraints::Constraint;
    use quent_ref_target::RefTargetConstraint;
    use quent_schema::builder::AnnotationsBuilder;
    use quent_schema::builder::SchemaBuilder;
    use quent_schema::test_utils::{entity, event, field, path, record, record_type};
    use quent_schema::{Annotations, DataType};

    #[test]
    fn places_entity_types_in_path_modules() {
        let schema = SchemaBuilder::try_new("Demo")
            .unwrap()
            .with_entity(entity("Foo::Query", [event("event", [])]))
            .build()
            .unwrap();

        let source = generate_str(&schema, &Options::default()).unwrap();
        assert!(source.contains("pub mod foo"));
        assert!(source.contains("pub enum QueryEvent"));
        assert!(!source.contains("pub type Observer"));
        assert!(source.contains(
            "pub struct Handle<E: ::quent_instrumentation::Entity<Context = Context<Demo>>>"
        ));
        assert!(source.contains("impl super::Handle<Query>"));
        assert!(source.contains("impl ::quent_instrumentation::Entity for Query"));
        assert!(source.contains("type Context = super::Context<super::Demo>"));
        assert!(source.contains("pub struct DemoObservers"));
        assert!(source.contains("struct FooObservers"));
        assert!(source.contains("foo_observers: foo::FooObservers"));
        assert!(source.contains("query_observer: ::quent_instrumentation::Observer<Query>"));
        assert!(source.contains(
            "impl ::quent_instrumentation::ObserverProvider<foo::Query> for DemoObservers"
        ));
        assert!(source.contains(r#"const NAME: &'static str = "Foo::Query""#));
        assert!(!source.contains("foo_query_observer"));
    }

    #[test]
    fn separates_types_with_colliding_flattened_paths() {
        let schema = SchemaBuilder::try_new("Demo")
            .unwrap()
            .with_record(record("Foo::BarBaz", []))
            .with_record(record("FooBar::Baz", []))
            .build()
            .unwrap();

        let source = generate_str(&schema, &Options::default()).unwrap();
        assert!(source.contains("pub mod foo"));
        assert!(source.contains("pub struct BarBaz"));
        assert!(source.contains("pub mod foo_bar"));
        assert!(source.contains("pub struct Baz"));
    }

    #[test]
    fn rejects_observer_type_collisions() {
        let conflicting_path = path("Foo::FooObservers");
        let schema = SchemaBuilder::try_new("Demo")
            .unwrap()
            .with_record(record("Foo::FooObservers", []))
            .with_entity(entity("Foo::Query", [event("event", [])]))
            .build()
            .unwrap();

        assert!(matches!(
            generate_str(&schema, &Options::default()),
            Err(GenerateError::GeneratedTypeCollision {
                generated,
                schema_path,
            }) if generated == "FooObservers" && schema_path == conflicting_path
        ));
    }

    #[test]
    fn does_not_merge_namespaces_that_share_a_rust_name() {
        let schema = SchemaBuilder::try_new("Demo")
            .unwrap()
            .with_record(record("FooBar::First", []))
            .with_record(record("foo_bar::Second", []))
            .build()
            .unwrap();

        let source = generate_str(&schema, &Options::default()).unwrap();
        assert_eq!(source.matches("pub mod foo_bar").count(), 2);
    }

    #[test]
    fn qualifies_types_across_path_modules() {
        let target_annotations = AnnotationsBuilder::new()
            .with_constraint(RefTargetConstraint::NAME, Some("Foo::Worker".to_string()))
            .build()
            .unwrap();
        let schema = SchemaBuilder::try_new("Demo")
            .unwrap()
            .with_record(record("Bar::Meta", []))
            .with_record(record("Foo::Parent", []))
            .with_record(record("Foo::Nested::Local", []))
            .with_record(record("Foo::Nested::Child::Value", []))
            .with_record(record("Foo::Sibling::Value", []))
            .with_entity(entity("Foo::Worker", [event("created", [])]))
            .with_entity(entity(
                "Foo::Nested::Task",
                [event(
                    "created",
                    [
                        field("meta", record_type("Bar::Meta")),
                        field("parent", record_type("Foo::Parent")),
                        field("local", record_type("Foo::Nested::Local")),
                        field("child", record_type("Foo::Nested::Child::Value")),
                        field("sibling", record_type("Foo::Sibling::Value")),
                        field(
                            "worker",
                            DataType::EntityRef {
                                data: None,
                                annotations: target_annotations,
                            },
                        ),
                        field(
                            "any",
                            DataType::EntityRef {
                                data: None,
                                annotations: Annotations::default(),
                            },
                        ),
                    ],
                )],
            ))
            .build()
            .unwrap();

        let source = generate_str(&schema, &Options::default()).unwrap();
        assert!(source.contains("meta: super::super::bar::Meta"));
        assert!(source.contains("parent: super::Parent"));
        assert!(source.contains("local: Local"));
        assert!(source.contains("child: child::Value"));
        assert!(source.contains("sibling: super::sibling::Value"));
        assert!(source.contains("worker: ::quent_instrumentation::EntityRef<super::Worker>"));
        assert!(
            source.contains("any: ::quent_instrumentation::EntityRef<super::super::AnyEntity>")
        );
    }

    #[test]
    fn generates_any_event_per_entity_namespace() {
        let schema = SchemaBuilder::try_new("Demo")
            .unwrap()
            .with_entity(entity("Root", [event("created", [])]))
            .with_entity(entity("Foo::Query", [event("created", [])]))
            .with_entity(entity("Foo::Nested::Task", [event("created", [])]))
            .build()
            .unwrap();
        let opts = Options {
            any_event: true,
            ..Options::default()
        };

        let source = generate_str(&schema, &opts).unwrap();
        assert!(source.contains("Root(&'a ::quent_instrumentation::Event<RootEvent>)"));
        assert!(source.contains("Foo(foo::AnyEvent<'a>)"));
        assert!(source.contains("Query(&'a ::quent_instrumentation::Event<QueryEvent>)"));
        assert!(source.contains("Nested(nested::AnyEvent<'a>)"));
        assert!(source.contains("Task(&'a ::quent_instrumentation::Event<TaskEvent>)"));
        assert!(source.contains("foo::AnyEvent::from_any(any)"));
        assert!(source.contains("nested::AnyEvent::from_any(any)"));
        assert!(
            source.rfind("pub enum AnyEvent") > source.rfind("impl ::quent_instrumentation::Model")
        );
    }
}
