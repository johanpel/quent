// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of schema-wide model event enums and metadata.

use convert_case::Case;
use proc_macro2::TokenStream;
use quent_schema::Schema;
use quote::quote;
use syn::Ident;

use crate::common::{
    derive_attr, module_ident, path_name_pascal, raw_ident, relative_type_path, to_case,
};
use crate::namespace::Namespace;
use crate::{GenerateError, Options};

pub(crate) fn generate(
    schema: &Schema,
    namespace: &Namespace<'_>,
    opts: &Options,
) -> Result<TokenStream, GenerateError> {
    validate_variants(namespace)?;

    let event = event_ident(schema, namespace);
    let docs = if namespace.path().is_empty() {
        format!("Events emitted by the `{}` model.", schema.name())
    } else {
        let path = namespace
            .path()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("::");
        format!("Events emitted by entities in the `{path}` namespace.")
    };
    let derives = derive_attr(opts.event_derives, opts.debug, opts.serde, opts.serde)?;

    let entity_variants = namespace.entities().iter().map(|entity| {
        let variant = raw_ident(path_name_pascal(entity.path()));
        let event = raw_ident(format!("{}Event", path_name_pascal(entity.path())));
        quote! { #variant(#event) }
    });
    let child_variants = namespace.children_with_entities().map(|child| {
        let segment = child
            .path()
            .last()
            .expect("child namespaces extend their parent");
        let variant = raw_ident(to_case(segment, Case::Pascal));
        let module = module_ident(segment);
        let child_event = event_ident(schema, child);
        quote! { #variant(#module::#child_event) }
    });

    let entity_conversions = namespace.entities().iter().map(|entity| {
        let variant = raw_ident(path_name_pascal(entity.path()));
        let source = raw_ident(format!("{}Event", path_name_pascal(entity.path())));
        quote! {
            impl ::core::convert::From<#source> for #event {
                fn from(event: #source) -> Self {
                    Self::#variant(event)
                }
            }
        }
    });
    let mut child_conversions = Vec::new();
    for child in namespace.children_with_entities() {
        let segment = child
            .path()
            .last()
            .expect("child namespaces extend their parent");
        let variant = raw_ident(to_case(segment, Case::Pascal));
        let module = module_ident(segment);
        let child_event = event_ident(schema, child);
        let child_event_path = quote! { #module::#child_event };

        child_conversions.push(quote! {
            impl ::core::convert::From<#child_event_path> for #event {
                fn from(event: #child_event_path) -> Self {
                    Self::#variant(event)
                }
            }
        });
        for entity in child.all_entities() {
            let source = relative_type_path(entity.path(), namespace.path(), "Event");
            child_conversions.push(quote! {
                impl ::core::convert::From<#source> for #event {
                    fn from(event: #source) -> Self {
                        Self::#variant(#child_event_path::from(event))
                    }
                }
            });
        }
    }

    let model = if namespace.path().is_empty() {
        let model = raw_ident(to_case(schema.name(), Case::Pascal));
        let model_name = schema.name().to_string();
        let model_docs = format!("The `{model_name}` model.");
        let runtime = opts.event_runtime();
        let analyzer_package = match opts.analyzer_package {
            Some(package) => quote! {
                fn analyzer_package() -> ::core::option::Option<&'static str> {
                    ::core::option::Option::Some(#package)
                }
            },
            None => quote! {},
        };
        quote! {
            #[doc = #model_docs]
            pub struct #model;

            impl #runtime::build_info::ModelSource for #model {
                fn package() -> &'static str {
                    env!("CARGO_PKG_NAME")
                }

                fn source() -> #runtime::build_info::BuildInfo {
                    #runtime::build_info::source_or_quent(
                        env!("CARGO_PKG_VERSION"),
                        option_env!("QUENT_SOURCE_REMOTE"),
                        option_env!("QUENT_SOURCE_COMMIT"),
                        option_env!("QUENT_SOURCE_BRANCH"),
                        option_env!("QUENT_SOURCE_DIRTY"),
                        option_env!("QUENT_SOURCE_BUILT_AT"),
                    )
                }

                #analyzer_package
            }

            impl #runtime::Model for #model {
                const NAME: &'static str = #model_name;
                type Event = #event;
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #[doc = #docs]
        #derives
        pub enum #event {
            #(#entity_variants,)*
            #(#child_variants,)*
        }

        #(#entity_conversions)*
        #(#child_conversions)*
        #model
    })
}

fn event_ident(schema: &Schema, namespace: &Namespace<'_>) -> Ident {
    let name = namespace.path().last().unwrap_or_else(|| schema.name());
    raw_ident(format!("{}Event", to_case(name, Case::Pascal)))
}

fn validate_variants(namespace: &Namespace<'_>) -> Result<(), GenerateError> {
    let mut variants = std::collections::BTreeSet::new();
    for variant in namespace
        .entities()
        .iter()
        .map(|entity| path_name_pascal(entity.path()))
        .chain(namespace.children_with_entities().map(|child| {
            to_case(
                child
                    .path()
                    .last()
                    .expect("child namespaces extend their parent"),
                Case::Pascal,
            )
        }))
    {
        if !variants.insert(variant.clone()) {
            let path = namespace
                .path()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("::");
            return Err(GenerateError::ModelEventVariantCollision {
                namespace: path,
                variant,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::pretty;
    use quent_schema::builder::SchemaBuilder;
    use quent_schema::test_utils::{entity, event};

    #[test]
    fn generates_namespace_events_and_transitive_conversions() {
        let schema = SchemaBuilder::try_new("Demo")
            .unwrap()
            .with_entity(entity("Root", [event("created", [])]))
            .with_entity(entity("Foo::Query", [event("created", [])]))
            .with_entity(entity("Foo::Nested::Task", [event("created", [])]))
            .build()
            .unwrap();
        let namespaces = Namespace::root(&schema);

        let root = pretty(generate(&schema, &namespaces, &Options::default()).unwrap());
        let foo =
            pretty(generate(&schema, &namespaces.children()[0], &Options::default()).unwrap());

        assert!(root.contains("pub enum DemoEvent"));
        assert!(root.contains("Foo(foo::FooEvent)"));
        assert!(root.contains("impl ::core::convert::From<foo::QueryEvent> for DemoEvent"));
        assert!(root.contains("impl ::core::convert::From<foo::nested::TaskEvent> for DemoEvent"));
        assert!(foo.contains("pub enum FooEvent"));
        assert!(foo.contains("Query(QueryEvent)"));
        assert!(foo.contains("Nested(nested::NestedEvent)"));
    }

    #[test]
    fn rejects_entity_and_namespace_variant_collisions() {
        let schema = SchemaBuilder::try_new("Demo")
            .unwrap()
            .with_entity(entity("Foo", [event("created", [])]))
            .with_entity(entity("Foo::Query", [event("created", [])]))
            .build()
            .unwrap();
        let namespaces = Namespace::root(&schema);

        assert!(matches!(
            generate(&schema, &namespaces, &Options::default()),
            Err(GenerateError::ModelEventVariantCollision { variant, .. }) if variant == "Foo"
        ));
    }
}
