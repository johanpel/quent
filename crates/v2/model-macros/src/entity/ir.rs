// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use proc_macro2::TokenStream;
use quent_v2_model_ir::{
    entity::Entity,
    fsm::{Fsm, State},
};
use quote::quote;
use syn::{DeriveInput, Token, Variant, punctuated::Punctuated};

use crate::docs::{emit_docs_tokens, extract_docs};
use crate::entity::parse::conventions::ConventionArm;

/// Expand a struct into a single-event entity.
pub fn expand_struct(
    name: &syn::Ident,
    fields: &syn::Fields,
    input: &DeriveInput,
    entity: &Entity,
    convention_arms: &[ConventionArm],
) -> syn::Result<TokenStream> {
    if matches!(fields, syn::Fields::Unnamed(_)) {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Entity)] on a struct requires a unit struct or a struct with named fields",
        ));
    }

    let name_string = name.to_string();

    let has_payload = match fields {
        syn::Fields::Unit => false,
        syn::Fields::Named(n) => !n.named.is_empty(),
        syn::Fields::Unnamed(_) => unreachable!(),
    };

    let payload = if has_payload {
        quote! {
            ::std::vec![
                ::quent_v2_model_ir::event::EventField {
                    name: ::quent_v2_model_ir::identifier::Identifier::new_unchecked("payload"),
                    docs: ::std::option::Option::None,
                    ty: ::quent_v2_model_ir::event::EventFieldType::Payload(
                        ::quent_v2_model_ir::data_type::DataType::Record(
                            ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string)
                        ),
                    ),
                    conventions: ::std::vec::Vec::new(),
                }
            ]
        }
    } else {
        quote! { ::std::vec::Vec::new() }
    };

    let fsm = emit_fsm_opt(entity);
    let conventions = emit_conventions(convention_arms);
    let docs_tokens = emit_docs_tokens(extract_docs(&input.attrs).as_deref());

    let entity_impl = quote! {
        impl ::quent_v2_model::entity::Entity for #name {
            fn ir() -> ::quent_v2_model_ir::entity::Entity {
                ::quent_v2_model_ir::entity::Entity {
                    name: ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string),
                    docs: #docs_tokens,
                    events: ::std::vec![
                        ::quent_v2_model_ir::event::Event {
                            name: ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string),
                            docs: #docs_tokens,
                            cardinality: ::quent_v2_model_ir::event::Cardinality::Once,
                            payload: #payload,
                            conventions: ::std::vec::Vec::new(),
                        }
                    ],
                    fsm: #fsm,
                    conventions: #conventions,
                }
            }
            fn ir_ref_target() -> ::quent_v2_model_ir::event::EntityRefTarget {
                ::quent_v2_model_ir::event::EntityRefTarget::Specific(
                    ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string)
                )
            }
        }
    };

    Ok(quote! {
        #entity_impl
    })
}

pub fn expand_enum(
    name: &syn::Ident,
    variants: &Punctuated<Variant, Token![,]>,
    input: &DeriveInput,
    entity: &Entity,
    convention_arms: &[ConventionArm],
) -> syn::Result<TokenStream> {
    if variants.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Entity)] requires the enum to have at least one variant, otherwise it would have no event",
        ));
    }

    let name_string = name.to_string();

    let events: Vec<TokenStream> = variants
        .iter()
        .map(expand_variant)
        .collect::<syn::Result<_>>()?;

    let fsm = emit_fsm_opt(entity);
    let conventions = emit_conventions(convention_arms);
    let docs_tokens = emit_docs_tokens(extract_docs(&input.attrs).as_deref());

    let entity_impl = quote! {
        impl ::quent_v2_model::entity::Entity for #name {
            fn ir() -> ::quent_v2_model_ir::entity::Entity {
                ::quent_v2_model_ir::entity::Entity {
                    name: ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string),
                    docs: #docs_tokens,
                    events: ::std::vec![ #(#events),* ],
                    fsm: #fsm,
                    conventions: #conventions,
                }
            }
            fn ir_ref_target() -> ::quent_v2_model_ir::event::EntityRefTarget {
                ::quent_v2_model_ir::event::EntityRefTarget::Specific(
                    ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string)
                )
            }
        }
    };

    Ok(quote! {
        #entity_impl
    })
}

fn expand_variant(v: &Variant) -> syn::Result<TokenStream> {
    let variant_name = v.ident.to_string();
    let cardinality = parse_cardinality(&v.attrs)?;
    let payload = expand_variant_event_fields(&v.fields, v)?;
    let docs_tokens = emit_docs_tokens(extract_docs(&v.attrs).as_deref());

    Ok(quote! {
        ::quent_v2_model_ir::event::Event {
            name: ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#variant_name),
            docs: #docs_tokens,
            cardinality: #cardinality,
            payload: #payload,
            conventions: ::std::vec::Vec::new(),
        }
    })
}

fn parse_cardinality(attrs: &[syn::Attribute]) -> syn::Result<TokenStream> {
    let mut is_multi = false;
    for attr in attrs {
        if !attr.path().is_ident("quent") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("multi") {
                is_multi = true;
                Ok(())
            } else {
                Err(meta.error("unknown #[quent(...)] argument"))
            }
        })?;
    }
    Ok(if is_multi {
        quote! { ::quent_v2_model_ir::event::Cardinality::Multi }
    } else {
        quote! { ::quent_v2_model_ir::event::Cardinality::Once }
    })
}

fn expand_variant_event_fields(
    fields: &syn::Fields,
    span_source: &Variant,
) -> syn::Result<TokenStream> {
    match fields {
        syn::Fields::Unit => Ok(quote! { ::std::vec::Vec::new() }),
        // There can only be one unnamed field as we can't allow Rust's
        // desugared field names as they don't start with an alpha.
        syn::Fields::Unnamed(u) if u.unnamed.len() == 1 => {
            let ty = &u.unnamed.first().unwrap().ty;
            Ok(quote! {
                ::std::vec![
                    {
                        let __ty = ::quent_v2_model_ir::event::EventFieldType::Payload(
                            <#ty as ::quent_v2_model::data_type::DataType>::ir(),
                        );
                        let __name = match &__ty {
                            ::quent_v2_model_ir::event::EventFieldType::Payload(_) => "payload",
                            ::quent_v2_model_ir::event::EventFieldType::EntityRef { .. } => "entity",
                        };
                        ::quent_v2_model_ir::event::EventField {
                            name: ::quent_v2_model_ir::identifier::Identifier::new_unchecked(__name),
                            docs: ::std::option::Option::None,
                            ty: __ty,
                            conventions: ::std::vec::Vec::new(),
                        }
                    }
                ]
            })
        }
        syn::Fields::Unnamed(_) => Err(syn::Error::new_spanned(
            span_source,
            "#[derive(Entity)] does not support enum variants with more than one unnamed field",
        )),
        syn::Fields::Named(named) => {
            // The model crate has a blanket impl of EventField for
            // application-specific data types, which implement DataType.
            let field_defs = named.named.iter().map(|f| {
                let field_name = f.ident.as_ref().unwrap().to_string();
                let ty = &f.ty;
                let docs_tokens = emit_docs_tokens(extract_docs(&f.attrs).as_deref());
                quote! {
                    ::quent_v2_model_ir::event::EventField {
                        name: ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#field_name),
                        docs: #docs_tokens,
                        ty: <#ty as ::quent_v2_model::event::EventField>::ir(),
                        conventions: ::std::vec::Vec::new(),
                    }
                }
            });
            Ok(quote! {
                ::std::vec![ #(#field_defs),* ]
            })
        }
    }
}

fn emit_fsm_opt(entity: &Entity) -> TokenStream {
    match &entity.fsm {
        Some(fsm) => {
            let fsm_tokens = emit_fsm(fsm);
            quote! { ::std::option::Option::Some(#fsm_tokens) }
        }
        None => quote! { ::std::option::Option::None },
    }
}

fn emit_conventions(arms: &[ConventionArm]) -> TokenStream {
    let entries = arms.iter().map(|a| {
        let name = &a.name;
        let data = &a.data;
        quote! {
            ::quent_v2_model_ir::convention::Convention {
                name: ::std::string::String::from(#name),
                validated: true,
                data: ::std::option::Option::Some(#data),
            }
        }
    });
    quote! {
        ::std::vec![ #(#entries),* ]
    }
}

fn emit_fsm(fsm: &Fsm) -> TokenStream {
    let transitions = fsm.transitions.iter().map(|t| {
        let source = emit_state(&t.source);
        let target = emit_state(&t.target);
        quote! {
            ::quent_v2_model_ir::fsm::Transition {
                source: #source,
                target: #target,
            }
        }
    });
    quote! {
        ::quent_v2_model_ir::fsm::Fsm {
            transitions: ::std::vec![ #(#transitions),* ],
        }
    }
}

fn emit_state(s: &State) -> TokenStream {
    match s {
        State::Entry => quote! { ::quent_v2_model_ir::fsm::State::Entry },
        State::Exit => quote! { ::quent_v2_model_ir::fsm::State::Exit },
        State::State(id) => {
            let name = id.as_str();
            quote! {
                ::quent_v2_model_ir::fsm::State::State(
                    ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name)
                )
            }
        }
    }
}
