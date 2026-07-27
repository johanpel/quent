// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of the live instrumentation surface for a schema.

use convert_case::Case;
use proc_macro2::TokenStream;
use quent_schema::{Entity, Schema};
use quote::quote;
use syn::Ident;

use crate::GenerateError;
use crate::common::{raw_ident, to_case};

mod context;
mod handle;

pub(crate) use handle::MAX_ONCE_EVENTS;

/// Generates the entity runtime types and model integration.
pub(crate) fn generate_runtime_types(schema: &Schema) -> Result<TokenStream, GenerateError> {
    let entity_types = entity_types(schema);
    let entities = schema
        .entities()
        .map(|entity| entity_runtime_types(schema, entity))
        .collect::<Result<Vec<_>, _>>()?;
    let observer_storage = context::observer_storage(schema)?;
    let model = context::schema_model(schema);

    Ok(quote! {
        #entity_types
        #(#entities)*
        #observer_storage
        #model
    })
}

fn entity_runtime_types(schema: &Schema, entity: &Entity) -> Result<TokenStream, GenerateError> {
    let marker = entity_marker(entity);
    let event_impl = entity_event_impl(entity);
    let handle = handle::entity_handle(entity)?;
    let entity_impl = entity_impl(schema, entity);
    Ok(quote! {
        #marker
        #event_impl
        #handle
        #entity_impl
    })
}

fn entity_types(schema: &Schema) -> TokenStream {
    let model = model_ident(schema);
    let model_name = schema.name().to_string();
    let handle_docs =
        format!("Handle to one entity instance in the `{model_name}` instrumentation model.");
    quote! {
        #[doc = #handle_docs]
        pub struct Handle<E: ::quent_instrumentation::Entity<Context = Context<#model>>> {
            inner: ::quent_instrumentation::HandleInner<E>,
        }

        impl<E: ::quent_instrumentation::Entity<Context = Context<#model>>>
            ::core::convert::From<::quent_instrumentation::HandleInner<E>> for Handle<E>
        {
            fn from(inner: ::quent_instrumentation::HandleInner<E>) -> Self {
                Self { inner }
            }
        }

        impl<E: ::quent_instrumentation::Entity<Context = Context<#model>>> ::core::ops::Deref
            for Handle<E>
        {
            type Target = ::quent_instrumentation::HandleInner<E>;

            fn deref(&self) -> &Self::Target {
                &self.inner
            }
        }
    }
}

/// Re-exports shared runtime types used by the generated API.
pub(crate) fn reexports() -> TokenStream {
    quote! {
        pub use ::quent_instrumentation::{
            AnyEntity, Context, DynamicAttributes, EntityRef, Event, HandleError, Observer, Uuid,
        };
    }
}

fn entity_marker(entity: &Entity) -> TokenStream {
    let marker = marker_ident(entity);
    let doc = format!("Marker type for the `{}` entity.", entity.path());
    quote! {
        #[doc = #doc]
        #[derive(Debug, Clone, Copy)]
        pub struct #marker;
    }
}

fn entity_event_impl(entity: &Entity) -> TokenStream {
    let event_ty = event_ident(entity);
    let stream_name = to_case(entity.path().name(), Case::Snake);
    quote! {
        impl ::quent_instrumentation::EntityEvent for #event_ty {
            const NAME: &'static str = #stream_name;
        }
    }
}

fn event_ident(entity: &Entity) -> Ident {
    raw_ident(format!(
        "{}Event",
        to_case(entity.path().name(), Case::Pascal)
    ))
}

fn marker_ident(entity: &Entity) -> Ident {
    raw_ident(to_case(entity.path().name(), Case::Pascal))
}

fn model_ident(schema: &Schema) -> Ident {
    raw_ident(to_case(schema.name(), Case::Pascal))
}

fn entity_impl(schema: &Schema, entity: &Entity) -> TokenStream {
    let marker = marker_ident(entity);
    let event = event_ident(entity);
    let model = model_ident(schema);
    quote! {
        impl ::quent_instrumentation::Entity for #marker {
            type Event = #event;
            type Context = Context<#model>;
            type Handle = Handle<Self>;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::pretty;
    use quent_schema::Cardinality;
    use quent_schema::DataType;
    use quent_schema::builder::{EntityBuilder, EventBuilder, SchemaBuilder};
    use quent_schema::test_utils::{field, ident};

    #[test]
    fn generates_generic_instrumentation_api() {
        let connection = EntityBuilder::new(ident("Connection"))
            .with_event(
                EventBuilder::new(ident("data"), Cardinality::Multi)
                    .with_field(field("bytes", DataType::U64))
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let schema = SchemaBuilder::new(ident("Demo"))
            .with_entity(connection)
            .build()
            .unwrap();

        let source = pretty(generate_runtime_types(&schema).unwrap());

        assert!(source.contains("pub struct Handle<E:"));
        assert!(source.contains("impl Handle<Connection>"));
        assert!(source.contains("pub struct DemoObservers"));
        assert!(source.contains("connection_observer:"));
        assert!(source.contains("pub struct Demo"));
        assert!(source.contains("impl ::quent_instrumentation::Entity for Connection"));
        assert!(source.contains("type Context = Context<Demo>"));
        assert!(source.contains(r#"const NAME: &'static str = "connection""#));
    }
}
