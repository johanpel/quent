// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of `AnyEvent`: a decoder from a type-erased event to the concrete
//! `Event<T>` for whichever entity produced it.

use convert_case::Case;
use proc_macro2::TokenStream;
use quent_schema::Schema;
use quote::quote;
use syn::Ident;

use crate::common::{derive_attr, raw_ident, to_case};
use crate::{GenerateError, Options};

/// Generate the `AnyEvent` enum and its `from_any` decoder.
///
/// One variant per entity, each holding a borrowed `Event<{Entity}Event>`.
/// `from_any` downcasts a `&dyn Any` (e.g. a callback exporter's type-erased
/// event) to the matching variant, so a single handler can dispatch over every
/// entity's events with an exhaustive match. The enum carries the same derives
/// as the event enums ([`Options::event_derives`]).
///
/// # Errors
///
/// Returns [`GenerateError`] if a derive entry is not a parseable Rust path.
pub(crate) fn generate_any_event(
    schema: &Schema,
    opts: &Options,
) -> Result<TokenStream, GenerateError> {
    let derives = derive_attr(opts.event_derives)?;

    // (variant name, event enum name) per entity.
    let variants: Vec<(Ident, Ident)> = schema
        .entities()
        .map(|entity| {
            let pascal = to_case(entity.name(), Case::Pascal);
            (
                raw_ident(pascal.clone()),
                raw_ident(format!("{pascal}Event")),
            )
        })
        .collect();

    let decls = variants.iter().map(|(variant, event)| {
        quote! { #variant(&'a ::quent_instrumentation::Event<#event>) }
    });
    let arms = variants.iter().map(|(variant, event)| {
        quote! {
            if let Some(event) = any.downcast_ref::<::quent_instrumentation::Event<#event>>() {
                return Some(AnyEvent::#variant(event));
            }
        }
    });

    Ok(quote! {
        /// A borrowed reference to any entity's event, recovered from a
        /// type-erased value. One variant per entity.
        #derives
        pub enum AnyEvent<'a> {
            #(#decls),*
        }

        impl<'a> AnyEvent<'a> {
            /// Decode a type-erased event into the concrete `Event<T>` for the
            /// entity that produced it, or `None` if it is not one of this
            /// schema's events.
            pub fn from_any(any: &'a (dyn ::core::any::Any)) -> Option<AnyEvent<'a>> {
                #(#arms)*
                None
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::pretty;
    use quent_schema::builder::{EntityBuilder, EventBuilder, SchemaBuilder};
    use quent_schema::{Cardinality, test_utils::ident};

    #[test]
    fn emits_a_variant_and_arm_per_entity() {
        let entity = EntityBuilder::new(ident("Query"))
            .try_with_event(EventBuilder::new(ident("submitted"), Cardinality::Once).build())
            .unwrap()
            .build();
        let schema = SchemaBuilder::new(ident("Demo"))
            .try_with_entity(entity)
            .unwrap()
            .build();
        let opts = Options {
            event_derives: &["Debug"],
            ..Options::default()
        };
        let src = pretty(generate_any_event(&schema, &opts).unwrap());
        assert!(src.contains("#[derive(Debug)]"));
        assert!(src.contains("pub enum AnyEvent"));
        assert!(src.contains("Query(&'a ::quent_instrumentation::Event<QueryEvent>)"));
        assert!(src.contains("downcast_ref::<::quent_instrumentation::Event<QueryEvent>>()"));
    }
}
