// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Built-in constraint-backed YAML features.

use quent_constraints::{Constraint, validate};
use quent_fsm::FsmConstraint;
use quent_ref_target::RefTargetConstraint;
use quent_ref_tree::RefTreeConstraint;
use quent_resource::{Resource, ResourceConstraint};
use quent_schema::Schema;

use crate::diag::{Diagnostic, Diagnostics};

pub(crate) mod fsm;
pub(crate) mod reference;
pub(crate) mod resource;

pub(crate) fn direct_declaration_error(name: &str) -> Option<&'static str> {
    if name == FsmConstraint::NAME {
        Some("the FSM constraint is set from an `fsms:` block, not written directly")
    } else if name == Resource::NAME {
        Some("the resource constraint is set from a `resource:` block, not written directly")
    } else {
        None
    }
}

pub(crate) fn validate_schema(schema: &Schema, sink: &mut Diagnostics) -> Option<Vec<Diagnostic>> {
    let report = validate::<(
        RefTargetConstraint,
        RefTreeConstraint,
        FsmConstraint,
        ResourceConstraint,
    )>(schema);
    if let Err(error) = report.base_constraints {
        for entity in error.entities_without_events {
            sink.error(
                &format!("entities.{entity}"),
                format!("entity `{entity}` declares no events"),
                Some("entities must declare at least one event".to_string()),
            );
        }
        for record in error.recursive_records {
            sink.error(
                &format!("records.{record}"),
                format!("record `{record}` is recursive"),
                Some(
                    "records cannot contain themselves, directly or through other records"
                        .to_string(),
                ),
            );
        }
        for reference in error.invalid_references {
            sink.error("", format!("unresolved reference: {reference}"), None);
        }
    }

    let (ref_target, ref_tree, fsm, resource) = report.results;
    for result in [
        ref_target.map_err(|error| error.to_string()),
        ref_tree.map_err(|error| error.to_string()),
        fsm.map_err(|error| error.to_string()),
        resource.map_err(|error| error.to_string()),
    ] {
        if let Err(error) = result {
            sink.error("", error, None);
        }
    }
    if sink.has_errors() {
        return None;
    }

    Some(
        report
            .unregistered_constraints
            .into_iter()
            .map(|name| {
                sink.make(
                    "",
                    format!("constraint `{name}` has no registered validator"),
                    Some(
                        "it is passed through untouched; a downstream validator may check it"
                            .to_string(),
                    ),
                )
            })
            .collect(),
    )
}
