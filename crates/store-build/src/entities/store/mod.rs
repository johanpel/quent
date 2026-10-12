// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of entity stores across supported backends.

use proc_macro2::TokenStream;
use quent_schema::{Entity, Event};

use super::EventCode;

mod native;

/// Store fragments for each supported backend of one schema event.
pub(super) struct EventStoreCode {
    /// Fragments for the native in-memory backend.
    native: native::NativeEventStoreCode,
}

pub(super) fn generate_event(
    entity: &Entity,
    event: &Event,
    handle: &syn::Ident,
    method: &syn::Ident,
    variant_pattern: &TokenStream,
    accessor_method_signature: &TokenStream,
) -> EventStoreCode {
    EventStoreCode {
        native: native::generate_event(
            entity,
            event,
            handle,
            method,
            variant_pattern,
            accessor_method_signature,
        ),
    }
}

pub(super) fn generate_entity(
    entity: &Entity,
    native: &syn::Ident,
    store: &syn::Ident,
    event_index_ident: &syn::Ident,
    access: &syn::Ident,
    marker: &TokenStream,
    events: &[EventCode],
) -> TokenStream {
    native::generate_entity(
        entity,
        native,
        store,
        event_index_ident,
        access,
        marker,
        events,
    )
}
