// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of filesystem event-loader import descriptors.

use proc_macro2::TokenStream;
use quent_schema::Schema;
use quote::quote;

/// Generates code that lets the model load events from the filesystem.
///
/// It lists one event importer for each entity, in schema order.
///
/// For example, a model `Demo` with one entity `Task` gets:
///
/// ```ignore
/// impl ::quent_store::event::filesystem::Model for Demo {
///     const EVENT_IMPORTERS:
///         &'static [::quent_store::event::filesystem::EventImporter<Self>] = &[
///             ::quent_store::event::filesystem::EventImporter::<Self>::
///                 import_for_entity::<Task>(),
///         ];
/// }
/// ```
pub(super) fn generate_model_impl(schema: &Schema) -> TokenStream {
    let model = quent_instrumentation_build::generated_model_path(schema);
    let importers = schema.entities().map(|entity| {
        let marker = quent_instrumentation_build::generated_entity_path(entity);
        quote! {
            ::quent_store::event::filesystem::EventImporter::<Self>::import_for_entity::<#marker>()
        }
    });
    quote! {
        impl ::quent_store::event::filesystem::Model for #model {
            const EVENT_IMPORTERS:
                &'static [::quent_store::event::filesystem::EventImporter<Self>] = &[
                    #(#importers,)*
                ];
        }
    }
}
