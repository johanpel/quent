// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! #[derive(Entity)] implementation

use proc_macro2::TokenStream;
use quent_v2_model_ir::{
    entity::Entity,
    identifier::{Identifier, IdentifierError},
};
use quote::quote;
use syn::DeriveInput;

mod instrumentation;
mod ir;
mod parse;

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let name_ir: Identifier = name
        .to_string()
        .try_into()
        .map_err(|e: IdentifierError| syn::Error::new(name.span(), e.to_string()))?;

    // Fail quickly if there are generics or if this is used on a union.
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "#[derive(Entity)] does not support generics",
        ));
    }
    if let syn::Data::Union(u) = input.data {
        return Err(syn::Error::new_spanned(
            u.union_token,
            "#[derive(Entity)] not supported for union, use struct or enum",
        ));
    }

    // Parse into IR entity.
    let attributes: Vec<&syn::Attribute> = input
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("quent"))
        .collect();
    let docs = crate::docs::extract_docs(&input.attrs);
    let events = parse::events::parse(&input)?;
    let (fsm, convention_arms) = parse::conventions::parse(&attributes)?;
    // The conventions map is populated at runtime via the emitted IR. At
    // macro time, only FSM topology checks run (the convention data is an
    // opaque token expression yet to evaluate).
    let entity = Entity {
        name: name_ir,
        docs,
        events,
        fsm,
        conventions: Vec::new(),
    };
    if entity.fsm.is_some()
        && let Err(errs) = quent_v2_validation::fsm::validate(&entity)
    {
        return Err(syn::Error::new_spanned(name, errs.join("\n")));
    }

    // Emit the instrumentation library.
    let instrumentation = if cfg!(feature = "instrumentation") {
        match &input.data {
            syn::Data::Struct(s) => instrumentation::expand_struct(name, &s.fields, &input),
            syn::Data::Enum(e) => instrumentation::expand_enum(name, &e.variants, &input, &entity),
            syn::Data::Union(_) => unreachable!(),
        }?
    } else {
        TokenStream::new()
    };

    // Emit the IR trait impls.
    let ir = if cfg!(feature = "ir") {
        match &input.data {
            syn::Data::Struct(s) => {
                ir::expand_struct(name, &s.fields, &input, &entity, &convention_arms)
            }
            syn::Data::Enum(e) => {
                ir::expand_enum(name, &e.variants, &input, &entity, &convention_arms)
            }
            syn::Data::Union(_) => unreachable!(),
        }?
    } else {
        TokenStream::new()
    };

    Ok(quote! {
        #instrumentation

        #ir
    })
}
