use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Token, Variant, punctuated::Punctuated};

pub fn expand_struct(
    name: &syn::Ident,
    name_str: &str,
    fields: &syn::Fields,
    input: &DeriveInput,
) -> syn::Result<TokenStream> {
    if matches!(fields, syn::Fields::Unnamed(_)) {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Entity)] on a struct requires a unit struct or a struct with named fields",
        ));
    }

    let vis = &input.vis;
    let observer_name = format_ident!("{}Observer", name);
    let handle_name = format_ident!("{}Handle", name);
    let method_name = format_ident!("{}", name_str.to_case(Case::Snake));

    let (event_arg, event_value) = match fields {
        syn::Fields::Unit => (quote! {}, quote! { #name }),
        _ => (quote! { payload: #name, }, quote! { payload }),
    };

    Ok(quote! {
        #vis struct #observer_name {
            inner: ::quent_v2_instrumentation::Observer<#name>,
        }

        impl #observer_name {
            #vis fn new(
                root_id: ::uuid::Uuid,
                opts: ::quent_v2_instrumentation::ExporterOptions,
            ) -> ::std::result::Result<Self, ::quent_v2_instrumentation::ObserverError> {
                ::std::result::Result::Ok(Self {
                    inner: ::quent_v2_instrumentation::Observer::new(root_id, opts)?,
                })
            }

            #vis fn handle(&self) -> #handle_name {
                #handle_name {
                    inner: ::quent_v2_instrumentation::Handle::new(
                        self.inner.sender(),
                        ::uuid::Uuid::now_v7(),
                    ),
                }
            }
        }

        #vis struct #handle_name {
            inner: ::quent_v2_instrumentation::Handle<#name>,
        }

        impl ::std::ops::Deref for #handle_name {
            type Target = ::quent_v2_instrumentation::Handle<#name>;
            fn deref(&self) -> &Self::Target {
                &self.inner
            }
        }

        impl #handle_name {
            #vis fn #method_name(
                &self,
                #event_arg
            ) -> ::std::result::Result<(), ::quent_v2_instrumentation::ObserverError> {
                self.inner.emit(#event_value)
            }
        }
    })
}

pub fn expand_enum(
    name: &syn::Ident,
    name_str: &str,
    variants: &Punctuated<Variant, Token![,]>,
    input: &DeriveInput,
) -> syn::Result<TokenStream> {
    Ok(TokenStream::new())
}
