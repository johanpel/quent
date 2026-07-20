// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of Parquet companion rows for entity event streams.

use convert_case::Case;
use proc_macro2::{Ident, TokenStream};
use quent_schema::{DataType, Entity, Event, Record, Schema};
use quote::{format_ident, quote};

use crate::Options;
use crate::common::{raw_ident, to_case};
use crate::data_type::map_data_type;

pub(crate) fn generate(schema: &Schema, opts: &Options) -> TokenStream {
    if !opts.parquet {
        return quote! {};
    }

    let records = record_companions(schema);
    let events = schema.entities().map(entity_event_companion);
    quote! {
        #records
        #(#events)*
    }
}

fn record_companions(schema: &Schema) -> TokenStream {
    let records = schema.records().map(record_companion).collect::<Vec<_>>();
    let implementations = schema.records().map(record_impl).collect::<Vec<_>>();

    quote! {
        #[doc(hidden)]
        pub mod __quent_parquet_records {
            use ::quent_instrumentation::parquet::narrow;
            use super::*;

            #(#records)*
        }

        #(#implementations)*
    }
}

fn record_companion(record: &Record) -> TokenStream {
    if record.fields().next().is_none() {
        return quote! {};
    }

    let name = raw_ident(to_case(record.name(), Case::Pascal));
    let companion = format_ident!("{name}Parquet");
    let fields = record.fields().map(|field| {
        let name = raw_ident(to_case(field.name(), Case::Snake));
        let ty = parquet_type(field.ty());
        quote! { pub #name: #ty }
    });

    quote! {
        #[derive(narrow::ArrayType, Default)]
        pub struct #companion {
            #(#fields,)*
        }
    }
}

fn record_impl(record: &Record) -> TokenStream {
    let name = raw_ident(to_case(record.name(), Case::Pascal));
    let companion = format_ident!("{name}Parquet");
    let fields = record
        .fields()
        .map(|field| (raw_ident(to_case(field.name(), Case::Snake)), field.ty()))
        .collect::<Vec<_>>();

    if fields.is_empty() {
        return quote! {
            impl ::quent_instrumentation::parquet::ParquetValue for #name {
                type Value = bool;

                fn into_parquet(self) -> Self::Value {
                    let Self {} = self;
                    true
                }
            }
        };
    }

    let field_names = fields.iter().map(|(name, _)| name).collect::<Vec<_>>();
    let values = fields
        .iter()
        .map(|(name, ty)| parquet_value(ty, name))
        .collect::<Vec<_>>();

    quote! {
        impl ::quent_instrumentation::parquet::ParquetValue for #name {
            type Value = __quent_parquet_records::#companion;

            fn into_parquet(self) -> Self::Value {
                let Self { #(#field_names,)* } = self;
                __quent_parquet_records::#companion {
                    #(
                        #field_names: #values,
                    )*
                }
            }
        }
    }
}

fn entity_event_companion(entity: &Entity) -> TokenStream {
    let entity_name = to_case(entity.name(), Case::Pascal);
    let event_enum = raw_ident(format!("{entity_name}Event"));
    let module = format_ident!(
        "__quent_parquet_{}_event",
        to_case(entity.name(), Case::Snake)
    );
    let row = format_ident!("{entity_name}EventParquetRow");
    let payloads = entity.events().map(event_payload).collect::<Vec<_>>();
    let fields = entity
        .events()
        .map(|event| raw_ident(to_case(event.name(), Case::Snake)))
        .collect::<Vec<_>>();
    let payload_types = entity
        .events()
        .map(|event| {
            if event.fields().next().is_none() {
                quote! { bool }
            } else {
                let payload = format_ident!("{}Parquet", to_case(event.name(), Case::Pascal));
                quote! { #payload }
            }
        })
        .collect::<Vec<_>>();
    let arms = entity
        .events()
        .enumerate()
        .map(|(active, event)| event_arm(&event_enum, &module, &row, event, &fields, active))
        .collect::<Vec<_>>();

    quote! {
        #[doc(hidden)]
        pub mod #module {
            use ::quent_instrumentation::parquet::narrow;
            use super::*;

            #(#payloads)*

            #[derive(narrow::ArrayType)]
            pub struct #row {
                pub id: ::quent_instrumentation::Uuid,
                pub timestamp: u64,
                pub event: String,
                #(
                    pub #fields: Option<#payload_types>,
                )*
            }
        }

        impl ::quent_instrumentation::parquet::ParquetEvent for #event_enum {
            fn parquet_schema() -> ::quent_instrumentation::parquet::ExporterResult<
                ::std::sync::Arc<
                    ::quent_instrumentation::parquet::narrow::arrow_schema::Schema,
                >,
            > {
                Ok(::std::sync::Arc::new(
                    ::quent_instrumentation::parquet::narrow::array::StructArray::<
                        #module::#row
                    >::schema(),
                ))
            }

            fn into_record_batch(
                events: Vec<::quent_instrumentation::Event<Self>>,
            ) -> ::quent_instrumentation::parquet::ExporterResult<
                ::quent_instrumentation::parquet::narrow::arrow_array::RecordBatch,
            > {
                let rows = events.into_iter().map(|event| {
                    let ::quent_instrumentation::Event { id, timestamp, data } = event;
                    match data {
                        #(#arms,)*
                    }
                });
                Ok(::quent_instrumentation::parquet::narrow::arrow_array::RecordBatch::from(
                    rows.collect::<
                        ::quent_instrumentation::parquet::narrow::array::StructArray<
                            #module::#row
                        >,
                    >(),
                ))
            }
        }
    }
}

fn event_payload(event: &Event) -> TokenStream {
    if event.fields().next().is_none() {
        return quote! {};
    }

    let payload = format_ident!("{}Parquet", to_case(event.name(), Case::Pascal));
    let fields = event.fields().map(|field| {
        let name = raw_ident(to_case(field.name(), Case::Snake));
        let ty = parquet_type(field.ty());
        quote! { pub #name: #ty }
    });

    quote! {
        #[derive(narrow::ArrayType, Default)]
        pub struct #payload {
            #(#fields,)*
        }
    }
}

fn event_arm(
    event_enum: &Ident,
    module: &Ident,
    row: &Ident,
    event: &Event,
    fields: &[Ident],
    active: usize,
) -> TokenStream {
    let variant = raw_ident(to_case(event.name(), Case::Pascal));
    let payload = format_ident!("{}Parquet", to_case(event.name(), Case::Pascal));
    let event_name = event.name().to_string();
    let event_fields = event
        .fields()
        .map(|field| (raw_ident(to_case(field.name(), Case::Snake)), field.ty()))
        .collect::<Vec<_>>();
    let event_field_names = event_fields
        .iter()
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    let pattern = if event_fields.is_empty() {
        quote! { #event_enum::#variant }
    } else {
        quote! { #event_enum::#variant { #(#event_field_names,)* } }
    };
    let payload_value = if event_fields.is_empty() {
        quote! { true }
    } else {
        let values = event_fields
            .iter()
            .map(|(name, ty)| parquet_value(ty, name))
            .collect::<Vec<_>>();
        quote! {
            #module::#payload {
                #(
                    #event_field_names: #values,
                )*
            }
        }
    };
    let values = fields.iter().enumerate().map(|(index, field)| {
        if index == active {
            quote! { #field: Some(#payload_value) }
        } else {
            quote! { #field: None }
        }
    });

    quote! {
        #pattern => #module::#row {
            id,
            timestamp,
            event: #event_name.to_string(),
            #(#values,)*
        }
    }
}

fn parquet_type(ty: &DataType) -> TokenStream {
    match ty {
        DataType::Option(inner) => {
            let inner = parquet_type(inner);
            quote! { Option<#inner> }
        }
        DataType::List(inner) => {
            let inner = parquet_type(inner);
            quote! { Vec<#inner> }
        }
        DataType::Record(name) => {
            let name = raw_ident(to_case(name, Case::Pascal));
            quote! {
                <#name as ::quent_instrumentation::parquet::ParquetValue>::Value
            }
        }
        DataType::DynamicRecord => {
            let ty = map_data_type(ty, 0);
            quote! {
                <#ty as ::quent_instrumentation::parquet::ParquetValue>::Value
            }
        }
        DataType::EntityRef { data: None, .. } => {
            quote! { ::quent_instrumentation::Uuid }
        }
        DataType::EntityRef {
            data: Some(inner), ..
        } => {
            let inner = parquet_type(inner);
            quote! { ::quent_instrumentation::EntityRefParquet<#inner> }
        }
        _ => map_data_type(ty, 0),
    }
}

fn parquet_value(ty: &DataType, value: &Ident) -> TokenStream {
    match ty {
        DataType::Option(inner) => {
            if direct_parquet_value(inner) {
                return quote! {
                    #value.map(::quent_instrumentation::parquet::ParquetValue::into_parquet)
                };
            }
            let inner_value = raw_ident("__quent_value".to_string());
            let inner = parquet_value(inner, &inner_value);
            quote! { #value.map(|#inner_value| #inner) }
        }
        DataType::List(inner) => {
            if direct_parquet_value(inner) {
                return quote! {
                    #value
                        .into_iter()
                        .map(::quent_instrumentation::parquet::ParquetValue::into_parquet)
                        .collect()
                };
            }
            let item = raw_ident("item".to_string());
            let inner = parquet_value(inner, &item);
            quote! { #value.into_iter().map(|#item| #inner).collect() }
        }
        DataType::EntityRef { data: None, .. } => {
            quote! { #value.target }
        }
        DataType::EntityRef {
            data: Some(inner), ..
        } => {
            let data = raw_ident("data".to_string());
            let inner = parquet_value(inner, &data);
            quote! {
                {
                    let ::quent_instrumentation::EntityRef { target, data } = #value;
                    ::quent_instrumentation::EntityRefParquet {
                        target,
                        data: #inner,
                    }
                }
            }
        }
        _ => quote! {
            ::quent_instrumentation::parquet::ParquetValue::into_parquet(#value)
        },
    }
}

fn direct_parquet_value(ty: &DataType) -> bool {
    !matches!(
        ty,
        DataType::Option(_) | DataType::List(_) | DataType::EntityRef { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::pretty;
    use quent_schema::DataType;
    use quent_schema::test_utils::{entity, event, field, ident, record, schema};

    #[test]
    fn disabled_generation_is_empty() {
        let s = schema("M", [entity("E", [event("ev", [])])], []);
        assert!(generate(&s, &Options::default()).is_empty());
    }

    #[test]
    fn generates_record_and_event_companions() {
        let s = schema(
            "M",
            [entity(
                "Connection",
                [
                    event(
                        "opened",
                        [
                            field("endpoint", DataType::Record(ident("Endpoint"))),
                            field(
                                "peer",
                                DataType::EntityRef {
                                    data: Some(Box::new(DataType::U64)),
                                    annotations: Default::default(),
                                },
                            ),
                        ],
                    ),
                    event("closed", []),
                ],
            )],
            [record("Endpoint", [field("host", DataType::String)])],
        );
        let opts = Options {
            parquet: true,
            ..Default::default()
        };
        let src = pretty(generate(&s, &opts));

        assert!(src.contains("pub struct EndpointParquet"));
        assert!(src.contains("impl ::quent_instrumentation::parquet::ParquetValue for Endpoint"));
        assert!(src.contains("pub struct ConnectionEventParquetRow"));
        assert!(src.contains("pub opened: Option<OpenedParquet>"));
        assert!(src.contains("pub closed: Option<bool>"));
        assert!(
            src.contains("impl ::quent_instrumentation::parquet::ParquetEvent for ConnectionEvent")
        );
    }
}
