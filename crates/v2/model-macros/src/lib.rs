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
pub fn RootRg(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
