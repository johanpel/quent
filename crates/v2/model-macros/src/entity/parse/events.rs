// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model_ir::{
    event::{Cardinality, Event, Field},
    identifier::Identifier,
    value_type::ValueType,
};
use syn::{DeriveInput, Fields, Variant};

use crate::value_type::parse_value_type;

pub fn parse(input: &DeriveInput) -> syn::Result<Vec<Event>> {
    match &input.data {
        syn::Data::Struct(s) => parse_struct_events(&input.ident, &s.fields, input),
        syn::Data::Enum(e) => e.variants.iter().map(parse_enum_variant_event).collect(),
        syn::Data::Union(u) => Err(syn::Error::new_spanned(
            u.union_token,
            "#[derive(Entity)] not supported for union, use struct or enum",
        )),
    }
}

fn parse_struct_events(
    name: &syn::Ident,
    fields: &Fields,
    input: &DeriveInput,
) -> syn::Result<Vec<Event>> {
    if matches!(fields, Fields::Unnamed(_)) {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Entity)] on a struct requires a unit struct or a struct with named fields",
        ));
    }
    let name_ident = Identifier::new_unchecked(name.to_string());
    let has_payload = matches!(fields, Fields::Named(n) if !n.named.is_empty());
    let payload = if has_payload {
        vec![Field::new(
            EventValueType::Attribute,
            ValueType::Attributes(name_ident.clone()),
        )]
    } else {
        Vec::new()
    };
    Ok(vec![Event::new(name_ident, Cardinality::Once, payload)])
}

fn parse_enum_variant_event(v: &Variant) -> syn::Result<Event> {
    let variant_name = Identifier::new_unchecked(v.ident.to_string());
    let cardinality = parse_cardinality(&v.attrs)?;
    let payload = parse_variant_payloads(&v.fields, v)?;
    Ok(Event::new(variant_name, cardinality, payload))
}

fn parse_cardinality(attrs: &[syn::Attribute]) -> syn::Result<Cardinality> {
    // The cardinality of entity events is Once by default
    let mut is_multi = false;

    // Go over the variant attributes and check for quent-related ones.
    for attr in attrs {
        if !attr.path().is_ident("quent") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            // TODO(johanpel): reject this if this is placed on FSM transitions
            if meta.path.is_ident("multi") {
                is_multi = true;
                Ok(())
            } else {
                Err(meta.error("unknown #[quent(...)] argument"))
            }
        })?;
    }

    Ok(if is_multi {
        Cardinality::Multi
    } else {
        Cardinality::Once
    })
}

fn parse_variant_payloads(fields: &syn::Fields, span_source: &Variant) -> syn::Result<Vec<Field>> {
    match fields {
        syn::Fields::Unit => Ok(Vec::new()),
        syn::Fields::Unnamed(u) if u.unnamed.len() == 1 => {
            let unnamed_field = u.unnamed.first().unwrap();
            let ty = &unnamed_field.ty;
            Ok(vec![Field::new(
                EventValueType::Attribute,
                parse_value_type(ty)?,
            )])
        }
        syn::Fields::Unnamed(_) => Err(syn::Error::new_spanned(
            span_source,
            "#[derive(Entity)] does not support enum variants with more than one unnamed field",
        )),
        syn::Fields::Named(named) => named
            .named
            .iter()
            .map(|f| {
                let field_name = f.ident.as_ref().unwrap();
                let role = parse_field_role(field_name)?;
                let ty = &f.ty;
                Ok(Field::new(role, parse_value_type(ty)?))
            })
            .collect(),
    }
}

fn parse_field_role(name: &syn::Ident) -> syn::Result<EventValueType> {
    EventValueType::try_from(name.to_string().as_str())
        .map_err(|e| syn::Error::new(name.span(), e.to_string()))
}
