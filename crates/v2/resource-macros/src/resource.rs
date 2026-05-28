// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! `resource! { ... }` implementation.
//!
//! Reads a DSL describing a resource (name + capacity fields), then emits a
//! `#[derive(Entity)]` enum carrying the canonical resource lifecycle FSM and
//! a `"Resource"` convention. The Entity macro does all the IR construction;
//! this macro only translates the DSL into Entity attributes.
//!
//! Canonical FSM topology (Mealy-style transitions named after destination):
//! - `entry -> init -> operating -> finalizing -> exit`
//! - if any capacity has `Boundedness::Resizable`: add `operating -> resizing -> operating`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Token, braced, parse::Parse, punctuated::Punctuated};

use crate::parse::{
    CapacityField, ParsedBoundedness, ParsedCapacity, ParsedCapacityKind, capacities,
};

pub struct ResourceInput {
    pub attrs: Vec<syn::Attribute>,
    pub vis: syn::Visibility,
    pub name: syn::Ident,
    pub fields: Punctuated<CapacityField, Token![,]>,
}

impl Parse for ResourceInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let vis: syn::Visibility = input.parse()?;
        let name: syn::Ident = input.parse()?;
        let content;
        let _brace = braced!(content in input);
        let fields = content.parse_terminated(CapacityField::parse, Token![,])?;
        Ok(Self {
            attrs,
            vis,
            name,
            fields,
        })
    }
}

pub fn expand(input: ResourceInput) -> syn::Result<TokenStream> {
    let ResourceInput {
        attrs,
        vis,
        name,
        fields,
    } = input;

    let caps = capacities(&fields)?;
    if caps.is_empty() {
        return Err(syn::Error::new_spanned(
            &name,
            "resource! requires at least one capacity field",
        ));
    }
    if caps.len() > 1 {
        return Err(syn::Error::new_spanned(
            &name,
            "multi-capacity resources not yet supported",
        ));
    }

    let any_resizable = caps
        .iter()
        .any(|c| matches!(c.boundedness, ParsedBoundedness::Resizable));

    let bounds_ty = bounds_type_tokens(&caps);

    let fsm_arm = fsm_arm_tokens(any_resizable);
    let convention_arm = convention_arm_tokens(&caps);
    let variants = enum_variants_tokens(&bounds_ty, any_resizable);

    Ok(quote! {
        #(#attrs)*
        #[derive(::quent_v2_model::Entity)]
        #[quent(
            fsm(#fsm_arm),
            convention("Resource", #convention_arm),
        )]
        #vis enum #name {
            #variants
        }

        impl ::quent_v2_resource::ResourceEntity for #name {
            type UsageType = u64;
            type BoundsType = #bounds_ty;
        }
    })
}

fn bounds_type_tokens(caps: &[ParsedCapacity]) -> TokenStream {
    match caps[0].kind {
        ParsedCapacityKind::Occupancy => quote! { ::quent_v2_resource::OccupancyBound<u64> },
        ParsedCapacityKind::Rate => quote! { ::quent_v2_resource::RateBound<u64> },
    }
}

fn fsm_arm_tokens(any_resizable: bool) -> TokenStream {
    let mut t = quote! {
        entry -> init,
        init -> operating,
        operating -> finalizing,
        finalizing -> exit
    };
    if any_resizable {
        t.extend(quote! {
            ,
            operating -> resizing,
            resizing -> operating
        });
    }
    t
}

/// Emit a Rust expression producing the JSON-encoded `ResourceData` string for
/// the `"Resource"` convention payload.
fn convention_arm_tokens(caps: &[ParsedCapacity]) -> TokenStream {
    let cap_literals = caps.iter().map(|c| {
        let n = c.name_ir.as_str();
        let kind = match c.kind {
            ParsedCapacityKind::Occupancy => quote! {
                ::quent_v2_resource::CapacityKindData::Occupancy
            },
            ParsedCapacityKind::Rate => quote! {
                ::quent_v2_resource::CapacityKindData::Rate
            },
        };
        let boundedness = match c.boundedness {
            ParsedBoundedness::Fixed => quote! {
                ::quent_v2_resource::BoundednessData::Fixed
            },
            ParsedBoundedness::Resizable => quote! {
                ::quent_v2_resource::BoundednessData::Resizable
            },
            ParsedBoundedness::Unbounded => quote! {
                ::quent_v2_resource::BoundednessData::Unbounded
            },
        };
        quote! {
            ::quent_v2_resource::CapacityData {
                name: ::std::string::String::from(#n),
                kind: #kind,
                boundedness: #boundedness,
            }
        }
    });
    quote! {
        {
            let __data = ::quent_v2_resource::ResourceData {
                capacities: ::std::vec![ #(#cap_literals),* ],
            };
            ::quent_v2_resource::__private::to_json(&__data)
        }
    }
}

fn enum_variants_tokens(bounds_ty: &TokenStream, any_resizable: bool) -> TokenStream {
    let operating = if any_resizable {
        quote! { #[quent(multi)] operating }
    } else {
        quote! { operating }
    };
    let resizing = if any_resizable {
        quote! {
            ,
            #[quent(multi)]
            resizing(#bounds_ty)
        }
    } else {
        quote! {}
    };
    quote! {
        init(#bounds_ty),
        #operating,
        finalizing
        #resizing
    }
}
