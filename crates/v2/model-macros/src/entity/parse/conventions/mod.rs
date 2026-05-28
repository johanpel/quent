// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model_ir::fsm::Fsm;
use syn::Token;

mod fsm;

/// A parsed `#[quent(convention(<name>, <data_expr>))]` arm.
pub struct ConventionArm {
    /// The name expression of the convention. Either a string literal or an
    /// identifier; both are emitted into `Identifier::new_unchecked(...)`.
    pub name: syn::Expr,
    /// A Rust expression evaluating to a `String` (the serialized data).
    pub data: syn::Expr,
}

/// Parse `#[quent(...)]` attributes attached to the type into:
/// - An `Option<Fsm>` for the optional `fsm(...)` arm (at most one).
/// - A `Vec<ConventionArm>` collected from any number of `convention(name, data)` arms.
pub fn parse(quent_attrs: &[&syn::Attribute]) -> syn::Result<(Option<Fsm>, Vec<ConventionArm>)> {
    let mut fsm_opt: Option<Fsm> = None;
    let mut conventions: Vec<ConventionArm> = Vec::new();

    for attr in quent_attrs {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("fsm") {
                if fsm_opt.is_some() {
                    return Err(meta.error("duplicate fsm(...) arm in #[quent(...)]"));
                }
                let content;
                syn::parenthesized!(content in meta.input);
                fsm_opt = Some(fsm::parse(&content)?);
            } else if meta.path.is_ident("convention") {
                let content;
                syn::parenthesized!(content in meta.input);
                let name: syn::Expr = content.parse()?;
                let _comma: Token![,] = content.parse()?;
                let data: syn::Expr = content.parse()?;
                conventions.push(ConventionArm { name, data });
            } else {
                let key = meta
                    .path
                    .get_ident()
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "<non-ident>".into());
                return Err(meta.error(format!("unknown #[quent(...)] argument: {key}")));
            }
            Ok(())
        })?;
    }

    Ok((fsm_opt, conventions))
}
