// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of the schema context — builds every entity's observer on
//! construction and hands out cheap clones.

use convert_case::Case;
use proc_macro2::TokenStream;
use quent_schema::Schema;
use quote::quote;

use super::{event_ident, observer_ident};
use crate::common::{raw_ident, to_case};

/// The `{Schema}Context`: builds one observer per entity on construction and
/// hands out cheap clones via `{entity}_observer()`.
pub(super) fn schema_context(schema: &Schema) -> TokenStream {
    let schema_pascal = to_case(schema.name(), Case::Pascal);
    let context_ty = raw_ident(format!("{schema_pascal}Context"));
    let model_name = schema.name().to_string();

    let fields: Vec<_> = schema
        .entities()
        .map(|e| raw_ident(to_case(e.name(), Case::Snake)))
        .collect();
    let observer_tys: Vec<_> = schema.entities().map(observer_ident).collect();
    let event_tys: Vec<_> = schema.entities().map(event_ident).collect();
    let accessors: Vec<_> = schema
        .entities()
        .map(|e| raw_ident(format!("{}_observer", to_case(e.name(), Case::Snake))))
        .collect();
    let accessor_docs: Vec<String> = schema
        .entities()
        .map(|e| {
            format!(
                "Observer for `{}` entities.",
                to_case(e.name(), Case::Pascal)
            )
        })
        .collect();

    let context_doc = format!(
        "Instrumentation context for the `{model_name}` model. Construct it with \
         [`Self::try_new`], then call a `*_observer()` accessor to get an entity's \
         event observer, which creates the per-instance handles that emit events."
    );

    quote! {
        #[doc = #context_doc]
        pub struct #context_ty {
            #(#fields: #observer_tys,)*
            _inner: ::quent_instrumentation_runtime::Context,
        }

        impl #context_ty {
            /// Create a context, building every entity's exporter pipeline.
            /// Pass `None` for a no-op context that discards events.
            pub fn try_new(
                exporter: ::core::option::Option<::quent_instrumentation_runtime::ExporterOptions>,
            ) -> ::core::result::Result<Self, ::std::boxed::Box<dyn ::std::error::Error>> {
                Self::assemble(::quent_instrumentation_runtime::Context::try_new(
                    Self::model_info(),
                    exporter,
                )?)
            }

            /// Create a context that adopts an existing `id` rather than
            /// generating one.
            pub fn try_with_id(
                id: ::uuid::Uuid,
                exporter: ::core::option::Option<::quent_instrumentation_runtime::ExporterOptions>,
            ) -> ::core::result::Result<Self, ::std::boxed::Box<dyn ::std::error::Error>> {
                Self::assemble(::quent_instrumentation_runtime::Context::try_with_id(
                    id,
                    Self::model_info(),
                    exporter,
                )?)
            }

            fn model_info() -> ::quent_instrumentation_runtime::build_info::ModelInfo {
                ::quent_instrumentation_runtime::build_info::ModelInfo {
                    name: #model_name.to_string(),
                    package: env!("CARGO_PKG_NAME").to_string(),
                    // No umbrella event enum on the schema-driven path; record
                    // the module the generated library is included into.
                    type_path: module_path!().to_string(),
                    source: ::quent_instrumentation_runtime::build_info::source_or_quent(
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

            fn assemble(
                inner: ::quent_instrumentation_runtime::Context,
            ) -> ::core::result::Result<Self, ::std::boxed::Box<dyn ::std::error::Error>> {
                let ( #(#fields,)* ) = inner.block_on(async {
                    ::core::result::Result::<_, ::std::boxed::Box<dyn ::std::error::Error>>::Ok((
                        #( inner.observer::<#event_tys>().await?, )*
                    ))
                })?;
                ::core::result::Result::Ok(Self {
                    #( #fields: #observer_tys { inner: ::std::sync::Arc::new(#fields) }, )*
                    _inner: inner,
                })
            }

            /// Identity of this context.
            pub fn id(&self) -> ::uuid::Uuid {
                self._inner.id()
            }

            #(
                #[doc = #accessor_docs]
                pub fn #accessors(&self) -> #observer_tys {
                    ::core::clone::Clone::clone(&self.#fields)
                }
            )*
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::pretty;
    use quent_schema::DataType;
    use quent_schema::builder::SchemaBuilder;
    use quent_schema::test_utils::{entity, event, field, ident};

    #[test]
    fn context_builds_and_exposes_one_observer_per_entity() {
        let s = SchemaBuilder::new(ident("Demo"))
            .entities([
                entity(
                    "Connection",
                    [event("data", [field("bytes", DataType::U64)])],
                ),
                entity("Sensor", [event("reading", [field("v", DataType::F64)])]),
            ])
            .unwrap()
            .build();
        let src = pretty(schema_context(&s));
        assert!(src.contains("pub struct DemoContext"));
        assert!(src.contains("connection: ConnectionObserver"));
        assert!(src.contains("sensor: SensorObserver"));
        assert!(src.contains("inner.observer::<ConnectionEvent>().await?"));
        assert!(src.contains("inner.observer::<SensorEvent>().await?"));
        assert!(src.contains("pub fn connection_observer(&self) -> ConnectionObserver"));
        assert!(src.contains("pub fn sensor_observer(&self) -> SensorObserver"));
        assert!(src.contains(r#"name: "Demo".to_string()"#));
    }
}
