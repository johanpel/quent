// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Log-sink forms and their schema elaboration.

use indexmap::IndexMap;
use quent_log::{LevelDecl, LogEntityBuilder};
use quent_schema::Entity;
use quent_schema::builder::AnnotationsBuilder;
use serde::Deserialize;

use crate::ast::{self, AnnotationMap};
use crate::diag::Diagnostics;
use crate::extensions::{Elaborator, EventContext};
use crate::lower::{
    annotations_builder, build_or_diagnose, event_fields, event_of, ident, type_decl_ident,
};

/// A log sink: common attributes and ordered levels.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LogSpec {
    // Fields shared with core entities.
    #[serde(default)]
    doc: Option<String>,
    #[serde(default)]
    constraints: AnnotationMap,
    #[serde(default)]
    metadata: AnnotationMap,
    #[serde(default)]
    events: IndexMap<String, ast::Event>,

    // Log extension fields.
    #[serde(default)]
    attributes: IndexMap<String, ast::Field>,
    levels: Vec<LogLevel>,
}

/// One log level and its event-specific additions.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LogLevel {
    name: String,
    #[serde(default)]
    doc: Option<String>,
    #[serde(default)]
    attributes: IndexMap<String, ast::Field>,
}

/// Elaborate a `logs:` declaration into one repeatable event per declared level.
pub(crate) fn elaborate(
    name: &str,
    spec: &LogSpec,
    extensions: &Elaborator,
    sink: &mut Diagnostics,
) -> Option<Entity> {
    let log_path = format!("logs.{name}");
    let id = type_decl_ident(name, "logs", sink);
    let annotations = annotations_builder(
        &spec.doc,
        &spec.constraints,
        &spec.metadata,
        extensions,
        &log_path,
        sink,
    );
    let annotations = build_or_diagnose(annotations.build(), &log_path, sink).unwrap_or_default();
    let event_context = EventContext::default();
    let events = spec
        .events
        .iter()
        .filter_map(|(event_name, event)| {
            event_of(
                event_name,
                event,
                &log_path,
                &event_context,
                extensions,
                sink,
            )
        })
        .collect::<Vec<_>>();
    let common = event_fields(
        &spec.attributes,
        &format!("{log_path}.attributes"),
        &event_context,
        extensions,
        sink,
    );

    let mut levels = Vec::new();
    let mut complete = true;
    for (rank, level) in spec.levels.iter().enumerate() {
        let level_path = format!("{log_path}.levels.{rank}");
        let Some(name) = ident(&level.name, &format!("{level_path}.name"), sink) else {
            complete = false;
            continue;
        };
        let attributes = event_fields(
            &level.attributes,
            &format!("{level_path}.attributes"),
            &event_context,
            extensions,
            sink,
        );
        let event_annotations = match &level.doc {
            Some(doc) => AnnotationsBuilder::new().with_docs(doc).build(),
            None => AnnotationsBuilder::new().build(),
        };
        let Some(event_annotations) = build_or_diagnose(event_annotations, &level_path, sink)
        else {
            complete = false;
            continue;
        };
        levels.push(LevelDecl {
            name,
            annotations: event_annotations,
            attributes,
        });
    }
    if !complete {
        return None;
    }

    match LogEntityBuilder::new(id?)
        .with_annotations(annotations)
        .with_events(events)
        .with_attributes(common)
        .with_levels(levels)
        .build()
    {
        Ok(entity) => Some(entity),
        Err(error) => {
            sink.error(&log_path, error.to_string(), None);
            None
        }
    }
}
