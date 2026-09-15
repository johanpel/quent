// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Constraint-backed entity-reference syntax and lowering.

use quent_constraints::Constraint;
use quent_ref_target::RefTargetConstraint;
use quent_ref_tree::RefTreeConstraint;
use quent_schema::DataType;
use quent_schema::builder::AnnotationsBuilder;
use serde::Deserialize;

use crate::ast::TypeExpr;
use crate::constraints::resource::Lowerer as ResourceLowerer;
use crate::diag::Diagnostics;
use crate::lower::{build_or_diagnose, type_of};

/// A targeted entity reference: `ref` names the entity it points at, with an
/// optional `data` type the reference carries.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RefForm {
    pub(crate) r#ref: String,
    #[serde(default)]
    pub(crate) data: Option<Box<TypeExpr>>,
}

/// A tree-forming targeted reference: `scope-ref` names the entity it points
/// at and marks the reference as part of the scoping tree, with an optional
/// `data` type the reference carries.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScopeForm {
    #[serde(rename = "scope-ref")]
    pub(crate) scope_ref: String,
    #[serde(default)]
    pub(crate) data: Option<Box<TypeExpr>>,
}

/// Lower a targeted entity reference, optionally carrying data and forming a tree.
pub(crate) fn lower(
    target: &str,
    data: Option<&TypeExpr>,
    tree: bool,
    path: &str,
    resources: &ResourceLowerer,
    sink: &mut Diagnostics,
) -> Option<DataType> {
    let data = match data {
        Some(expr) => Some(type_of(expr, path, resources, sink)?),
        None => None,
    };
    entity_ref_type(target, data, tree, path, sink)
}

pub(super) fn entity_ref_type(
    target: &str,
    data: Option<DataType>,
    tree: bool,
    path: &str,
    sink: &mut Diagnostics,
) -> Option<DataType> {
    let mut builder = AnnotationsBuilder::new()
        .with_constraint(RefTargetConstraint::NAME, Some(target.to_string()));
    if tree {
        builder = builder.with_constraint(RefTreeConstraint::NAME, None);
    }
    Some(DataType::EntityRef {
        data: data.map(Box::new),
        annotations: build_or_diagnose(builder.build(), path, sink).unwrap_or_default(),
    })
}
