// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Support to parse a syn::Type into an IR [`ValueType`].
//!
//! [`ValueType`] intentionally does not include any built-in types related to
//! framework concepts (`EntityRef`, `Usage`, etc.). These belong at the
//! event-field level and are parsed into
//! [`quent_v2_model_ir::event::FieldType`]s.
//!
//! TODO(johanpel): Proc macros run before name resolution so if a user does
//! something like use x::String; #[derive(Entity)] struct Foo { bar: String },
//! then when we parse String, we just see those tokens, but not the use
//! declaration. So it should have been parsed into ValueType::Attributes, but
//! instead it will parse as ValueType::String.
use quent_v2_model_ir::{identifier::Identifier, value_type::ValueType};

pub fn parse_value_type(ty: &syn::Type) -> syn::Result<ValueType> {
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
        "bool" => Some(ValueType::Bool),
        "u8" => Some(ValueType::U8),
        "u16" => Some(ValueType::U16),
        "u32" => Some(ValueType::U32),
        "u64" => Some(ValueType::U64),
        "i8" => Some(ValueType::I8),
        "i16" => Some(ValueType::I16),
        "i32" => Some(ValueType::I32),
        "i64" => Some(ValueType::I64),
        "f32" => Some(ValueType::F32),
        "f64" => Some(ValueType::F64),
        "String" => Some(ValueType::String),
        "Uuid" => Some(ValueType::Uuid),
        "CustomAttributes" => Some(ValueType::CustomAttributes),
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
            Ok(ValueType::Option(Box::new(parse_value_type(inner)?)))
        }
        "Vec" => {
            let inner = parse_generic(&last.arguments, ty)?;
            Ok(ValueType::List(Box::new(parse_value_type(inner)?)))
        }
        "EntityRef" | "Usage" => Err(syn::Error::new_spanned(
            ty,
            format!("`{name}` is only valid in a named field of an entity event variant"),
        )),
        // Assume a user-defined Attributes-derived type for anything else.
        _ => Ok(ValueType::Attributes(Identifier::new_unchecked(name))),
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
