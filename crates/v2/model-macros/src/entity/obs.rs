use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Token, Variant, punctuated::Punctuated};

pub fn expand_struct(
    name: &syn::Ident,
    name_str: &str,
    fields: &syn::Fields,
    input: &DeriveInput,
) -> syn::Result<TokenStream> {
    todo!()
}

pub fn expand_enum(
    name: &syn::Ident,
    name_str: &str,
    variants: &Punctuated<Variant, Token![,]>,
    input: &DeriveInput,
) -> syn::Result<TokenStream> {
    todo!()
}
