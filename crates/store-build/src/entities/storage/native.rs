// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of native in-memory event storage.

use proc_macro2::TokenStream;
use quent_schema::{Cardinality, Entity, Event};
use quote::quote;

use super::super::EventCode;

/// Native in-memory storage, conversion, and accessor implementation fragments.
pub(super) struct NativeEventStorageCode {
    /// A native storage field holding an optional event or a vector of events.
    ///
    /// For example, `event_init: Option<Event<InitPayload>>`.
    pub(super) storage_field_def: TokenStream,
    /// A field initializer creating empty native storage.
    ///
    /// For example, `event_init: Default::default()`.
    pub(super) storage_field_init: TokenStream,
    /// A match arm moving an enum variant into native storage and checking
    /// cardinality.
    ///
    /// For example, a multi-event arm:
    /// ```text
    /// TaskEvent::Tick => {
    ///     result.event_tick.push(Event::new(id, ts, TickPayload {}));
    /// }
    /// ```
    pub(super) conversion_match_arm: TokenStream,
    /// The native storage implementation of the trait accessor.
    ///
    /// For example:
    /// ```text
    /// fn init(&self) -> Option<&Event<InitPayload>> {
    ///     self.event_init.as_ref()
    /// }
    /// ```
    pub(super) accessor_method_impl: TokenStream,
}

pub(super) fn generate_event(
    event: &Event,
    payload: &syn::Ident,
    method: &syn::Ident,
    variant_pattern: &TokenStream,
    event_constructor_expr: &TokenStream,
) -> NativeEventStorageCode {
    let slot = quote::format_ident!("event_{}", method.to_string().trim_start_matches("r#"));
    let storage_field_init = quote! { #slot: ::core::default::Default::default() };
    let (storage_field_def, conversion_match_arm, accessor_method_impl) = if event.cardinality()
        == Cardinality::Once
    {
        let event_name = event.name().to_string();
        (
            quote! { #slot: ::core::option::Option<::quent_events::Event<#payload>> },
            quote! { #variant_pattern => {
                ::quent_store::entity::insert_once(&mut result.#slot, #event_constructor_expr)
                    .map_err(|event| ::quent_store::entity::DuplicateOnceEvent {
                        entity_id: event.id,
                        event_name: #event_name,
                    })?;
            } },
            quote! { fn #method(&self) -> ::core::option::Option<&::quent_events::Event<#payload>> { self.#slot.as_ref() } },
        )
    } else {
        (
            quote! { #slot: ::std::vec::Vec<::quent_events::Event<#payload>> },
            quote! { #variant_pattern => { result.#slot.push(#event_constructor_expr); } },
            quote! { fn #method(&self) -> impl ::core::iter::Iterator<Item = &::quent_events::Event<#payload>> { self.#slot.iter() } },
        )
    };
    NativeEventStorageCode {
        storage_field_def,
        storage_field_init,
        conversion_match_arm,
        accessor_method_impl,
    }
}

pub(super) fn generate_entity(
    entity: &Entity,
    native: &syn::Ident,
    access: &syn::Ident,
    marker: &TokenStream,
    events: &[EventCode],
) -> TokenStream {
    let storage = events
        .iter()
        .map(|event| &event.storage.native.storage_field_def);
    let defaults = events
        .iter()
        .map(|event| &event.storage.native.storage_field_init);
    let arms = events
        .iter()
        .map(|event| &event.storage.native.conversion_match_arm);
    let implementations = events
        .iter()
        .map(|event| &event.storage.native.accessor_method_impl);
    let docs = format!("Owns events grouped by event type for `{}`.", entity.path());
    quote! {
        #[doc = #docs]
        ///
        /// Conversion consumes the sequence without cloning payloads and rejects duplicate once-events.
        pub struct #native {
            id: ::quent_events::Uuid,
            #(#storage,)*
        }
        impl ::quent_store::entity::EntityHandle for #native {
            type Entity = #marker;
            fn id(&self) -> ::quent_events::Uuid { self.id }
        }
        impl #access for #native { #(#implementations)* }
        impl ::core::convert::TryFrom<::quent_store::entity::sequence::EventSequence<#marker>> for #native {
            type Error = ::quent_store::entity::DuplicateOnceEvent;
            fn try_from(sequence: ::quent_store::entity::sequence::EventSequence<#marker>) -> ::core::result::Result<Self, Self::Error> {
                let mut result = Self { id: sequence.id(), #(#defaults,)* };
                for ::quent_events::Event { id: event_id, timestamp: event_timestamp, data } in sequence.into_events() {
                    match data { #(#arms,)* }
                }
                Ok(result)
            }
        }
    }
}
