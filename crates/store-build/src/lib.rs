// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generates event-store models from schemas.
//!
//! Add `quent-store-build` to `[build-dependencies]`, call [`generate`] from
//! `build.rs`, and include the generated file from Cargo's `OUT_DIR`. The
//! consuming crate requires normal dependencies on `quent-store`,
//! serde-enabled `quent-events`, and derive-enabled `serde`.

use std::path::PathBuf;

use quent_schema::Schema;
use quote::quote;

/// Options controlling event-store source generation.
pub struct Options {
    /// Derive [`Debug`](std::fmt::Debug) on generated event and record types.
    pub debug: bool,

    /// Derives applied to every generated event payload enum.
    pub event_derives: &'static [&'static str],

    /// Derives applied to every generated record struct.
    pub record_derives: &'static [&'static str],

    /// Generate a model-wide umbrella event and model-wide loading support.
    pub umbrella_event: bool,

    /// Directory the generated file is written into.
    pub out_dir: PathBuf,

    /// File name to write; defaults to the lowercase schema name with a `.rs` extension.
    pub file_name: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            debug: true,
            event_derives: Default::default(),
            record_derives: Default::default(),
            umbrella_event: false,
            out_dir: PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()),
            file_name: None,
        }
    }
}

/// An error from generating event-store source.
#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    #[error(transparent)]
    EventModel(#[from] quent_instrumentation_build::GenerateError),
    #[error("generated event-store code did not form a valid Rust file")]
    InvalidGeneratedCode(#[source] syn::Error),
    #[error("failed to write generated event-store source")]
    Io(#[from] std::io::Error),
}

/// Information about generated event-store source.
pub struct GenerateInfo {
    /// Path of the generated Rust source file.
    pub path: PathBuf,
    /// Constraint names without registered validators.
    pub warnings: Vec<String>,
}

/// Generates an event model and its event-store descriptors.
///
/// # Errors
///
/// Returns an error when the schema cannot be generated or the output cannot be written.
pub fn generate(schema: &Schema, opts: &Options) -> Result<GenerateInfo, GenerateError> {
    let warnings = quent_instrumentation_build::validate_schema(schema)?;
    let file_name = opts
        .file_name
        .clone()
        .unwrap_or_else(|| format!("{}.rs", schema.name().to_string().to_lowercase()));
    let path = opts.out_dir.join(file_name);
    std::fs::write(&path, generate_str(schema, opts)?)?;
    Ok(GenerateInfo { path, warnings })
}

/// Returns event-store model source for `schema`.
///
/// # Errors
///
/// Returns an error when event generation fails or the combined output is not valid Rust.
pub fn generate_str(schema: &Schema, opts: &Options) -> Result<String, GenerateError> {
    let event_opts = quent_instrumentation_build::Options {
        instrumentation: false,
        debug: opts.debug,
        serde: true,
        event_derives: opts.event_derives,
        record_derives: opts.record_derives,
        umbrella_event: opts.umbrella_event,
        ..quent_instrumentation_build::Options::default()
    };
    let events = quent_instrumentation_build::generate_str(schema, &event_opts)?;
    let events =
        syn::parse_str::<syn::File>(&events).map_err(GenerateError::InvalidGeneratedCode)?;

    let model = quent_instrumentation_build::generated_model_path(schema);
    let stored_model = if opts.umbrella_event {
        let streams = schema.entities().map(|entity| {
            let event = quent_instrumentation_build::generated_entity_event_path(entity);
            quote! {
                ::quent_store::event::filesystem::EventStream::new(
                    <#event as ::quent_events::EntityEvent>::NAME,
                    ::quent_store::event::filesystem::import_event_files::<#model, #event>,
                )
            }
        });
        quote! {
            impl ::quent_store::event::filesystem::Model for #model {
                fn event_streams(
                ) -> &'static [::quent_store::event::filesystem::EventStream<Self>] {
                    static STREAMS: &[
                        ::quent_store::event::filesystem::EventStream<#model>
                    ] = &[
                        #(#streams,)*
                    ];
                    STREAMS
                }
            }
        }
    } else {
        quote! {}
    };
    let entities = schema.entities().map(|entity| {
        let marker = quent_instrumentation_build::generated_entity_path(entity);
        quote! {
            impl ::quent_store::event::StoredEntity<#model> for #marker {}
        }
    });

    let file = syn::parse2::<syn::File>(quote! {
        #events

        #stored_model

        #(#entities)*
    })
    .map_err(GenerateError::InvalidGeneratedCode)?;

    Ok(prettyplease::unparse(&file))
}

#[cfg(test)]
mod tests {
    use quent_schema::builder::{AnnotationsBuilder, EntityBuilder, SchemaBuilder};
    use quent_schema::test_utils::{entity, event, eventless_entity, unchecked_schema};

    use super::*;

    #[test]
    fn generates_store_model_and_nested_entity_membership() {
        let schema = SchemaBuilder::try_new("Demo")
            .unwrap()
            .with_entity(entity("Foo::Query", [event("created", [])]))
            .with_entity(entity("Foo::Nested::Task", [event("created", [])]))
            .build()
            .unwrap();

        let opts = Options {
            umbrella_event: true,
            ..Options::default()
        };
        let source = generate_str(&schema, &opts).unwrap();

        assert!(source.contains("impl ::quent_store::event::filesystem::Model for Demo"));
        assert_eq!(source.matches("import_event_files::<").count(), 2);
        assert!(source.contains("foo::QueryEvent"));
        assert!(source.contains("foo::nested::TaskEvent"));
        assert!(source.contains("event::StoredEntity<Demo> for foo::Query"));
        assert!(source.contains("event::StoredEntity<Demo> for foo::nested::Task"));
        assert!(!source.contains("quent_instrumentation"));
    }

    #[test]
    fn generates_entity_loading_without_an_umbrella_by_default() {
        let schema = SchemaBuilder::try_new("Demo")
            .unwrap()
            .with_entity(entity("Query", [event("created", [])]))
            .build()
            .unwrap();

        let source = generate_str(&schema, &Options::default()).unwrap();

        assert!(source.contains("event::StoredEntity<Demo> for Query"));
        assert!(!source.contains("filesystem::Model for Demo"));
        assert!(!source.contains("pub enum DemoEvent"));
    }

    #[test]
    fn generate_rejects_invalid_schemas() {
        let schema = unchecked_schema("Demo", [eventless_entity("Query")], []);
        let output = tempfile::tempdir().unwrap();
        let options = Options {
            out_dir: output.path().to_owned(),
            ..Options::default()
        };

        assert!(matches!(
            generate(&schema, &options),
            Err(GenerateError::EventModel(
                quent_instrumentation_build::GenerateError::InvalidSchema(_)
            ))
        ));
    }

    #[test]
    fn generate_returns_unregistered_constraint_warnings() {
        let annotations = AnnotationsBuilder::new()
            .with_constraint("example.unknown.v0.1.0", None)
            .build()
            .unwrap();
        let query = EntityBuilder::try_new("Query")
            .unwrap()
            .with_event(event("created", []))
            .with_annotations(annotations)
            .build()
            .unwrap();
        let schema = SchemaBuilder::try_new("Demo")
            .unwrap()
            .with_entity(query)
            .build()
            .unwrap();
        let output = tempfile::tempdir().unwrap();
        let options = Options {
            out_dir: output.path().to_owned(),
            ..Options::default()
        };

        let generated = generate(&schema, &options).unwrap();

        assert_eq!(generated.warnings, ["example.unknown.v0.1.0"]);
    }
}
