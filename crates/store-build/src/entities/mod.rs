// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of entity-specific event access.

use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quent_instrumentation_build::{
    Options, generated_entity_event_path, generated_entity_path, generated_field_type,
    generated_module_ident, generated_type_ident,
};
use quent_schema::{Cardinality, Entity, Event, Identifier, Schema};
use quote::quote;

use crate::GenerateError;

mod storage;

use storage::EventStorageCode;

/// An output module containing generated Rust items and nested modules.
#[derive(Default)]
struct Module {
    /// Child modules keyed by Rust name, with their identifier and schema
    /// origin.
    children: BTreeMap<String, (syn::Ident, String, Module)>,
    /// Generated type names mapped to their origins for conflict checking.
    fields: BTreeMap<String, String>,
    /// Rust items emitted directly in this module.
    ///
    /// For example, `pub struct InitPayload { pub name: String }`.
    items: Vec<syn::Item>,
}

impl Module {
    fn child(&mut self, ident: syn::Ident, origin: String) -> Result<&mut Self, GenerateError> {
        let name = ident.to_string();
        let (_, existing, child) = self
            .children
            .entry(name.clone())
            .or_insert_with(|| (ident, origin.clone(), Self::default()));
        if *existing != origin {
            return Err(GenerateError::EntityEventsNameConflict {
                name,
                first: existing.clone(),
                second: origin,
            });
        }
        Ok(child)
    }

    fn into_item(self, ident: syn::Ident) -> Result<syn::ItemMod, GenerateError> {
        let mut items = self.items;
        for (ident, _, child) in self.children.into_values() {
            items.push(syn::Item::Mod(child.into_item(ident)?));
        }
        syn::parse2(quote! { pub mod #ident { #(#items)* } })
            .map_err(GenerateError::InvalidGeneratedCode)
    }
}

/// Rust fragments declaring an event-specific payload and converting an enum
/// variant into it.
struct PayloadCode {
    /// The payload struct definition, including derives and documentation.
    ///
    /// For example, `pub struct InitPayload { pub name: String }`.
    payload_struct_def: TokenStream,
    /// An enum variant pattern binding its payload fields by value.
    ///
    /// For example, `TaskEvent::Init { name: field_0 }`.
    variant_pattern: TokenStream,
    /// An expression constructing a typed event from the bound fields and
    /// recorded metadata.
    ///
    /// For example,
    /// `Event::new(id, ts, InitPayload { name: field_0 })`.
    event_constructor_expr: TokenStream,
}

/// Backend-independent payload and accessor declarations for one schema event.
struct EventCode {
    /// The event-specific payload struct definition.
    ///
    /// For example, `pub struct InitPayload { pub name: String }`.
    payload_struct_def: TokenStream,
    /// An accessor declaration for the entity-specific trait.
    ///
    /// For example, `fn init(&self) -> Option<&Event<InitPayload>>;`.
    accessor_method_decl: TokenStream,
    /// Storage fragments grouped by backend.
    storage: EventStorageCode,
}

pub(super) fn generate(
    schema: &Schema,
    opts: &Options,
    events: &syn::File,
) -> Result<syn::ItemMod, GenerateError> {
    if events
        .items
        .iter()
        .any(|item| matches!(item, syn::Item::Mod(module) if module.ident == "entity_events"))
    {
        return Err(GenerateError::EntityEventsNameConflict {
            name: "entity_events".into(),
            first: "schema namespace".into(),
            second: "entity event access".into(),
        });
    }
    let root = Identifier::try_new("entity_events").expect("valid identifier");
    let mut tree = Module::default();
    for entity in schema.entities() {
        let (module, namespace) = entity_module(&mut tree, entity, &root)?;
        let items = generate_entity(entity, &namespace, opts, &mut module.fields)?;
        module.items.extend(items.items);
    }
    tree.into_item(syn::parse_quote!(entity_events))
}

fn entity_module<'a>(
    tree: &'a mut Module,
    entity: &Entity,
    root: &Identifier,
) -> Result<(&'a mut Module, Vec<Identifier>), GenerateError> {
    let mut namespace = vec![root.clone()];
    let mut module = tree;
    for segment in entity.path().namespace() {
        namespace.push(segment.clone());
        let path = namespace[1..]
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("::");
        module = module.child(generated_module_ident(segment), format!("namespace {path}"))?;
    }
    namespace.push(entity.path().name().clone());
    module = module.child(
        generated_module_ident(entity.path().name()),
        format!("entity {}", entity.path()),
    )?;
    Ok((module, namespace))
}

fn generate_entity(
    entity: &Entity,
    namespace: &[Identifier],
    opts: &Options,
    type_names: &mut BTreeMap<String, String>,
) -> Result<syn::File, GenerateError> {
    let parents = vec![quote!(super); namespace.len()];
    let marker = generated_entity_path(entity);
    let original = generated_entity_event_path(entity);
    let marker = quote! { #(#parents::)* #marker };
    let original = quote! { #(#parents::)* #original };
    let entity_name = generated_type_ident(entity.path().name())
        .to_string()
        .trim_start_matches("r#")
        .to_owned();
    let access = quote::format_ident!("{}Events", entity_name);
    let native = quote::format_ident!("Native{}Events", entity_name);
    type_names.insert(access.to_string(), "entity access trait".into());
    type_names.insert(native.to_string(), "native entity storage".into());
    let mut method_names = BTreeMap::from([
        ("id".to_owned(), "entity identity".to_owned()),
        ("type_name".to_owned(), "entity type name".to_owned()),
    ]);
    let mut code = Vec::new();
    for event in entity.events() {
        let payload = payload_ident(event);
        let method = generated_module_ident(event.name());
        let origin = format!("event {}::{}", entity.path(), event.name());
        for (names, name) in [
            (&mut *type_names, payload.to_string()),
            (&mut method_names, method.to_string()),
        ] {
            if let Some(first) = names.insert(name.clone(), origin.clone()) {
                return Err(GenerateError::EntityEventsNameConflict {
                    name,
                    first,
                    second: origin.clone(),
                });
            }
        }
        code.push(generate_event(entity, event, namespace, opts, &original)?);
    }
    let payloads = code.iter().map(|event| &event.payload_struct_def);
    let methods = code.iter().map(|event| &event.accessor_method_decl);
    let native_storage = storage::generate_entity(entity, &native, &access, &marker, &code);
    let docs = format!("Provides event-type-scoped access for `{}`.", entity.path());
    syn::parse2(quote! {
        #(#payloads)*
        #[doc = #docs]
        pub trait #access: ::quent_store::entity::EntityHandle<Entity = #marker> {
            #(#methods)*
        }
        #native_storage
    })
    .map_err(GenerateError::InvalidGeneratedCode)
}

fn payload_ident(event: &Event) -> syn::Ident {
    let variant = generated_type_ident(event.name());
    quote::format_ident!("{}Payload", variant.to_string().trim_start_matches("r#"))
}

fn generate_payload(
    entity: &Entity,
    event: &Event,
    namespace: &[Identifier],
    opts: &Options,
    original: &TokenStream,
) -> Result<PayloadCode, GenerateError> {
    let variant = generated_type_ident(event.name());
    let payload = payload_ident(event);
    let mut names = BTreeMap::new();
    let mut fields = Vec::new();
    let mut field_patterns = Vec::new();
    let mut field_values = Vec::new();
    for (index, field) in event.fields().enumerate() {
        let ident = generated_module_ident(field.name());
        if let Some(first) = names.insert(ident.to_string(), field.name().to_string()) {
            return Err(GenerateError::EntityEventsNameConflict {
                name: ident.to_string(),
                first,
                second: field.name().to_string(),
            });
        }
        let ty = generated_field_type(field, namespace, opts)?;
        fields.push(quote! { pub #ident: #ty });
        let binding = quote::format_ident!("field_{}", index);
        field_patterns.push(quote! { #ident: #binding });
        field_values.push(quote! { #ident: #binding });
    }
    let debug = opts.debug.then(|| quote! { #[derive(Debug)] });
    let docs = format!(
        "Payload of the `{}` event for `{}`.",
        event.name(),
        entity.path()
    );
    let payload_struct_def = quote! {
        #[doc = #docs]
        #debug
        #[derive(::serde::Serialize, ::serde::Deserialize)]
        pub struct #payload { #(#fields,)* }
    };
    let variant_pattern = if field_patterns.is_empty() {
        quote! { #original::#variant }
    } else {
        quote! { #original::#variant { #(#field_patterns,)* } }
    };
    let event_constructor_expr = quote! { ::quent_events::Event::new(event_id, event_timestamp, #payload { #(#field_values,)* }) };
    Ok(PayloadCode {
        payload_struct_def,
        variant_pattern,
        event_constructor_expr,
    })
}

fn generate_event(
    entity: &Entity,
    event: &Event,
    namespace: &[Identifier],
    opts: &Options,
    original: &TokenStream,
) -> Result<EventCode, GenerateError> {
    let payload = payload_ident(event);
    let method = generated_module_ident(event.name());
    let PayloadCode {
        payload_struct_def,
        variant_pattern,
        event_constructor_expr,
    } = generate_payload(entity, event, namespace, opts, original)?;
    let accessor_method_decl = if event.cardinality() == Cardinality::Once {
        quote! {
            /// Borrows the recorded event, returning `None` when absent.
            fn #method(&self) -> ::core::option::Option<&::quent_events::Event<#payload>>;
        }
    } else {
        quote! {
            /// Borrows events in timestamp order; equal timestamps retain input order.
            fn #method(&self) -> impl ::core::iter::Iterator<Item = &::quent_events::Event<#payload>>;
        }
    };
    let storage = storage::generate_event(
        event,
        &payload,
        &method,
        &variant_pattern,
        &event_constructor_expr,
    );
    Ok(EventCode {
        payload_struct_def,
        accessor_method_decl,
        storage,
    })
}

#[cfg(test)]
mod tests {
    use quent_schema::test_utils::{entity, event, event_with, field, schema};
    use quent_schema::{Cardinality, DataType};

    use crate::{GenerateError, Options, generate_str};

    #[test]
    fn generation_is_opt_in_and_independent_of_loading() {
        let schema = schema(
            "Demo",
            [entity(
                "Task",
                [
                    event("created", []),
                    event_with(
                        "updated",
                        Cardinality::Multi,
                        [field("name", DataType::String)],
                    ),
                ],
            )],
            [],
        );
        let options = Options {
            combined_event: false,
            filesystem: false,
            ..Options::default()
        };
        let source = generate_str(&schema, &options).unwrap();
        assert!(!source.contains("pub mod entity_events"));
        let source = generate_str(
            &schema,
            &Options {
                entity_events: true,
                debug: false,
                ..options
            },
        )
        .unwrap();
        assert!(source.contains("pub trait TaskEvents"));
        assert!(source.contains("pub struct NativeTaskEvents"));
        assert!(source.contains("pub struct CreatedPayload"));
        assert!(source.contains("pub struct UpdatedPayload"));
        assert!(!source.contains("filesystem::Model"));
        assert!(!source.contains("pub enum DemoEvent"));
        assert!(!source.contains("derive(Debug)"));
    }

    #[test]
    fn rejects_namespace_and_normalized_name_conflicts() {
        let schemas = [
            schema(
                "Demo",
                [entity("EntityEvents::Task", [event("created", [])])],
                [],
            ),
            schema(
                "Demo",
                [
                    entity("Worker", [event("created", [])]),
                    entity("Worker::Child", [event("created", [])]),
                ],
                [],
            ),
            schema(
                "Demo",
                [
                    entity("Foo::Task", [event("created", [])]),
                    entity("foo::Worker", [event("created", [])]),
                ],
                [],
            ),
            schema(
                "Demo",
                [entity(
                    "Task",
                    [event("first_name", []), event("FirstName", [])],
                )],
                [],
            ),
            schema(
                "Demo",
                [entity(
                    "Task",
                    [event(
                        "created",
                        [
                            field("field_name", DataType::String),
                            field("FieldName", DataType::String),
                        ],
                    )],
                )],
                [],
            ),
            schema("Demo", [entity("Task", [event("id", [])])], []),
        ];
        for schema in schemas {
            assert!(matches!(
                generate_str(
                    &schema,
                    &Options {
                        entity_events: true,
                        filesystem: false,
                        combined_event: false,
                        ..Options::default()
                    }
                ),
                Err(GenerateError::EntityEventsNameConflict { .. })
            ));
        }
    }
}
