// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model_ir::qualifications::Qualification;

mod fsm;

pub fn parse(quent_attrs: &[&syn::Attribute]) -> syn::Result<Vec<Qualification>> {
    let mut result: Vec<Qualification> = Vec::new();

    for attr in quent_attrs {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("fsm") {
                let content;
                syn::parenthesized!(content in meta.input);
                let fsm = fsm::parse(&content)?;
                result.push(Qualification::Fsm(fsm));
            } else if meta.path.is_ident("resource") {
                todo!()
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

    Ok(result)
}
