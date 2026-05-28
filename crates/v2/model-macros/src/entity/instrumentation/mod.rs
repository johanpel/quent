// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use convert_case::{Case, Casing};
use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quent_v2_model_ir::entity::Entity;
use quote::{format_ident, quote};
use syn::{DeriveInput, Token, Variant, punctuated::Punctuated};

mod fsm;
mod plain;

// Structs can only produce plain (non-fsm) entities.
pub fn expand_struct(
    name: &syn::Ident,
    fields: &syn::Fields,
    input: &DeriveInput,
) -> syn::Result<TokenStream> {
    let name_string = name.to_string();
    if matches!(fields, syn::Fields::Unnamed(_)) {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Entity)] on a struct requires a unit struct or a struct with named fields",
        ));
    }

    let vis = &input.vis;
    let observer_name = format_ident!("{}Observer", name);
    let handle_name = format_ident!("{}Handle", name);

    let observer = plain::emit_observer(name, &observer_name, &handle_name, vis);

    let method_name = format_ident!("{}", name_string.to_case(Case::Snake));

    let (event_arg, event_value) = match fields {
        syn::Fields::Unit => (quote! {}, quote! { #name }),
        syn::Fields::Named(n) if n.named.is_empty() => (quote! {}, quote! { #name {} }),
        _ => (quote! { payload: #name, }, quote! { payload }),
    };

    Ok(quote! {
        #observer

        #vis struct #handle_name {
            inner: ::quent_v2_instrumentation::handle::Handle<#name>,
        }

        impl #handle_name {
            #vis fn #method_name(
                &self,
                #event_arg
            ) -> ::std::result::Result<(), ::quent_v2_instrumentation::ObserverError> {
                self.inner.emit(#event_value)
            }
        }

        impl ::quent_v2_model::EntityHandle for #handle_name {
            type EntityType = #name;
            fn id(&self) -> ::uuid::Uuid {
                self.inner.id()
            }
        }
    })
}

pub fn expand_enum(
    name: &syn::Ident,
    variants: &Punctuated<Variant, Token![,]>,
    input: &DeriveInput,
    entity: &Entity,
) -> syn::Result<TokenStream> {
    if variants.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Entity)] requires the enum to have at least one variant, otherwise it would represent an entity without an event",
        ));
    }

    // Build an ordered map of state name to for enum variants, used in multiple emission paths.
    let variants: IndexMap<String, &Variant> =
        variants.iter().map(|v| (v.ident.to_string(), v)).collect();

    let vis = &input.vis;
    let observer_name = format_ident!("{}Observer", name);
    let handle_name = format_ident!("{}Handle", name);

    let (observer, handle) = if let Some(fsm) = &entity.fsm {
        (
            fsm::emit_observer(name, &observer_name, &handle_name, vis, &variants, fsm)?,
            fsm::emit_handle(name, &handle_name, vis, &variants, fsm)?,
        )
    } else {
        (
            plain::emit_observer(name, &observer_name, &handle_name, vis),
            plain::emit_handle(name, &handle_name, vis, &variants)?,
        )
    };

    Ok(quote! {
        #observer

        #handle
    })
}
