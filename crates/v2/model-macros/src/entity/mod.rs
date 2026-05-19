use proc_macro2::TokenStream;
use quent_v2_model_ir::{
    entity::Entity,
    identifier::{Identifier, IdentifierError},
};
use quote::quote;
use syn::DeriveInput;

use crate::entity::{event::parse_events, qualifications::parse_qualifications};

mod event;
mod instrumentation;
mod qualifications;

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let name_ir: Identifier = name
        .to_string()
        .try_into()
        .map_err(|e: IdentifierError| syn::Error::new(name.span(), e.to_string()))?;

    // Fail quickly if there are generics.
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "#[derive(Entity)] does not support generics",
        ));
    }

    // Parse into IR entity and validate.
    let attributes: Vec<&syn::Attribute> = input
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("quent"))
        .collect();
    let events = parse_events(&input)?;
    let qualifications = parse_qualifications(&attributes)?;
    let entity = Entity::new(name_ir, events, qualifications, name.to_string());
    // TODO(johanpel): run validation goes here

    // Emit the instrumentation library
    let instrumentation = match &input.data {
        syn::Data::Struct(s) => instrumentation::expand_struct(name, &s.fields, &input),
        syn::Data::Enum(e) => instrumentation::expand_enum(name, &e.variants, &input, &entity),
        syn::Data::Union(u) => Err(syn::Error::new_spanned(
            u.union_token,
            "#[derive(Entity)] not supported for union, use struct or enum",
        )),
    }?;

    // Ok(TokenStream::new())

    Ok(quote! {
        #instrumentation
        // #ir
    })
}
