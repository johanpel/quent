// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of native in-memory event storage.

use proc_macro2::TokenStream;
use quent_schema::{Cardinality, Entity, Event};
use quote::quote;

use super::super::EventCode;

/// Native storage, insertion, and accessor implementation fragments.
pub(super) struct NativeEventStorageCode {
    /// A field holding an optional position or a vector of positions.
    ///
    /// For example, `event_init: Option<usize>`.
    pub(super) storage_field_def: TokenStream,
    /// A match arm recording the position and checking cardinality.
    ///
    /// For example, `TaskEvent::Tick => self.event_tick.push(position)`.
    pub(super) conversion_match_arm: TokenStream,
    /// The native implementation returning borrowing handles.
    ///
    /// For example:
    /// ```text
    /// fn init(&self) -> Option<InitHandle<'_>> {
    ///     self.event_index().event_init.map(|position| InitHandle {
    ///         event: &self.event_sequence().events()[position],
    ///     })
    /// }
    /// ```
    pub(super) accessor_method_impl: TokenStream,
}

pub(super) fn generate_event(
    entity: &Entity,
    event: &Event,
    handle: &syn::Ident,
    method: &syn::Ident,
    variant_pattern: &TokenStream,
    accessor_method_signature: &TokenStream,
) -> NativeEventStorageCode {
    let slot = quote::format_ident!("event_{}", method.to_string().trim_start_matches("r#"));
    // TODO(johanpel): Consider u32 positions with u32::MAX for absent Once events,
    // and block-based delta encoding for large Multi groups.
    let (storage_field_def, conversion_match_arm, accessor_method_impl) = if event.cardinality()
        == Cardinality::Once
    {
        let event_name = event.name().to_string();
        let accessor_expr = if entity.events().count() == 1 {
            quote! { #handle {
                event: &self.event_sequence().events()[self.event_index().#slot.expect("non-empty entity must contain its sole event")],
            } }
        } else {
            quote! { self.event_index().#slot.map(|position| #handle {
                event: &self.event_sequence().events()[position],
            }) }
        };
        (
            quote! { #slot: ::core::option::Option<usize> },
            quote! { #variant_pattern => {
                ::quent_store::entity::native::insert_once(&mut self.#slot, position)
                    .map_err(|_| ::quent_store::Error::DuplicateOnceEvent {
                        entity_id: event.id,
                        event_name: #event_name,
                    })?;
            } },
            quote! { #accessor_method_signature { #accessor_expr } },
        )
    } else {
        (
            quote! { #slot: ::std::vec::Vec<usize> },
            quote! { #variant_pattern => { self.#slot.push(position); } },
            quote! { #accessor_method_signature {
                self.event_index().#slot.iter().map(|&position| #handle {
                    event: &self.event_sequence().events()[position],
                })
            } },
        )
    };
    NativeEventStorageCode {
        storage_field_def,
        conversion_match_arm,
        accessor_method_impl,
    }
}

pub(super) fn generate_entity(
    entity: &Entity,
    native: &syn::Ident,
    store: &syn::Ident,
    event_storage_ident: &syn::Ident,
    access: &syn::Ident,
    marker: &TokenStream,
    events: &[EventCode],
) -> TokenStream {
    let storage = events
        .iter()
        .map(|event| &event.storage.native.storage_field_def);
    let arms = events
        .iter()
        .map(|event| &event.storage.native.conversion_match_arm);
    let implementations = events
        .iter()
        .map(|event| &event.storage.native.accessor_method_impl)
        .collect::<Vec<_>>();
    let store_docs = format!("Stores indexed events for `{}` entities.", entity.path());
    let docs = format!("Owns an indexed event sequence for `{}`.", entity.path());
    let storage_docs = format!("Event-type indices for `{}`.", entity.path());
    quote! {
        #[doc = #storage_docs]
        #[doc(hidden)]
        #[derive(Default)]
        pub struct #event_storage_ident {
            #(#storage,)*
        }
        impl ::quent_store::entity::native::EventIndex<#marker> for #event_storage_ident {
            fn push(&mut self, event: &::quent_events::Event<<#marker as ::quent_events::EntityMarker>::Payload>, position: usize) -> ::core::result::Result<(), ::quent_store::Error> {
                match &event.data { #(#arms,)* }
                Ok(())
            }
        }
        #[doc = #docs]
        ///
        /// Conversion rejects duplicate once-events.
        pub type #native = ::quent_store::entity::native::NativeEntity<#marker, #event_storage_ident>;
        impl #access for #native { #(#implementations)* }
        impl #access for &#native { #(#implementations)* }
        #[doc = #store_docs]
        pub type #store = ::quent_store::entity::native::Store<#marker, #native>;
    }
}
