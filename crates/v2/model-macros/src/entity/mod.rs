use proc_macro2::TokenStream;
use quent_v2_model_ir::{Span, entity::Entity, qualifications::fsm::Fsm};
use syn::DeriveInput;

use crate::entity::{event::parse_events, qualifications::parse_qualifications};

mod event;
mod qualifications;

// mod ir;
// mod obs;

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let name_str = name.to_string();

    // Reject generics, they are not support (yet).
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "#[derive(Entity)] does not support generics",
        ));
    }

    // Obtain and parse quent attributes
    let quent_attrs: Vec<&syn::Attribute> = input
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("quent"))
        .collect();

    let events = parse_events(&input)?;
    let qualifications = parse_qualifications(&quent_attrs)?;

    let entity = Entity::with_span(
        name_str,
        events,
        qualifications,
        String::new(),
        Span(Some(name.span())),
    );

    // let (obs, ir) = match &input.data {
    //     syn::Data::Struct(s) => Ok((
    //         obs::expand_struct(name, &name_str, &s.fields, &input)?,
    //         ir::expand_struct(name, &name_str, &s.fields, &input)?,
    //     )),
    //     syn::Data::Enum(e) => Ok((
    //         obs::expand_enum(name, &name_str, &e.variants, &input)?,
    //         ir::expand_enum(name, &name_str, &e.variants, &input)?,
    //     )),
    //     syn::Data::Union(u) => Err(syn::Error::new_spanned(
    //         u.union_token,
    //         "#[derive(Entity)] not supported for union, use struct or enum",
    //     )),
    // }?;

    Ok(TokenStream::new())

    // Ok(quote! {
    //     #obs
    //     #ir
    // })
}
