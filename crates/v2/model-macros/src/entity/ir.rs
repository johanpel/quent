use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Token, Variant, punctuated::Punctuated};

/// Expand a struct into a single-event entity.
pub fn expand_struct(
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

    let has_payload = match fields {
        syn::Fields::Unit => false,
        syn::Fields::Named(n) => !n.named.is_empty(),
        syn::Fields::Unnamed(_) => unreachable!(),
    };

    let payload = if has_payload {
        quote! {
            ::std::vec![
                ::quent_v2_model_ir::event::Field::new(
                    "payload",
                    ::quent_v2_model_ir::value_type::ValueType::Attributes(
                        #name_str.to_string()
                    ),
                )
            ]
        }
    } else {
        quote! { ::std::vec::Vec::new() }
    };

    // Ensure the type is considered an entity for entity references.
    let entity_decl = quote! {
        impl ::quent_v2_model::EntityDeclaration for #name {}
    };
    let entity_ref_target = quote! {
        impl ::quent_v2_model_ir::value_type::ModelEntityRefTarget for #name {
            fn model_entity_ref_target() -> ::quent_v2_model_ir::attributes::EntityRefTarget {
                ::quent_v2_model_ir::attributes::EntityRefTarget::Specific(
                    #name_str.to_string()
                )
            }
        }
    };

    let entity = quote! {
        impl ::quent_v2_model_ir::entity::ModelEntity for #name {
            fn model_entity() -> ::quent_v2_model_ir::entity::Entity {
                ::quent_v2_model_ir::entity::Entity {
                    name: #name_str.to_string(),
                    events: ::std::vec![
                        ::quent_v2_model_ir::event::Event {
                            name: #name_str.to_string(),
                            cardinality: ::quent_v2_model_ir::event::Cardinality::Once,
                            payload: #payload,
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

// Important to note is that an enum gets flattened out into events.
//
// So, the user doesn't see the enum type itself anymore appear in the
// instrumentation API. Instead, they will see observer/entity handle functions
// with the names of the enum variants.
//
// The inner type of a variant is what they will soo on those functions as
// payload argument.
//
// Other arguments besides the payload can exist, and they are to be supplied
// here as named fields within an enum variant. These will be reserved to adhere
// to certain qualifications.
//
// Thus, if a variant has:
//
// - Unnamed fields: implicitly the payload field, but there can only be one
// (see comments below).
//
// - Named fields: at least one called payload, plus others depending on the
// entity qualification. To simplify things for now :tm:, this validation is not
// done in this derive macro yet.
pub fn expand_enum(
    name: &syn::Ident,
    name_str: &str,
    variants: &Punctuated<Variant, Token![,]>,
    input: &DeriveInput,
) -> syn::Result<TokenStream> {
    if variants.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Entity)] requires the enum to have at least one variant, otherwise it would have no event",
        ));
    }

    // Ensure the type is considered an entity for entity references.
    let entity_decl = quote! {
        impl ::quent_v2_model::EntityDeclaration for #name {}
    };
    let entity_ref_target = quote! {
        impl ::quent_v2_model_ir::value_type::ModelEntityRefTarget for #name {
            fn model_entity_ref_target() -> ::quent_v2_model_ir::attributes::EntityRefTarget {
                ::quent_v2_model_ir::attributes::EntityRefTarget::Specific(
                    #name_str.to_string()
                )
            }
        }
    };

    // Derive the events
    let events: Vec<TokenStream> = variants
        .iter()
        .map(expand_variant)
        .collect::<syn::Result<_>>()?;

    let entity = quote! {
        impl ::quent_v2_model_ir::entity::ModelEntity for #name {
            fn model_entity() -> ::quent_v2_model_ir::entity::Entity {
                ::quent_v2_model_ir::entity::Entity {
                    name: #name_str.to_string(),
                    events: ::std::vec![ #(#events),* ],
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

fn expand_variant(v: &Variant) -> syn::Result<TokenStream> {
    let variant_name = v.ident.to_string();
    let cardinality = parse_cardinality(&v.attrs)?;
    let payload = expand_variant_payload(&v.fields, v)?;

    Ok(quote! {
        ::quent_v2_model_ir::event::Event {
            name: #variant_name.to_string(),
            cardinality: #cardinality,
            payload: #payload,
        }
    })
}

fn parse_cardinality(attrs: &[syn::Attribute]) -> syn::Result<TokenStream> {
    // The cardinality of entity events is Once by default
    let mut is_multi = false;

    // Go over the variant attributes and check for quent-related ones.
    for attr in attrs {
        if !attr.path().is_ident("quent") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("multi") {
                is_multi = true;
                Ok(())
            } else {
                Err(meta.error("unknown #[quent(...)] argument"))
            }
        })?;
    }

    Ok(if is_multi {
        quote! { ::quent_v2_model_ir::event::Cardinality::Multi }
    } else {
        quote! { ::quent_v2_model_ir::event::Cardinality::Once }
    })
}

fn expand_variant_payload(fields: &syn::Fields, span_source: &Variant) -> syn::Result<TokenStream> {
    match fields {
        syn::Fields::Unit => Ok(quote! { ::std::vec::Vec::new() }),
        syn::Fields::Unnamed(u) if u.unnamed.len() == 1 => {
            let ty = &u.unnamed.first().unwrap().ty;
            Ok(quote! {
                ::std::vec![
                    ::quent_v2_model_ir::event::Field::new(
                        "payload",
                        <#ty as ::quent_v2_model_ir::value_type::ModelValueType>::model_value_type(),
                    )
                ]
            })
        }
        syn::Fields::Unnamed(_) => Err(syn::Error::new_spanned(
            span_source,
            "#[derive(Entity)] does not support enum variants with more than one unnamed field",
        )),
        syn::Fields::Named(named) => {
            let field_defs = named.named.iter().map(|f| {
                let field_name = f.ident.as_ref().unwrap().to_string();
                let ty = &f.ty;
                quote! {
                    ::quent_v2_model_ir::event::Field::new(
                        #field_name.to_string(),
                        <#ty as ::quent_v2_model_ir::value_type::ModelValueType>::model_value_type(),
                    )
                }
            });
            Ok(quote! {
                ::std::vec![ #(#field_defs),* ]
            })
        }
    }
}
