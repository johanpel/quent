use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod attributes;
mod entity;
mod value_type;

#[proc_macro_derive(Attributes)]
pub fn derive_attributes(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    attributes::expand(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_derive(Entity, attributes(quent))]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    // TokenStream::new()
    let input = parse_macro_input!(input as DeriveInput);
    entity::expand(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
