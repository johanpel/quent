// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Parse `Capacity<T, K, B>` fields of a `resource! { ... }` DSL body.

use quent_v2_model_ir::identifier::Identifier;
use syn::{GenericArgument, PathArguments, Token, Type, parse::Parse, punctuated::Punctuated};

/// Parsed capacity kind marker.
pub enum ParsedCapacityKind {
    Occupancy,
    Rate,
}

/// Parsed boundedness marker.
pub enum ParsedBoundedness {
    Fixed,
    Resizable,
    Unbounded,
}

pub struct ParsedCapacity {
    pub name_ir: Identifier,
    pub kind: ParsedCapacityKind,
    pub boundedness: ParsedBoundedness,
}

/// `name: Capacity<T, K, B>` — a single capacity declaration inside
/// `resource! { ... }`.
pub struct CapacityField {
    pub name: syn::Ident,
    pub ty: syn::Type,
}

impl Parse for CapacityField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let _vis: syn::Visibility = input.parse()?;
        let name: syn::Ident = input.parse()?;
        let _colon: Token![:] = input.parse()?;
        let ty: syn::Type = input.parse()?;
        Ok(Self { name, ty })
    }
}

pub fn capacities(
    fields: &Punctuated<CapacityField, Token![,]>,
) -> syn::Result<Vec<ParsedCapacity>> {
    fields.iter().map(parse_capacity_field).collect()
}

fn parse_capacity_field(field: &CapacityField) -> syn::Result<ParsedCapacity> {
    let name_ir = Identifier::new_unchecked(field.name.to_string());

    let Type::Path(type_path) = &field.ty else {
        return Err(syn::Error::new_spanned(
            &field.ty,
            "capacity field must be `Capacity<T, K, B>`",
        ));
    };

    let last =
        type_path.path.segments.last().ok_or_else(|| {
            syn::Error::new_spanned(&field.ty, "capacity field type path is empty")
        })?;

    if last.ident != "Capacity" {
        return Err(syn::Error::new_spanned(
            &last.ident,
            "capacity field type must be `Capacity<T, K, B>`",
        ));
    }

    let args = match &last.arguments {
        PathArguments::AngleBracketed(a) => &a.args,
        _ => {
            return Err(syn::Error::new_spanned(
                &last.arguments,
                "`Capacity` requires <T, K, B> arguments",
            ));
        }
    };

    if args.len() != 3 {
        return Err(syn::Error::new_spanned(
            args,
            "`Capacity` requires exactly three type arguments: <T, K, B>",
        ));
    }

    let value_type = type_ident(&args[0])?;
    if value_type != "u64" {
        return Err(syn::Error::new_spanned(
            &args[0],
            "capacity value type must be `u64` (other types not yet supported)",
        ));
    }

    let kind = match type_ident(&args[1])?.as_str() {
        "Occupancy" => ParsedCapacityKind::Occupancy,
        "Rate" => ParsedCapacityKind::Rate,
        other => {
            return Err(syn::Error::new_spanned(
                &args[1],
                format!("capacity kind must be `Occupancy` or `Rate`, got `{other}`"),
            ));
        }
    };

    let boundedness = match type_ident(&args[2])?.as_str() {
        "Fixed" => ParsedBoundedness::Fixed,
        "Resizeable" => ParsedBoundedness::Resizable,
        "Unbounded" => ParsedBoundedness::Unbounded,
        other => {
            return Err(syn::Error::new_spanned(
                &args[2],
                format!(
                    "capacity boundedness must be `Fixed`, `Resizeable`, or `Unbounded`, got `{other}`"
                ),
            ));
        }
    };

    Ok(ParsedCapacity {
        name_ir,
        kind,
        boundedness,
    })
}

fn type_ident(arg: &GenericArgument) -> syn::Result<String> {
    let GenericArgument::Type(Type::Path(p)) = arg else {
        return Err(syn::Error::new_spanned(
            arg,
            "expected a simple type identifier",
        ));
    };
    let last = p
        .path
        .segments
        .last()
        .ok_or_else(|| syn::Error::new_spanned(arg, "empty type path"))?;
    Ok(last.ident.to_string())
}
