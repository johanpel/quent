use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let name_ident = &input.ident;
    let name_string = &input.ident.to_string();

    // Generics are not supported for now :tm:.
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "#[derive(Attributes)] does not support generic types",
        ));
    }

    // Get the fields.
    let fields: &syn::Fields = match &input.data {
        syn::Data::Enum(e) => {
            return Err(syn::Error::new_spanned(
                e.enum_token,
                "#[derive(Attributes)] not allowed on enum",
            ));
        }
        syn::Data::Union(u) => {
            return Err(syn::Error::new_spanned(
                u.union_token,
                "#[derive(Attributes)] not allowed on union",
            ));
        }
        syn::Data::Struct(s) => match &s.fields {
            // Using Rust's convention for unnamed fields (enumerating them)
            // would violate the modeling spec, which sort of follows ANSI C
            // field naming rules as a common denominator across all sorts of
            // target languages. We could choose to prefix it with _ or
            // something, but that would mean the event data would have a
            // mangled field name vs. the Rust struct declaration.
            syn::Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    &input,
                    "#[derive(Attributes)] requires a struct with named fields",
                ));
            }
            _ => &s.fields,
        },
    };

    // Iterator over token streams representing field defs.
    let field_defs = fields.iter().map(|f| {
        // Safety: should be safe to unwrap since we rejected tuple structs.
        let field_name = (&f.ident).as_ref().unwrap().to_string();
        let field_type = &f.ty;
        quote! {
            ::quent_v2_model::ir::attributes::FieldDef {
                name: #field_name.to_string(),
                value_type: <#field_type as ::quent_v2_model::HasValueType>::value_type(),
            }
        }
    });

    // Emit HasAttributesDef and HasValueType traits.
    Ok(quote! {
        impl ::quent_v2_model::HasAttributesDef for #name_ident {
            fn attributes_def() -> ::quent_v2_model::ir::attributes::AttributesDef {
                ::quent_v2_model::ir::attributes::AttributesDef {
                    name: #name_string.to_string(),
                    rust_path: ::std::format!("{}::{}", ::std::module_path!(), #name_string),
                    fields: ::std::vec![
                        #(#field_defs),*
                    ],
                }
            }
        }

        impl ::quent_v2_model::HasValueType for #name_ident {
            fn value_type() -> ::quent_v2_model::ir::attributes::ValueType {
                ::quent_v2_model::ir::attributes::ValueType::Attributes(#name_string.to_string())
            }
        }
    })
}
