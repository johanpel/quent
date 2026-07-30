// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of the schema model used by the generic instrumentation context.

use convert_case::Case;
use proc_macro2::TokenStream;
use quent_schema::{Entity, Schema};
use quote::quote;
use syn::Ident;

use super::{event_ident, marker_ident, model_ident};
use crate::GenerateError;
use crate::common::{raw_ident, to_case};

/// Generates the model's typed observer storage.
pub(super) fn observer_storage(schema: &Schema) -> Result<TokenStream, GenerateError> {
    let storage = observers_ident(schema);
    let storage_name = storage.to_string();
    if let Some(schema_path) = schema
        .records()
        .map(|record| record.path())
        .chain(schema.entities().map(|entity| entity.path()))
        .find(|path| to_case(path.name(), Case::Pascal) == storage_name)
    {
        return Err(GenerateError::GeneratedTypeCollision {
            generated: storage_name,
            schema_path: schema_path.clone(),
        });
    }

    let description = format!(
        "Observers for the `{}` instrumentation model.",
        schema.name()
    );
    let hidden_docs = "Hidden because the model context provides typed observer access.";
    let entity_fields = schema.entities().map(|entity| {
        let field = entity_observer_field(entity);
        let entity_ty = marker_ident(entity);
        quote! {
            #field: ::quent_instrumentation::Observer<#entity_ty>
        }
    });

    Ok(quote! {
        #[doc = #description]
        #[doc = ""]
        #[doc = #hidden_docs]
        #[doc(hidden)]
        pub struct #storage {
            #(#entity_fields,)*
        }
    })
}

/// Generates the model marker and its runtime integration.
pub(super) fn schema_model(schema: &Schema) -> TokenStream {
    let model = model_ident(schema);
    let model_name = schema.name().to_string();
    let observers = observers_ident(schema);
    let active_observers = observer_storage_initializer(schema, true);
    let noop_observers = observer_storage_initializer(schema, false);
    let options_binding = if schema.entities().next().is_some() {
        raw_ident("options".to_owned())
    } else {
        raw_ident("_options".to_owned())
    };
    let observer_impls = schema
        .entities()
        .map(|entity| observer_storage_impl(schema, entity));

    let model_doc = format!("The `{model_name}` instrumentation model.");

    quote! {
        #[doc = #model_doc]
        pub struct #model;

        #(#observer_impls)*

        impl ::quent_instrumentation::Model for #model {
            type Observers = #observers;

            fn build_observers(
                context: &::quent_instrumentation::ContextInner,
                exporter: ::core::option::Option<&::quent_instrumentation::ExporterOptions>,
            ) -> ::core::result::Result<
                Self::Observers,
                ::std::boxed::Box<dyn ::std::error::Error>,
            > {
                match exporter {
                    ::core::option::Option::Some(#options_binding) => {
                        context.block_on(async {
                            ::core::result::Result::<
                                _,
                                ::std::boxed::Box<dyn ::std::error::Error>,
                            >::Ok(#active_observers)
                        })
                    }
                    ::core::option::Option::None => {
                        ::core::result::Result::Ok(#noop_observers)
                    }
                }
            }

            fn model_info() -> ::quent_instrumentation::build_info::ModelInfo {
                ::quent_instrumentation::build_info::ModelInfo {
                    name: #model_name.to_string(),
                    package: env!("CARGO_PKG_NAME").to_string(),
                    // No umbrella event enum on the schema-driven path; record
                    // the module the generated library is included into.
                    type_path: module_path!().to_string(),
                    source: ::quent_instrumentation::build_info::source_or_quent(
                        env!("CARGO_PKG_VERSION"),
                        option_env!("QUENT_SOURCE_REMOTE"),
                        option_env!("QUENT_SOURCE_COMMIT"),
                        option_env!("QUENT_SOURCE_BRANCH"),
                        option_env!("QUENT_SOURCE_DIRTY"),
                        option_env!("QUENT_SOURCE_BUILT_AT"),
                    ),
                    // The schema declares no analyzer entry.
                    analyzer_package: ::core::option::Option::None,
                }
            }
        }
    }
}

fn observer_storage_initializer(schema: &Schema, active: bool) -> TokenStream {
    let storage = observers_ident(schema);
    let entity_fields = schema.entities().map(|entity| {
        let field = entity_observer_field(entity);
        let entity_ty = marker_ident(entity);
        let event_ty = event_ident(entity);
        let observer = if active {
            quote! {
                context
                    .observer::<#event_ty>(::core::clone::Clone::clone(options))
                    .await?
            }
        } else {
            quote! {
                ::quent_instrumentation::ObserverInner::<#event_ty>::noop()
            }
        };
        quote! {
            #field: ::quent_instrumentation::Observer::<#entity_ty>::new(
                ::std::sync::Arc::new(#observer),
            )
        }
    });
    quote! {
        #storage {
            #(#entity_fields,)*
        }
    }
}

fn observer_storage_impl(schema: &Schema, entity: &Entity) -> TokenStream {
    let storage = observers_ident(schema);
    let entity_ty = marker_ident(entity);
    let field = entity_observer_field(entity);

    quote! {
        impl ::quent_instrumentation::ObserverProvider<#entity_ty> for #storage {
            fn observer(&self) -> ::quent_instrumentation::Observer<#entity_ty> {
                ::core::clone::Clone::clone(&self.#field)
            }
        }
    }
}

fn observers_ident(schema: &Schema) -> Ident {
    raw_ident(format!("{}Observers", to_case(schema.name(), Case::Pascal)))
}

fn entity_observer_field(entity: &Entity) -> Ident {
    raw_ident(format!(
        "{}_observer",
        to_case(entity.path().name(), Case::Snake)
    ))
}
