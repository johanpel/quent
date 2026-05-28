// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use proc_macro::TokenStream;

mod parse;
mod resource;

/// `resource! { [#[derive(...)]]? [vis] Name { field: Capacity<T, K, B>, ... } }`
///
/// Emits a `#[derive(Entity)]` enum with the canonical resource lifecycle FSM
/// and a `Resource` constraint listing the capacities. The Entity macro builds
/// the actual IR via its `#[quent(fsm(...), constraint(...))]` hooks; this
/// macro only translates the resource DSL into that input.
#[proc_macro]
pub fn resource(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as resource::ResourceInput);
    resource::expand(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
