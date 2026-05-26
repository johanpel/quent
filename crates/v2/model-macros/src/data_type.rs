// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Support to parse a syn::Type into an IR [`DataType`].
//!
//! [`DataType`] intentionally does not include any built-in types related to
//! framework concepts (`EntityRef`, `Usage`, etc.). These belong at the
//! event-field level and are parsed into
//! [`quent_v2_model_ir::event::EventFieldType`]s.
//!
//! TODO(johanpel): Proc macros run before name resolution so if a user does
//! something like use x::String; #[derive(Entity)] struct Foo { bar: String },
//! then when we parse String, we just see those tokens, but not the use
//! declaration. So it should have been parsed into DataType::Record, but
//! instead it will parse as DataType::String.
use quent_v2_model_ir::{data_type::DataType, identifier::Identifier};

pub fn parse_data_type(ty: &syn::Type) -> syn::Result<DataType> {
    let syn::Type::Path(type_path) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "unsupported type: must be a named path type",
        ));
    };

    let last = type_path
        .path
        .segments
        .last()
        .ok_or_else(|| syn::Error::new_spanned(ty, "type has no path segments"))?;
    let name = last.ident.to_string();

    // Primitive types
    let atom = match name.as_str() {
        "bool" => Some(DataType::Bool),
        "u8" => Some(DataType::U8),
        "u16" => Some(DataType::U16),
        "u32" => Some(DataType::U32),
        "u64" => Some(DataType::U64),
        "i8" => Some(DataType::I8),
        "i16" => Some(DataType::I16),
        "i32" => Some(DataType::I32),
        "i64" => Some(DataType::I64),
        "f32" => Some(DataType::F32),
        "f64" => Some(DataType::F64),
        "String" => Some(DataType::String),
        "Uuid" => Some(DataType::Uuid),
        "CustomAttributes" => Some(DataType::DynamicRecord),
        _ => None,
    };
    if let Some(v) = atom {
        if !last.arguments.is_empty() {
            return Err(syn::Error::new_spanned(
                &last.arguments,
                format!("`{name}` does not take type arguments"),
            ));
        }
        return Ok(v);
    }

    // Composite types
    match name.as_str() {
        "Option" => {
            let inner = parse_generic(&last.arguments, ty)?;
            Ok(DataType::Option(Box::new(parse_data_type(inner)?)))
        }
        "Vec" => {
            let inner = parse_generic(&last.arguments, ty)?;
            Ok(DataType::List(Box::new(parse_data_type(inner)?)))
        }
        "EntityRef" | "Usage" => Err(syn::Error::new_spanned(
            ty,
            format!("`{name}` is only valid in a named field of an entity event variant"),
        )),
        // Assume a user-defined Record-derived type for anything else.
        _ => Ok(DataType::Record(Identifier::new_unchecked(name))),
    }
}

fn parse_generic<'a>(
    args: &'a syn::PathArguments,
    span_source: &syn::Type,
) -> syn::Result<&'a syn::Type> {
    let syn::PathArguments::AngleBracketed(angled) = args else {
        return Err(syn::Error::new_spanned(
            span_source,
            "expected one generic argument",
        ));
    };
    if angled.args.len() != 1 {
        return Err(syn::Error::new_spanned(
            angled,
            "expected exactly one type argument",
        ));
    }
    let syn::GenericArgument::Type(t) = angled.args.first().unwrap() else {
        return Err(syn::Error::new_spanned(angled, "expected a type argument"));
    };
    Ok(t)
}
