// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Extract `#[doc = "..."]` attributes (i.e. `///` rustdoc lines) into a
//! single `Option<String>` for the IR `docs` field.

use proc_macro2::TokenStream;
use quote::quote;

/// Concatenate all `#[doc = "..."]` attributes of `attrs` into a single
/// `\n`-separated string. Returns `None` if none are present.
pub fn extract_docs(attrs: &[syn::Attribute]) -> Option<String> {
    let lines: Vec<String> = attrs
        .iter()
        .filter_map(|a| {
            if !a.path().is_ident("doc") {
                return None;
            }
            let syn::Meta::NameValue(nv) = &a.meta else {
                return None;
            };
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
            else {
                return None;
            };
            // rustdoc convention: a leading single space appears for `/// foo`.
            let raw = s.value();
            Some(raw.strip_prefix(' ').map(str::to_string).unwrap_or(raw))
        })
        .collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// Emit the docs tokens as an `Option<String>` expression.
pub fn emit_docs_tokens(docs: Option<&str>) -> TokenStream {
    match docs {
        Some(s) => quote! { ::std::option::Option::Some(::std::string::String::from(#s)) },
        None => quote! { ::std::option::Option::None },
    }
}
