// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use convert_case::{Case, Casing};
use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Variant;

pub(crate) fn emit_observer(
    name: &syn::Ident,
    observer_name: &syn::Ident,
    handle_name: &syn::Ident,
    vis: &syn::Visibility,
) -> TokenStream {
    let observer_type = quote! {
        #vis struct #observer_name {
            inner: ::quent_v2_instrumentation::Observer<#name>,
        }
    };
    let observer_impl = quote! {
        impl #observer_name {
            #vis fn try_new(
                root_id: ::uuid::Uuid,
                opts: ::std::option::Option<::quent_v2_instrumentation::ExporterOptions>,
            ) -> ::std::result::Result<Self, ::quent_v2_instrumentation::ObserverError> {
                ::std::result::Result::Ok(Self {
                    inner: ::quent_v2_instrumentation::Observer::new(root_id, opts)?,
                })
            }

            #vis fn handle(&self) -> #handle_name {
                #handle_name {
                    inner: ::quent_v2_instrumentation::handle::Handle::new(
                        self.inner.sender(),
                        ::uuid::Uuid::now_v7(),
                    ),
                }
            }
        }
    };
    quote! {
        #observer_type

        #observer_impl
    }
}

pub(crate) fn emit_handle(
    name: &syn::Ident,
    handle_name: &syn::Ident,
    vis: &syn::Visibility,
    variants: &IndexMap<String, &Variant>,
) -> syn::Result<TokenStream> {
    let handle_type = quote! {
        #vis struct #handle_name {
            inner: ::quent_v2_instrumentation::handle::Handle<#name>,
        }
    };
    let handle_methods: Vec<TokenStream> = variants
        .iter()
        .map(|(_, v)| emit_handle_method(name, v, vis))
        .collect::<syn::Result<_>>()?;
    let handle_impl = quote! {
        impl #handle_name {
            #(#handle_methods)*
        }
    };
    let entity_handle_impl = quote! {
        impl ::quent_v2_model::EntityHandle for #handle_name {
            type DeclarationType = #name;
            fn id(&self) -> ::uuid::Uuid {
                self.inner.id()
            }
        }
    };
    Ok(quote! {
        #handle_type

        #handle_impl

        #entity_handle_impl
    })
}

pub(crate) fn emit_handle_method(
    enum_name: &syn::Ident,
    variant: &Variant,
    vis: &syn::Visibility,
) -> syn::Result<TokenStream> {
    let variant_ident = &variant.ident;
    let method_name = format_ident!("{}", variant_ident.to_string().to_case(Case::Snake));

    let (args, construct) = match &variant.fields {
        syn::Fields::Unit => (quote! {}, quote! { #enum_name::#variant_ident }),
        syn::Fields::Unnamed(u) if u.unnamed.len() == 1 => {
            let ty = &u.unnamed.first().unwrap().ty;
            (
                quote! { payload: #ty },
                quote! { #enum_name::#variant_ident(payload) },
            )
        }
        syn::Fields::Unnamed(_) => {
            return Err(syn::Error::new_spanned(
                variant,
                "#[derive(Entity)] does not support enum variants with more than one unnamed field",
            ));
        }
        syn::Fields::Named(named) => {
            let arg_defs = named.named.iter().map(|f| {
                let ident = f.ident.as_ref().unwrap();
                let ty = &f.ty;
                quote! { #ident: #ty }
            });
            let field_idents: Vec<_> = named
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap())
                .collect();
            (
                quote! { #(#arg_defs),* },
                quote! { #enum_name::#variant_ident { #(#field_idents),* } },
            )
        }
    };

    Ok(quote! {
        #vis fn #method_name(
            &self,
            #args
        ) -> ::std::result::Result<(), ::quent_v2_instrumentation::ObserverError> {
            self.inner.emit(#construct)
        }
    })
}
