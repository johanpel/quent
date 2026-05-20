//! #[derive(Entity)] implementation

use proc_macro2::TokenStream;
use quent_v2_model_ir::{
    entity::Entity,
    identifier::{Identifier, IdentifierError},
};
use quote::quote;
use syn::DeriveInput;

mod instrumentation;
mod ir;
mod parse;

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let name_ir: Identifier = name
        .to_string()
        .try_into()
        .map_err(|e: IdentifierError| syn::Error::new(name.span(), e.to_string()))?;

    // Fail quickly if there are generics or if this is used on a union.
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "#[derive(Entity)] does not support generics",
        ));
    }
    if let syn::Data::Union(u) = input.data {
        return Err(syn::Error::new_spanned(
            u.union_token,
            "#[derive(Entity)] not supported for union, use struct or enum",
        ));
    }

    // Parse into IR entity.
    let attributes: Vec<&syn::Attribute> = input
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("quent"))
        .collect();
    let events = parse::events::parse(&input)?;
    let qualifications = parse::qualifications::parse(&attributes)?;
    let entity = Entity::new(name_ir, events, qualifications, name.to_string());
    // Run IR entity validation.
    if let Err(errs) = entity.validate() {
        return Err(syn::Error::new_spanned(
            name,
            errs.into_iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        ));
    }

    // Emit the instrumentation library
    let instrumentation = if cfg!(feature = "instrumentation") {
        match &input.data {
            syn::Data::Struct(s) => instrumentation::expand_struct(name, &s.fields, &input),
            syn::Data::Enum(e) => instrumentation::expand_enum(name, &e.variants, &input, &entity),
            syn::Data::Union(_) => unreachable!(),
        }?
    } else {
        TokenStream::new()
    };

    // Emit the IR trait impls
    let ir = if cfg!(feature = "ir") {
        match &input.data {
            syn::Data::Struct(s) => ir::expand_struct(name, &s.fields, &input, &entity),
            syn::Data::Enum(e) => ir::expand_enum(name, &e.variants, &input, &entity),
            syn::Data::Union(_) => unreachable!(),
        }?
    } else {
        TokenStream::new()
    };

    Ok(quote! {
        #instrumentation

        #ir
    })
}
