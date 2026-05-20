use quent_v2_model_ir::qualifications::resource_group::ResourceGroup;
use syn::parse::ParseStream;

pub fn parse(input: ParseStream) -> syn::Result<ResourceGroup> {
    if !input.peek(syn::token::Paren) {
        return Ok(ResourceGroup { is_root: false });
    }
    let content;
    syn::parenthesized!(content in input);
    if content.is_empty() {
        return Ok(ResourceGroup { is_root: false });
    }
    let ident: syn::Ident = content.parse()?;
    if ident != "root" {
        return Err(syn::Error::new(
            ident.span(),
            format!("unknown resource_group argument: `{ident}`"),
        ));
    }
    if !content.is_empty() {
        return Err(syn::Error::new(
            content.span(),
            "unexpected tokens after `root`",
        ));
    }
    Ok(ResourceGroup { is_root: true })
}
