// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use proc_macro2::TokenStream;
use quent_v2_model_ir::identifier::Identifier;
use quote::quote;
use syn::DeriveInput;

pub(crate) fn expand_struct(input: DeriveInput) -> syn::Result<TokenStream> {
    let name_ident = &input.ident;
    let name_string = input.ident.to_string();

    // Validate the name of the type derived from is a valid identifier in Quent:
    Identifier::try_new(name_string.clone())
        .map_err(|e| syn::Error::new(name_ident.span(), e.to_string()))?;

    // Generics are not supported for now :tm:.
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "#[derive(Record)] does not support generic types",
        ));
    }

    // Get the fields.
    let fields: &syn::Fields = match &input.data {
        syn::Data::Enum(e) => {
            return Err(syn::Error::new_spanned(
                e.enum_token,
                "#[derive(Record)] not allowed on enum",
            ));
        }
        syn::Data::Union(u) => {
            return Err(syn::Error::new_spanned(
                u.union_token,
                "#[derive(Record)] not allowed on union",
            ));
        }
        syn::Data::Struct(s) => match &s.fields {
            // Using Rust's convention for unnamed fields (enumerating them as
            // "0", "1", ...) would violate rules of forming a valid Identifier.
            syn::Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    &input,
                    "#[derive(Record)] requires a unit struct or a struct with named fields",
                ));
            }
            _ => &s.fields,
        },
    };

    // Iterator over token streams representing field defs.
    let fields = fields.iter().map(|f| {
        // Safety: should be safe to unwrap since we rejected tuple structs.
        let field_name = &f.ident.as_ref().unwrap().to_string();
        let field_type = &f.ty;
        quote! {
            ::quent_v2_model_ir::record::Field {
                name: ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#field_name),
                ty: <#field_type as ::quent_v2_model::data_type::DataType>::ir(),
            }
        }
    });

    // Emit the traits that produce the IR from this type
    Ok(quote! {
        impl ::quent_v2_model::record::Record for #name_ident {
            fn ir() -> ::quent_v2_model_ir::record::Record {
                ::quent_v2_model_ir::record::Record {
                    name: ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string),
                    rust_path: ::std::format!("{}::{}", ::std::module_path!(), #name_string),
                    fields: ::std::vec![
                        #(#fields),*
                    ],
                }
            }
        }

        impl ::quent_v2_model::data_type::DataType for #name_ident {
            fn ir() -> ::quent_v2_model_ir::data_type::DataType {
                ::quent_v2_model_ir::data_type::DataType::Record(
                    ::quent_v2_model_ir::identifier::Identifier::new_unchecked(#name_string),
                )
            }
        }
    })
}
