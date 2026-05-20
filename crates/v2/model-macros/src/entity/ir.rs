use proc_macro2::TokenStream;
use quent_v2_model_ir::{entity::Entity, qualifications::Qualification};
use quote::quote;
use syn::{DeriveInput, Token, Variant, punctuated::Punctuated};

fn qualification_marker_impls(name: &syn::Ident, entity: &Entity) -> TokenStream {
    let mut out = TokenStream::new();
    for q in &entity.qualifications {
        match q {
            Qualification::ResourceGroup(rg) => {
                let is_root = rg.is_root;
                out.extend(quote! {
                    impl ::quent_v2_model::ResourceGroupDeclaration for #name {
                        const IS_ROOT: bool = #is_root;
                    }
                });
            }
            Qualification::Fsm(_) | Qualification::Resource(_) => {}
        }
    }
    out
}

/// Expand a struct into a single-event entity.
pub fn expand_struct(
    name: &syn::Ident,
    fields: &syn::Fields,
    input: &DeriveInput,
    entity: &Entity,
) -> syn::Result<TokenStream> {
    // TODO(johanpel): this would be caught at IR validation already so could remove
    if matches!(fields, syn::Fields::Unnamed(_)) {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Entity)] on a struct requires a unit struct or a struct with named fields",
        ));
    }

    let name_string = name.to_string();

    let has_payload = match fields {
        syn::Fields::Unit => false,
        syn::Fields::Named(n) => !n.named.is_empty(),
        syn::Fields::Unnamed(_) => unreachable!(),
    };

    let payload = if has_payload {
        quote! {
            ::std::vec![
                ::quent_v2_model_ir::event::Field::new(
                    ::quent_v2_model_ir::event::FieldRole::Payload,
                    ::quent_v2_model_ir::value_type::ValueType::Attributes(
                        ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string)
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
                    ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string)
                )
            }
        }
    };

    let entity_impl = quote! {
        impl ::quent_v2_model_ir::entity::ModelEntity for #name {
            fn model_entity() -> ::quent_v2_model_ir::entity::Entity {
                ::quent_v2_model_ir::entity::Entity::new(
                    ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string),
                    ::std::vec![
                        ::quent_v2_model_ir::event::Event::new(
                            ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string),
                            ::quent_v2_model_ir::event::Cardinality::Once,
                            #payload,
                        )
                    ],
                    ::std::vec::Vec::new(),
                    ::std::format!(
                        "{}::{}",
                        ::std::module_path!(),
                        #name_string,
                    ),
                )
            }
        }
    };

    let qualification_markers = qualification_marker_impls(name, entity);

    Ok(quote! {
        #entity_decl
        #entity_ref_target
        #entity_impl
        #qualification_markers
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
    variants: &Punctuated<Variant, Token![,]>,
    input: &DeriveInput,
    entity: &Entity,
) -> syn::Result<TokenStream> {
    // TODO(johanpel): this would be caught at IR validation already so could remove
    if variants.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Entity)] requires the enum to have at least one variant, otherwise it would have no event",
        ));
    }

    let name_string = name.to_string();

    // Ensure the type is considered an entity for entity references.
    let entity_decl = quote! {
        impl ::quent_v2_model::EntityDeclaration for #name {}
    };
    let entity_ref_target = quote! {
        impl ::quent_v2_model_ir::value_type::ModelEntityRefTarget for #name {
            fn model_entity_ref_target() -> ::quent_v2_model_ir::attributes::EntityRefTarget {
                ::quent_v2_model_ir::attributes::EntityRefTarget::Specific(
                    ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string)
                )
            }
        }
    };

    // Derive the events
    let events: Vec<TokenStream> = variants
        .iter()
        .map(expand_variant)
        .collect::<syn::Result<_>>()?;

    let entity_impl = quote! {
        impl ::quent_v2_model_ir::entity::ModelEntity for #name {
            fn model_entity() -> ::quent_v2_model_ir::entity::Entity {
                ::quent_v2_model_ir::entity::Entity::new(
                    ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string),
                    ::std::vec![ #(#events),* ],
                    ::std::vec::Vec::new(),
                    ::std::format!(
                        "{}::{}",
                        ::std::module_path!(),
                        #name_string,
                    ),
                )
            }
        }
    };

    let qualification_markers = qualification_marker_impls(name, entity);

    Ok(quote! {
        #entity_decl
        #entity_ref_target
        #entity_impl
        #qualification_markers
    })
}

fn expand_variant(v: &Variant) -> syn::Result<TokenStream> {
    let variant_name = v.ident.to_string();
    let cardinality = parse_cardinality(&v.attrs)?;
    let payload = expand_variant_payload(&v.fields, v)?;

    Ok(quote! {
        ::quent_v2_model_ir::event::Event::new(
            ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#variant_name),
            #cardinality,
            #payload,
        )
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
                        ::quent_v2_model_ir::event::FieldRole::Payload,
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
            let field_defs = named
                .named
                .iter()
                .map(|f| {
                    let field_name = f.ident.as_ref().unwrap();
                    let role_tokens = field_role_tokens(field_name)?;
                    let ty = &f.ty;
                    Ok::<TokenStream, syn::Error>(quote! {
                        ::quent_v2_model_ir::event::Field::new(
                            #role_tokens,
                            <#ty as ::quent_v2_model_ir::value_type::ModelValueType>::model_value_type(),
                        )
                    })
                })
                .collect::<syn::Result<Vec<_>>>()?;
            Ok(quote! {
                ::std::vec![ #(#field_defs),* ]
            })
        }
    }
}

fn field_role_tokens(name: &syn::Ident) -> syn::Result<TokenStream> {
    match name.to_string().as_str() {
        "payload" => Ok(quote! { ::quent_v2_model_ir::event::FieldRole::Payload }),
        "parent" => Ok(quote! { ::quent_v2_model_ir::event::FieldRole::Parent }),
        other => Err(syn::Error::new(
            name.span(),
            format!("`{other}` is not a reserved event field name (allowed: payload, parent)"),
        )),
    }
}
