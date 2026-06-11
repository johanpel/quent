// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Schema-driven generation of Rust instrumentation code.
//!
//! Consumes a [`quent_schema::Schema`] and emits the per-entity event payload
//! types it describes. Output depends only on schema structure and `docs`
//! annotations; constraint and metadata annotations never affect it.

use convert_case::{Boundary, Case, Casing};
use proc_macro2::{Span, TokenStream};
use quent_schema::{DataType, Entity, Identifier, Record, Schema};
use quote::quote;
use syn::Ident;

/// Options controlling code generation.
#[derive(Debug, Clone)]
pub struct CodegenOptions {
    /// Derives applied to every generated event payload enum, as Rust path
    /// strings (e.g. `"Debug"`, `"::serde::Serialize"`). Emitted verbatim in a
    /// single `#[derive(...)]`; an empty list emits no derive attribute.
    pub event_derives: Vec<String>,
    /// Derives applied to every generated record struct, in the same form as
    /// [`CodegenOptions::event_derives`]. Keep compatible with `event_derives`
    /// where records appear as event fields (e.g. derive serde on both).
    pub record_derives: Vec<String>,
}

impl Default for CodegenOptions {
    fn default() -> Self {
        Self {
            event_derives: vec!["Debug".to_owned(), "Clone".to_owned()],
            record_derives: vec!["Debug".to_owned(), "Clone".to_owned()],
        }
    }
}

/// Generate the full instrumentation type surface for `schema`: every record
/// struct followed by every per-entity event enum.
///
/// # Panics
///
/// Panics under the same conditions as [`generate_record_types`] and
/// [`generate_event_types`].
pub fn generate(schema: &Schema, opts: &CodegenOptions) -> TokenStream {
    let records = generate_record_types(schema, opts);
    let events = generate_event_types(schema, opts);
    quote! { #records #events }
}

/// Pretty-printed form of [`generate`].
///
/// # Panics
///
/// See [`generate`]; additionally panics if the generated tokens do not form a
/// parseable Rust file.
pub fn generate_str(schema: &Schema, opts: &CodegenOptions) -> String {
    let file = syn::parse2::<syn::File>(generate(schema, opts))
        .expect("generated code must form a valid Rust file");
    prettyplease::unparse(&file)
}

/// Generate one event payload enum per entity in `schema`, in declaration order.
///
/// Each entity yields `pub enum <Entity>Event`; each of its events is a variant
/// (UpperCamel) whose payload fields are snake_case named fields. Type, variant
/// and field names are derived by case conversion that preserves digits, and
/// names that are Rust keywords are raw-escaped. The caller must ensure entity,
/// event and field names are unique after that conversion: two names that
/// converge produce a duplicate the consumer compiler rejects.
///
/// # Panics
///
/// Panics if any entry in `opts.event_derives` is not a parseable Rust path, or
/// if a field type nests deeper than [`MAX_TYPE_DEPTH`].
pub fn generate_event_types(schema: &Schema, opts: &CodegenOptions) -> TokenStream {
    let enums: Vec<TokenStream> = schema
        .entities()
        .map(|entity| entity_event_enum(entity, opts))
        .collect();
    quote! { #(#enums)* }
}

/// Pretty-printed form of [`generate_event_types`].
///
/// # Panics
///
/// Panics on an unparseable derive path (see [`generate_event_types`]), or if the
/// generated tokens do not form a parseable Rust file.
pub fn generate_event_types_str(schema: &Schema, opts: &CodegenOptions) -> String {
    let file = syn::parse2::<syn::File>(generate_event_types(schema, opts))
        .expect("generated event types must form a valid Rust file");
    prettyplease::unparse(&file)
}

/// Generate one struct per record in `schema`, in declaration order.
///
/// Each record yields `pub struct <Record>` with one public field per record
/// field. Naming, raw-escaping and the uniqueness obligation match
/// [`generate_event_types`].
///
/// # Panics
///
/// Panics if any entry in `opts.record_derives` is not a parseable Rust path, or
/// if a field type nests deeper than [`MAX_TYPE_DEPTH`].
pub fn generate_record_types(schema: &Schema, opts: &CodegenOptions) -> TokenStream {
    let records: Vec<TokenStream> = schema
        .records()
        .map(|record| record_struct(record, opts))
        .collect();
    quote! { #(#records)* }
}

/// Pretty-printed form of [`generate_record_types`].
///
/// # Panics
///
/// See [`generate_record_types`]; additionally panics if the generated tokens do
/// not form a parseable Rust file.
pub fn generate_record_types_str(schema: &Schema, opts: &CodegenOptions) -> String {
    let file = syn::parse2::<syn::File>(generate_record_types(schema, opts))
        .expect("generated record types must form a valid Rust file");
    prettyplease::unparse(&file)
}

fn entity_event_enum(entity: &Entity, opts: &CodegenOptions) -> TokenStream {
    let enum_ident = raw_ident(format!("{}Event", to_case(entity.name(), Case::Pascal)));
    let docs = doc_attr(entity.annotations().docs());
    let derives = derive_attr(&opts.event_derives);
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
    quote! {
        #docs
        #derives
        pub enum #enum_ident {
            #(#variants),*
        }
    }
}

fn record_struct(record: &Record, opts: &CodegenOptions) -> TokenStream {
    let ident = raw_ident(to_case(record.name(), Case::Pascal));
    let docs = doc_attr(record.annotations().docs());
    let derives = derive_attr(&opts.record_derives);
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
        quote! { #docs #derives pub struct #ident {} }
    } else {
        quote! {
            #docs
            #derives
            pub struct #ident {
                #(#fields),*
            }
        }
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

fn derive_attr(derives: &[String]) -> TokenStream {
    if derives.is_empty() {
        return quote! {};
    }
    let paths = derives.iter().map(|d| {
        syn::parse_str::<syn::Path>(d).unwrap_or_else(|e| panic!("invalid derive path {d:?}: {e}"))
    });
    quote! { #[derive(#(#paths),*)] }
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
