use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

mod ir;
mod obs;

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let name_str = name.to_string();

    // Rejections
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "#[derive(Entity)] does not support generics",
        ));
    }

    let (obs, ir) = match &input.data {
        syn::Data::Struct(s) => Ok((
            obs::expand_struct(name, &name_str, &s.fields, &input)?,
            ir::expand_struct(name, &name_str, &s.fields, &input)?,
        )),
        syn::Data::Enum(e) => Ok((
            obs::expand_enum(name, &name_str, &e.variants, &input)?,
            ir::expand_enum(name, &name_str, &e.variants, &input)?,
        )),
        syn::Data::Union(u) => Err(syn::Error::new_spanned(
            u.union_token,
            "#[derive(Entity)] not supported for union, use struct or enum",
        )),
    }?;

    Ok(quote! {
        #obs
        #ir
    })
}
