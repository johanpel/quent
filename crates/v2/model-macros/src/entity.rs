use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let name_str = name.to_string();

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "#[derive(Entity)] does not support generics",
        ));
    }

    match &input.data {
        syn::Data::Struct(s) => expand_struct(name, &name_str, &s.fields, &input),
        syn::Data::Enum(_e) => todo!(),
        syn::Data::Union(u) => Err(syn::Error::new_spanned(
            u.union_token,
            "#[derive(Entity)] not supported on union, use struct or enum",
        )),
    }
}

/// Expand a struct into a single-event entity.
fn expand_struct(
    name: &syn::Ident,
    name_str: &str,
    fields: &syn::Fields,
    input: &DeriveInput,
) -> syn::Result<TokenStream> {
    // Same reason to reject as attribute derives. Rust's naming convention
    // leading with a digit clashes with the spec.
    if matches!(fields, syn::Fields::Unnamed(_)) {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Entity)] on a struct requires a unit struct or a struct with named fields",
        ));
    }

    let entity_decl = quote! {
        impl ::quent_v2_model::EntityDeclaration for #name {}
    };

    let entity_ref_target = quote! {
        impl ::quent_v2_model::ir::value_type::ModelEntityRefTarget for #name {
            fn model_entity_ref_target() -> ::quent_v2_model::ir::attributes::EntityRefTarget {
                ::quent_v2_model::ir::attributes::EntityRefTarget::Specific(
                    #name_str.to_string()
                )
            }
        }
    };

    let entity = quote! {
        impl ::quent_v2_model::ir::entity::ModelEntity for #name {
            fn model_entity() -> ::quent_v2_model::ir::entity::Entity {
                ::quent_v2_model::ir::entity::Entity {
                    name: #name_str.to_string(),
                    events: ::std::vec![
                        ::quent_v2_model::ir::event::Event {
                            name: #name_str.to_string(),
                            cardinality: ::quent_v2_model::ir::event::Cardinality::Once,
                            payload: ::quent_v2_model::ir::event::Payload::Value(
                                ::quent_v2_model::ir::value_type::ValueType::Attributes(
                                    #name_str.to_string()
                                )
                            ),
                        }
                    ],
                    qualifications: ::std::vec::Vec::new(),
                    rust_path: ::std::format!(
                        "{}::{}",
                        ::std::module_path!(),
                        #name_str
                    ),
                }
            }
        }
    };

    Ok(quote! {
        #entity_decl
        #entity_ref_target
        #entity
    })
}
