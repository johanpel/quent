use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod attributes;
mod entity;
mod value_type;

/// TODO(johanpel): general docs in addition to diving into details below.
///
/// `#[derive(Attributes)]` is only required for cross-language code generation
/// workflows because it only implements a trait through which its IR
/// representation can be obtained at run-time. This requires the `ir` feature
/// to be enabled. For a pure Rust workflow (Rust model source, Rust application
/// using instrumentation), it is not necessary to use this derive macro. Note
/// that this derive macro stays available and no compilation error is produced
/// even if this derive is used without the `ir` feature flag enabled, because
/// this allows non-pure-Rust workflows to reuse the model declarations.
#[proc_macro_derive(Attributes)]
pub fn derive_attributes(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    if cfg!(feature = "ir") {
        attributes::ir::expand_struct(input)
            .unwrap_or_else(|err| err.to_compile_error())
            .into()
    } else {

        TokenStream::new()
    }
}

#[proc_macro_derive(Entity, attributes(quent))]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    // TokenStream::new()
    let input = parse_macro_input!(input as DeriveInput);
    entity::expand(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
