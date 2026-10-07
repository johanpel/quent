// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of entity-specific event access.

use std::collections::BTreeMap;

use quent_instrumentation_build::{
    Options, generated_entity_event_path, generated_entity_path, generated_field_type,
    generated_module_ident, generated_type_ident,
};
use quent_schema::{Cardinality, Identifier, Schema};
use quote::quote;

use crate::GenerateError;

#[derive(Default)]
struct Module {
    children: BTreeMap<String, (syn::Ident, String, Module)>,
    fields: BTreeMap<String, String>,
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
        let mut namespace = vec![root.clone()];
        let mut module = &mut tree;
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
        module
            .fields
            .insert(access.to_string(), "entity access trait".into());
        module
            .fields
            .insert(native.to_string(), "native entity storage".into());
        let mut payloads = Vec::new();
        let mut storage = Vec::new();
        let mut defaults = Vec::new();
        let mut arms = Vec::new();
        let mut methods = Vec::new();
        let mut implementations = Vec::new();
        let mut method_names = BTreeMap::from([
            ("id".to_owned(), "entity identity".to_owned()),
            ("type_name".to_owned(), "entity type name".to_owned()),
        ]);
        for event in entity.events() {
            let variant = generated_type_ident(event.name());
            let payload =
                quote::format_ident!("{}Payload", variant.to_string().trim_start_matches("r#"));
            let method = generated_module_ident(event.name());
            let origin = format!("event {}::{}", entity.path(), event.name());
            for (names, name) in [
                (&mut module.fields, payload.to_string()),
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
            let slot =
                quote::format_ident!("event_{}", method.to_string().trim_start_matches("r#"));
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
                let ty: syn::Type = syn::parse2(generated_field_type(field, &namespace, opts)?)
                    .map_err(GenerateError::InvalidGeneratedCode)?;
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
            payloads.push(quote! {
                #[doc = #docs]
                #debug
                #[derive(::serde::Serialize, ::serde::Deserialize)]
                pub struct #payload { #(#fields,)* }
            });
            let pattern = if field_patterns.is_empty() {
                quote! { #original::#variant }
            } else {
                quote! { #original::#variant { #(#field_patterns,)* } }
            };
            let value = quote! { ::quent_events::Event::new(event_id, event_timestamp, #payload { #(#field_values,)* }) };
            let once = event.cardinality() == Cardinality::Once;
            if once {
                let event_name = event.name().to_string();
                storage.push(
                    quote! { #slot: ::core::option::Option<::quent_events::Event<#payload>> },
                );
                defaults.push(quote! { #slot: ::core::option::Option::None });
                arms.push(quote! { #pattern => {
                    ::quent_store::entity::insert_once(&mut result.#slot, #value)
                        .map_err(|event| ::quent_store::entity::DuplicateOnceEvent {
                            entity_id: event.id,
                            event_name: #event_name,
                        })?;
                } });
                methods.push(quote! {
                    /// Borrows the recorded event, returning `None` when absent.
                    fn #method(&self) -> ::core::option::Option<&::quent_events::Event<#payload>>;
                });
                implementations.push(quote! { fn #method(&self) -> ::core::option::Option<&::quent_events::Event<#payload>> { self.#slot.as_ref() } });
            } else {
                storage.push(quote! { #slot: ::std::vec::Vec<::quent_events::Event<#payload>> });
                defaults.push(quote! { #slot: ::std::vec::Vec::new() });
                arms.push(quote! { #pattern => { result.#slot.push(#value); } });
                methods.push(quote! {
                    /// Borrows events in timestamp order; equal timestamps retain input order.
                    fn #method(&self) -> impl ::core::iter::Iterator<Item = &::quent_events::Event<#payload>>;
                });
                implementations.push(quote! { fn #method(&self) -> impl ::core::iter::Iterator<Item = &::quent_events::Event<#payload>> { self.#slot.iter() } });
            }
        }
        let docs = format!("Provides event-type-scoped access for `{}`.", entity.path());
        let native_docs = format!("Owns events grouped by event type for `{}`.", entity.path());
        let items: syn::File = syn::parse2(quote! {
            #(#payloads)*
            #[doc = #docs]
            pub trait #access: ::quent_store::entity::EntityHandle<Entity = #marker> {
                #(#methods)*
            }
            #[doc = #native_docs]
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
        }).map_err(GenerateError::InvalidGeneratedCode)?;
        module.items.extend(items.items);
    }
    tree.into_item(syn::parse_quote!(entity_events))
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
