use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod attributes;

#[proc_macro_derive(Attributes)]
pub fn derive_attributes(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    attributes::expand(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

// TODO
#[proc_macro_derive(Entity, attributes(quent))]
pub fn derive_entity(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(Fsm, attributes(quent))]
pub fn derive_fsm(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(Resource, attributes(quent))]
pub fn derive_resource(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(ResourceGroup, attributes(quent))]
pub fn derive_resource_group(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(RootResourceGroup, attributes(quent))]
pub fn derive_root_resource_group(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
