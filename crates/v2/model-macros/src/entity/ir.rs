// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use proc_macro2::TokenStream;
use quent_v2_model_ir::{
    entity::Entity,
    qualifications::{
        Qualification,
        fsm::{Fsm, State},
        resource::{Boundedness, CapacityKind, Resource},
    },
};
use quote::quote;
use syn::{DeriveInput, Token, Variant, punctuated::Punctuated};

/// Expand a struct into a single-event entity.
pub fn expand_struct(
    name: &syn::Ident,
    fields: &syn::Fields,
    input: &DeriveInput,
    entity: &Entity,
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
                ::quent_v2_model_ir::event::EventField::from_type(
                    ::quent_v2_model_ir::event::EventFieldType::Payload(
                        ::quent_v2_model_ir::value_type::ValueType::Attributes(
                            ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string)
                        ),
                    ),
                )
            ]
        }
    } else {
        quote! { ::std::vec::Vec::new() }
    };

    let entity_decl = quote! {
        impl ::quent_v2_model::Entity for #name {}
    };
    let entity_ref_target = quote! {
        impl ::quent_v2_model_ir::event::ModelEntityRefTarget for #name {
            fn model_entity_ref_target() -> ::quent_v2_model_ir::event::EntityRefTarget {
                ::quent_v2_model_ir::event::EntityRefTarget::Specific(
                    ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string)
                )
            }
        }
    };

    let qualifications = emit_qualifications(entity);

    let entity_impl = quote! {
        impl ::quent_v2_model_ir::entity::ModelEntity for #name {
            fn model_entity() -> ::quent_v2_model_ir::entity::Entity {
                ::quent_v2_model_ir::entity::Entity::new(
                    ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string),
                    ::std::vec![
                        ::quent_v2_model_ir::event::Event::new(
                            ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string),
                            ::quent_v2_model_ir::event::Cardinality::Once,
                            #payload,
                        )
                    ],
                    #qualifications,
                    ::std::format!(
                        "{}::{}",
                        ::std::module_path!(),
                        #name_string,
                    ),
                )
            }
        }
    };

    Ok(quote! {
        #entity_decl
        #entity_ref_target
        #entity_impl
    })
}

pub fn expand_enum(
    name: &syn::Ident,
    variants: &Punctuated<Variant, Token![,]>,
    input: &DeriveInput,
    entity: &Entity,
) -> syn::Result<TokenStream> {
    if variants.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Entity)] requires the enum to have at least one variant, otherwise it would have no event",
        ));
    }

    let name_string = name.to_string();

    let entity_decl = quote! {
        impl ::quent_v2_model::Entity for #name {}
    };
    let entity_ref_target = quote! {
        impl ::quent_v2_model_ir::event::ModelEntityRefTarget for #name {
            fn model_entity_ref_target() -> ::quent_v2_model_ir::event::EntityRefTarget {
                ::quent_v2_model_ir::event::EntityRefTarget::Specific(
                    ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string)
                )
            }
        }
    };

    let events: Vec<TokenStream> = variants
        .iter()
        .map(expand_variant)
        .collect::<syn::Result<_>>()?;

    let qualifications = emit_qualifications(entity);

    let entity_impl = quote! {
        impl ::quent_v2_model_ir::entity::ModelEntity for #name {
            fn model_entity() -> ::quent_v2_model_ir::entity::Entity {
                ::quent_v2_model_ir::entity::Entity::new(
                    ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string),
                    ::std::vec![ #(#events),* ],
                    #qualifications,
                    ::std::format!(
                        "{}::{}",
                        ::std::module_path!(),
                        #name_string,
                    ),
                )
            }
        }
    };

    Ok(quote! {
        #entity_decl
        #entity_ref_target
        #entity_impl
    })
}

fn expand_variant(v: &Variant) -> syn::Result<TokenStream> {
    let variant_name = v.ident.to_string();
    let cardinality = parse_cardinality(&v.attrs)?;
    let payload = expand_variant_event_fields(&v.fields, v)?;

    Ok(quote! {
        ::quent_v2_model_ir::event::Event::new(
            ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#variant_name),
            #cardinality,
            #payload,
        )
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
                    ::quent_v2_model_ir::event::EventField::from_type(
                        ::quent_v2_model_ir::event::EventFieldType::Payload(
                            <#ty as ::quent_v2_model_ir::value_type::ModelValueType>::model_value_type(),
                        ),
                    )
                ]
            })
        }
        syn::Fields::Unnamed(_) => Err(syn::Error::new_spanned(
            span_source,
            "#[derive(Entity)] does not support enum variants with more than one unnamed field",
        )),
        syn::Fields::Named(named) => {
            // Note the IR crate has a blanket impl of ModelEventFieldType for
            // application-specific value types, which implement ModelValueType.
            let field_defs = named.named.iter().map(|f| {
                let field_name = f.ident.as_ref().unwrap().to_string();
                let ty = &f.ty;
                quote! {
                    ::quent_v2_model_ir::event::EventField::new(
                        ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#field_name),
                        <#ty as ::quent_v2_model_ir::event::ModelEventFieldType>::model_event_field_type(),
                    )
                }
            });
            Ok(quote! {
                ::std::vec![ #(#field_defs),* ]
            })
        }
    }
}

fn emit_qualifications(entity: &Entity) -> TokenStream {
    let quals = entity.qualifications.iter().filter_map(emit_qualification);
    quote! { ::std::vec![ #(#quals),* ] }
}

fn emit_qualification(q: &Qualification) -> Option<TokenStream> {
    match q {
        Qualification::Fsm(fsm) => Some(emit_fsm(fsm)),
        Qualification::Resource(resource) => Some(emit_resource(resource)),
    }
}

fn emit_resource(resource: &Resource) -> TokenStream {
    let capacities = resource.capacities.iter().map(|c| {
        let name = c.name.as_str();
        let kind = emit_capacity_kind(&c.kind);
        let boundedness = emit_boundedness(&c.boundedness);
        quote! {
            ::quent_v2_model_ir::qualifications::resource::Capacity {
                name: ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name),
                kind: #kind,
                boundedness: #boundedness,
            }
        }
    });
    quote! {
        ::quent_v2_model_ir::qualifications::Qualification::Resource(
            ::quent_v2_model_ir::qualifications::resource::Resource {
                capacities: ::std::vec![ #(#capacities),* ],
            },
        )
    }
}

fn emit_capacity_kind(k: &CapacityKind) -> TokenStream {
    match k {
        CapacityKind::Occupancy => quote! {
            ::quent_v2_model_ir::qualifications::resource::CapacityKind::Occupancy
        },
        CapacityKind::Rate => quote! {
            ::quent_v2_model_ir::qualifications::resource::CapacityKind::Rate
        },
    }
}

fn emit_boundedness(b: &Boundedness) -> TokenStream {
    match b {
        Boundedness::Fixed => quote! {
            ::quent_v2_model_ir::qualifications::resource::Boundedness::Fixed
        },
        Boundedness::Resizable => quote! {
            ::quent_v2_model_ir::qualifications::resource::Boundedness::Resizable
        },
        Boundedness::Unbounded => quote! {
            ::quent_v2_model_ir::qualifications::resource::Boundedness::Unbounded
        },
    }
}

fn emit_fsm(fsm: &Fsm) -> TokenStream {
    let transitions = fsm.transitions.iter().map(|t| {
        let source = emit_state(&t.source);
        let target = emit_state(&t.target);
        quote! {
            ::quent_v2_model_ir::qualifications::fsm::Transition {
                source: #source,
                target: #target,
            }
        }
    });
    quote! {
        ::quent_v2_model_ir::qualifications::Qualification::Fsm(
            ::quent_v2_model_ir::qualifications::fsm::Fsm {
                transitions: ::std::vec![ #(#transitions),* ],
            },
        )
    }
}

fn emit_state(s: &State) -> TokenStream {
    match s {
        State::Entry => quote! { ::quent_v2_model_ir::qualifications::fsm::State::Entry },
        State::Exit => quote! { ::quent_v2_model_ir::qualifications::fsm::State::Exit },
        State::State(id) => {
            let name = id.as_str();
            quote! {
                ::quent_v2_model_ir::qualifications::fsm::State::State(
                    ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name)
                )
            }
        }
    }
}
