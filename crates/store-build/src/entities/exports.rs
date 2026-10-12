// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Re-exports of entity access traits and stores beside their markers.

use quent_instrumentation_build::{generated_module_ident, generated_type_ident};
use quent_schema::Entity;
use quote::quote;

use crate::{EntityStoreImplementations, GenerateError};

pub(super) fn generate(
    entity: &Entity,
    events: &mut syn::File,
    stores: &EntityStoreImplementations,
) -> Result<(), GenerateError> {
    let namespace = entity
        .path()
        .namespace()
        .iter()
        .map(generated_module_ident)
        .collect::<Vec<_>>();
    let mut items = &mut events.items;
    for segment in &namespace {
        items = items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Mod(module) if module.ident == *segment => {
                    module.content.as_mut().map(|(_, items)| items)
                }
                _ => None,
            })
            .expect("event generation emits schema namespaces");
    }
    let name = generated_type_ident(entity.path().name()).to_string();
    let name = name.trim_start_matches("r#");
    let access = quote::format_ident!("{}Events", name);
    let store = quote::format_ident!("{}Store", name);
    let mut exports = vec![access];
    if stores.native {
        exports.push(store);
    }
    for export in &exports {
        let conflict = items.iter().any(|item| {
            let ident = match item {
                syn::Item::Struct(item) => &item.ident,
                syn::Item::Enum(item) => &item.ident,
                syn::Item::Type(item) => &item.ident,
                syn::Item::Trait(item) => &item.ident,
                syn::Item::Mod(item) => &item.ident,
                _ => return false,
            };
            ident == export
        });
        if conflict {
            return Err(GenerateError::EntityEventsNameConflict {
                name: export.to_string(),
                first: "existing generated type or namespace".into(),
                second: format!("re-export for entity {}", entity.path()),
            });
        }
    }
    let parents = vec![quote!(super); namespace.len()];
    let prefix = if parents.is_empty() {
        quote! { self:: }
    } else {
        quote! { #(#parents::)* }
    };
    let module = generated_module_ident(entity.path().name());
    items.push(syn::parse_quote! {
        pub use #prefix entity_events::#(#namespace::)*#module::{#(#exports),*};
    });
    Ok(())
}
