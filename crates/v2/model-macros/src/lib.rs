// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod data_type;
pub(crate) mod docs;
mod entity;
mod record;

/// TODO(johanpel): general docs in addition to diving into details below.
///
/// `#[derive(Record)]` is only required for cross-language code generation
/// workflows because it only implements a trait through which its IR
/// representation can be obtained at run-time. This requires the `ir` feature
/// to be enabled. For a pure Rust workflow (Rust model source, Rust
/// application), it is not necessary to use this derive macro.
///
/// Note that this derive macro stays available and no compilation error is
/// produced even if this derive is used without the `ir` feature flag enabled,
/// because this allows non-pure-Rust workflows to reuse the model sources.
#[proc_macro_derive(Record)]
pub fn derive_record(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    if cfg!(feature = "ir") {
        record::ir::expand_struct(input)
            .unwrap_or_else(|err| err.to_compile_error())
            .into()
    } else {
        TokenStream::new()
    }
}

#[proc_macro_derive(Entity, attributes(quent))]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    entity::expand(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
