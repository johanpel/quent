// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of the schema model used by the generic instrumentation context.

use proc_macro2::TokenStream;
use quent_schema::Schema;
use quote::quote;

use super::model_ident;
use crate::common::relative_type_path;

/// Generate the model marker and its runtime integration.
pub(super) fn schema_model(schema: &Schema) -> TokenStream {
    let model = model_ident(schema);
    let model_name = schema.name().to_string();

    let event_tys: Vec<_> = schema
        .entities()
        .map(|entity| relative_type_path(entity.path(), &[], "Event"))
        .collect();

    let model_doc = format!("The `{model_name}` instrumentation model.");

    quote! {
        #[doc = #model_doc]
        pub struct #model;

        impl ::quent_instrumentation::Model for #model {
            #[allow(clippy::type_complexity)]
            type Observers = (
                #(
                    ::std::sync::Arc<::quent_instrumentation::EventPipeline<#event_tys>>,
                )*
            );

            fn build_observers(
                context: &::quent_instrumentation::ContextInner,
                exporter: ::core::option::Option<&::quent_instrumentation::ExporterOptions>,
            ) -> ::core::result::Result<
                Self::Observers,
                ::std::boxed::Box<dyn ::std::error::Error>,
            > {
                match exporter {
                    ::core::option::Option::Some(options) => {
                        context.block_on(async {
                            ::core::result::Result::<
                                _,
                                ::std::boxed::Box<dyn ::std::error::Error>,
                            >::Ok((
                                #(
                                    ::std::sync::Arc::new(
                                        context
                                            .observer::<#event_tys>(
                                                ::core::clone::Clone::clone(options),
                                            )
                                            .await?,
                                    ),
                                )*
                            ))
                        })
                    }
                    ::core::option::Option::None => ::core::result::Result::Ok((
                            #(
                                ::std::sync::Arc::new(
                                    ::quent_instrumentation::EventPipeline::<#event_tys>::noop(),
                                ),
                            )*
                        )),
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
