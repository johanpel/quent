// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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
//! use quent_instrumentation_build::{GenerateOptions, generate};
//!
//! let schema = todo!(); // some schema (ideally validated, see `quent-constraints`)
//! let opts = GenerateOptions {
//!     event_derives: &["Debug", "Clone", "::serde::Serialize"],
//!     record_derives: &["Debug", "Clone", "::serde::Serialize"],
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

use std::path::PathBuf;

use convert_case::{Boundary, Case, Casing};
use proc_macro2::{Span, TokenStream};
use quent_schema::{DataType, Entity, Identifier, Record, Schema};
use quote::quote;
use syn::Ident;

/// Options controlling code generation.
// TODO(johanpel): kept as simple as possible for now, but eventually some
// built-in options for exporters (e.g. serde-based or Narrow) will surface
// here as simple type-safe options.
#[derive(Default)]
pub struct GenerateOptions {
    /// Derives applied to every generated event payload enum.
    ///
    /// Use this to apply e.g. `&["Debug", "::serde::Serialize"]`
    pub event_derives: &'static [&'static str],
    /// Derives applied to every generated record struct.
    ///
    /// Use this to apply e.g. `&["Debug", "::serde::Serialize"]`
    pub record_derives: &'static [&'static str],
    /// Directory the generated file is written into, e.g. the build script's
    /// `OUT_DIR`.
    pub out_dir: PathBuf,
    /// File name to write; defaults to `<schema name>.rs` (lowercased) when
    /// `None`.
    pub file_name: Option<String>,
}

/// An error from generating instrumentation source.
#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    /// An entry in [`GenerateOptions::event_derives`] or
    /// [`GenerateOptions::record_derives`] is not a parseable Rust path.
    #[error("invalid derive path {derive:?}")]
    InvalidDerive {
        /// The offending derive entry.
        derive: String,
        /// The underlying parse error.
        source: syn::Error,
    },
    /// The generated tokens did not form a valid Rust file.
    #[error("generated code did not form a valid Rust file")]
    InvalidGeneratedCode(#[source] syn::Error),
    /// Writing the generated file failed.
    #[error("failed to write generated file")]
    Io(#[from] std::io::Error),
}

/// Combined token output: record structs followed by event enums.
fn combined_tokens(schema: &Schema, opts: &GenerateOptions) -> Result<TokenStream, GenerateError> {
    let records = generate_record_types(schema, opts)?;
    let events = generate_event_types(schema, opts)?;
    Ok(quote! { #records #events })
}

/// Generate the full instrumentation source for `schema` and write it to
/// `opts.out_dir`, returning the path written.
///
/// The file is named `opts.file_name` if set, otherwise `<schema name>.rs`
/// (lowercased).
///
/// # Errors
///
/// Returns [`GenerateError`] if a derive entry is not a parseable Rust path, if
/// the generated code is not a valid Rust file, or if writing the file fails.
///
/// # Panics
///
/// Panics if a field type nests deeper than [`MAX_TYPE_DEPTH`].
pub fn generate(schema: &Schema, opts: &GenerateOptions) -> Result<PathBuf, GenerateError> {
    let file_name = opts
        .file_name
        .clone()
        .unwrap_or_else(|| format!("{}.rs", schema.name().to_string().to_lowercase()));
    let path = opts.out_dir.join(file_name);
    std::fs::write(&path, generate_str(schema, opts)?)?;
    Ok(path)
}

/// Generate the full instrumentation source for `schema` — every record struct
/// followed by every per-entity event enum — formatted with `prettyplease`.
///
/// Use [`generate`] to write the result to a file in one step.
///
/// # Errors
///
/// Returns [`GenerateError`] if a derive entry is not a parseable Rust path, or
/// if the generated code is not a valid Rust file.
///
/// # Panics
///
/// Panics if a field type nests deeper than [`MAX_TYPE_DEPTH`].
pub fn generate_str(schema: &Schema, opts: &GenerateOptions) -> Result<String, GenerateError> {
    let file = syn::parse2::<syn::File>(combined_tokens(schema, opts)?)
        .map_err(GenerateError::InvalidGeneratedCode)?;
    Ok(prettyplease::unparse(&file))
}

/// Per-entity event payload enums, as tokens.
fn generate_event_types(
    schema: &Schema,
    opts: &GenerateOptions,
) -> Result<TokenStream, GenerateError> {
    let enums: Vec<TokenStream> = schema
        .entities()
        .map(|entity| entity_event_enum(entity, opts))
        .collect::<Result<_, _>>()?;
    Ok(quote! { #(#enums)* })
}

/// Generate the per-entity event payload enums for `schema`, in declaration
/// order, formatted with `prettyplease`.
///
/// Each entity yields `pub enum <Entity>Event`; each of its events is a variant
/// (UpperCamel) whose payload fields are snake_case named fields. Type, variant
/// and field names are derived by case conversion that preserves digits, and
/// names that are Rust keywords are raw-escaped. The caller must ensure entity,
/// event and field names are unique after that conversion: two names that
/// converge produce a duplicate definition that fails to compile.
///
/// # Errors
///
/// Returns [`GenerateError`] if a derive entry is not a parseable Rust path, or
/// if the generated code is not a valid Rust file.
///
/// # Panics
///
/// Panics if a field type nests deeper than [`MAX_TYPE_DEPTH`].
pub fn generate_event_types_str(
    schema: &Schema,
    opts: &GenerateOptions,
) -> Result<String, GenerateError> {
    let file = syn::parse2::<syn::File>(generate_event_types(schema, opts)?)
        .map_err(GenerateError::InvalidGeneratedCode)?;
    Ok(prettyplease::unparse(&file))
}

/// Record structs, as tokens.
fn generate_record_types(
    schema: &Schema,
    opts: &GenerateOptions,
) -> Result<TokenStream, GenerateError> {
    let records: Vec<TokenStream> = schema
        .records()
        .map(|record| record_struct(record, opts))
        .collect::<Result<_, _>>()?;
    Ok(quote! { #(#records)* })
}

/// Generate the record structs for `schema`, in declaration order, formatted
/// with `prettyplease`.
///
/// Each record yields `pub struct <Record>` with one public field per record
/// field. Naming, raw-escaping and the uniqueness obligation match
/// [`generate_event_types_str`].
///
/// # Errors
///
/// Returns [`GenerateError`] if a derive entry is not a parseable Rust path, or
/// if the generated code is not a valid Rust file.
///
/// # Panics
///
/// Panics if a field type nests deeper than [`MAX_TYPE_DEPTH`].
pub fn generate_record_types_str(
    schema: &Schema,
    opts: &GenerateOptions,
) -> Result<String, GenerateError> {
    let file = syn::parse2::<syn::File>(generate_record_types(schema, opts)?)
        .map_err(GenerateError::InvalidGeneratedCode)?;
    Ok(prettyplease::unparse(&file))
}

fn entity_event_enum(entity: &Entity, opts: &GenerateOptions) -> Result<TokenStream, GenerateError> {
    let enum_ident = raw_ident(format!("{}Event", to_case(entity.name(), Case::Pascal)));
    let docs = doc_attr(entity.annotations().docs());
    let derives = derive_attr(opts.event_derives)?;
    let variants: Vec<TokenStream> = entity
        .events()
        .map(|event| {
            let variant = raw_ident(to_case(event.name(), Case::Pascal));
            let variant_docs = doc_attr(event.annotations().docs());
            let fields: Vec<TokenStream> = event
                .fields()
                .map(|field| {
                    let name = raw_ident(to_case(field.name(), Case::Snake));
                    let ty = map_data_type(field.ty());
                    let field_docs = doc_attr(field.annotations().docs());
                    quote! { #field_docs #name: #ty }
                })
                .collect();
            if fields.is_empty() {
                quote! { #variant_docs #variant }
            } else {
                quote! { #variant_docs #variant { #(#fields),* } }
            }
        })
        .collect();
    Ok(quote! {
        #docs
        #derives
        pub enum #enum_ident {
            #(#variants),*
        }
    })
}

fn record_struct(record: &Record, opts: &GenerateOptions) -> Result<TokenStream, GenerateError> {
    let ident = raw_ident(to_case(record.name(), Case::Pascal));
    let docs = doc_attr(record.annotations().docs());
    let derives = derive_attr(opts.record_derives)?;
    let fields: Vec<TokenStream> = record
        .fields()
        .map(|field| {
            let name = raw_ident(to_case(field.name(), Case::Snake));
            let ty = map_data_type(field.ty());
            let field_docs = doc_attr(field.annotations().docs());
            quote! { #field_docs pub #name: #ty }
        })
        .collect();
    if fields.is_empty() {
        Ok(quote! { #docs #derives pub struct #ident {} })
    } else {
        Ok(quote! {
            #docs
            #derives
            pub struct #ident {
                #(#fields),*
            }
        })
    }
}

/// Maximum nesting depth of `Option`/`List`/`EntityRef` wrappers a single field
/// type may have. A defensive bound against runaway recursion on malformed input;
/// far above any realistic schema.
pub const MAX_TYPE_DEPTH: usize = 64;

/// Map a [`DataType`] to its Rust type tokens, recursing through `Option`,
/// `List` and `EntityRef` payloads.
fn map_data_type(ty: &DataType) -> TokenStream {
    map_data_type_at(ty, 0)
}

fn map_data_type_at(ty: &DataType, depth: usize) -> TokenStream {
    assert!(
        depth <= MAX_TYPE_DEPTH,
        "field type nesting exceeds the maximum depth of {MAX_TYPE_DEPTH}"
    );
    match ty {
        DataType::Bool => quote! { bool },
        DataType::Uuid => quote! { ::uuid::Uuid },
        DataType::String => quote! { String },
        DataType::U8 => quote! { u8 },
        DataType::U16 => quote! { u16 },
        DataType::U32 => quote! { u32 },
        DataType::U64 => quote! { u64 },
        DataType::I8 => quote! { i8 },
        DataType::I16 => quote! { i16 },
        DataType::I32 => quote! { i32 },
        DataType::I64 => quote! { i64 },
        DataType::F32 => quote! { f32 },
        DataType::F64 => quote! { f64 },
        DataType::Option(inner) => {
            let inner = map_data_type_at(inner, depth + 1);
            quote! { Option<#inner> }
        }
        DataType::List(inner) => {
            let inner = map_data_type_at(inner, depth + 1);
            quote! { Vec<#inner> }
        }
        DataType::Record(name) => {
            let ident = raw_ident(to_case(name, Case::Pascal));
            quote! { #ident }
        }
        DataType::DynamicRecord => quote! { ::quent_attributes::CustomAttributes },
        DataType::EntityRef { data, .. } => match data {
            Some(inner) => {
                let inner = map_data_type_at(inner, depth + 1);
                quote! { ::quent_instrumentation_runtime::EntityRef<#inner> }
            }
            None => quote! { ::quent_instrumentation_runtime::EntityRef },
        },
    }
}

fn derive_attr(derives: &[&str]) -> Result<TokenStream, GenerateError> {
    if derives.is_empty() {
        return Ok(quote! {});
    }
    let paths = derives
        .iter()
        .copied()
        .map(|d| {
            syn::parse_str::<syn::Path>(d).map_err(|source| GenerateError::InvalidDerive {
                derive: d.to_owned(),
                source,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quote! { #[derive(#(#paths),*)] })
}

fn doc_attr(docs: Option<&str>) -> TokenStream {
    match docs {
        Some(text) => quote! { #[doc = #text] },
        None => quote! {},
    }
}

/// Case-convert a schema identifier without splitting letter/digit boundaries,
/// so names such as `u8` or `http2` are preserved rather than mangled.
fn to_case(id: &Identifier, case: Case) -> String {
    const KEEP_DIGITS: &[Boundary] = &[
        Boundary::LOWER_DIGIT,
        Boundary::UPPER_DIGIT,
        Boundary::DIGIT_LOWER,
        Boundary::DIGIT_UPPER,
    ];
    id.to_string().without_boundaries(KEEP_DIGITS).to_case(case)
}

/// Build an identifier from an already-cased name, raw-escaping Rust keywords.
/// The keywords that cannot be raw (`crate`, `self`, `super`, `Self`) instead
/// receive a trailing underscore.
fn raw_ident(name: String) -> Ident {
    const NON_RAW: &[&str] = &["crate", "self", "super", "Self"];
    if NON_RAW.contains(&name.as_str()) {
        Ident::new(&format!("{name}_"), Span::call_site())
    } else if syn::parse_str::<Ident>(&name).is_ok() {
        Ident::new(&name, Span::call_site())
    } else {
        Ident::new_raw(&name, Span::call_site())
    }
}
