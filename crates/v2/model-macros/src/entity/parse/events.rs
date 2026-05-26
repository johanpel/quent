// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model_ir::{
    data_type::DataType,
    event::{Cardinality, EntityRefRole, EntityRefTarget, Event, EventField, EventFieldType},
    identifier::Identifier,
};
use syn::{DeriveInput, Fields, Variant, spanned::Spanned};

use crate::data_type::parse_data_type;

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
            "#[derive(Entity)] on struct requires a unit struct or a struct with named fields",
        ));
    }
    let name_ident = Identifier::new_unchecked(name.to_string());
    let has_payload = matches!(fields, Fields::Named(n) if !n.named.is_empty());
    let payload = if has_payload {
        vec![EventField::from_type(EventFieldType::Payload(
            DataType::Record(name_ident.clone()),
        ))]
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

fn parse_variant_payloads(
    fields: &syn::Fields,
    span_source: &Variant,
) -> syn::Result<Vec<EventField>> {
    match fields {
        syn::Fields::Unit => Ok(Vec::new()),
        syn::Fields::Unnamed(u) if u.unnamed.len() == 1 => {
            // Safety: unwrap, len is checked above:
            let ty = parse_event_field_type(&u.unnamed.first().unwrap().ty)?;
            Ok(vec![EventField::from_type(ty)])
        }
        syn::Fields::Unnamed(_) => Err(syn::Error::new_spanned(
            span_source,
            "#[derive(Entity)] does not support enum variants with more than one unnamed field",
        )),
        syn::Fields::Named(named) => named
            .named
            .iter()
            .map(|f| {
                // Safety: Fields::Named always carry an ident.
                let ident = f.ident.as_ref().unwrap();
                let name = Identifier::try_from(ident.to_string().as_str())
                    .map_err(|e| syn::Error::new(ident.span(), e.to_string()))?;
                let ty = &f.ty;
                Ok(EventField::new(name, parse_event_field_type(ty)?))
            })
            .collect(),
    }
}

fn parse_event_field_type(ty: &syn::Type) -> syn::Result<EventFieldType> {
    let syn::Type::Path(type_path) = ty else {
        return Ok(EventFieldType::Payload(parse_data_type(ty)?));
    };
    let Some(last) = type_path.path.segments.last() else {
        return Ok(EventFieldType::Payload(parse_data_type(ty)?));
    };
    match last.ident.to_string().as_str() {
        "EntityRef" => parse_entity_ref(&last.arguments),
        "Usage" => parse_usage(&last.arguments),
        _ => Ok(EventFieldType::Payload(parse_data_type(ty)?)),
    }
}

fn parse_entity_ref(args: &syn::PathArguments) -> syn::Result<EventFieldType> {
    // This must match the defaults set in the model crate:
    let mut role_type = EntityRefRole::Plain;
    let mut entity_type = EntityRefTarget::Any;

    if let syn::PathArguments::AngleBracketed(args) = args {
        let mut type_args = args.args.iter().filter_map(|a| match a {
            syn::GenericArgument::Type(t) => Some(t),
            _ => None,
        });
        if let Some(t) = type_args.next() {
            role_type = parse_entity_ref_role(t)?;
        }
        if let Some(t) = type_args.next() {
            entity_type = parse_entity_ref_target(t)?;
        }
    }
    Ok(EventFieldType::EntityRef {
        role_type,
        entity_type,
    })
}

fn parse_usage(args: &syn::PathArguments) -> syn::Result<EventFieldType> {
    if let syn::PathArguments::AngleBracketed(args) = args
        && let Some(syn::GenericArgument::Type(t)) = args.args.first()
    {
        Ok(EventFieldType::ResourceUsage {
            resource: parse_type_ident(t)?,
        })
    } else {
        Err(syn::Error::new(
            args.span(),
            "invalid type name of non-Quent Usage type in event field type",
        ))
    }
}

fn parse_entity_ref_role(ty: &syn::Type) -> syn::Result<EntityRefRole> {
    let name = parse_type_ident(ty)?;
    Ok(match name.as_str() {
        "Plain" => EntityRefRole::Plain,
        "Scope" => EntityRefRole::Scope,
        _ => EntityRefRole::User(name),
    })
}

fn parse_entity_ref_target(ty: &syn::Type) -> syn::Result<EntityRefTarget> {
    let name = parse_type_ident(ty)?;
    Ok(match name.as_str() {
        "AnyEntity" => EntityRefTarget::Any,
        _ => EntityRefTarget::Specific(name),
    })
}

fn parse_type_ident(ty: &syn::Type) -> syn::Result<Identifier> {
    let syn::Type::Path(type_path) = ty else {
        return Err(syn::Error::new_spanned(ty, "expected a named path type"));
    };
    let last = type_path
        .path
        .segments
        .last()
        .ok_or_else(|| syn::Error::new_spanned(ty, "type has no path segments"))?;
    Identifier::try_from(last.ident.to_string().as_str())
        .map_err(|e| syn::Error::new_spanned(ty, e.to_string()))
}
