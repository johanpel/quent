// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! TypeScript binding generation for the simulator UI.

use std::collections::BTreeSet;
use std::path::Path;

use quent_query_engine_ui::DataFlowTimelineBinned;
use quent_query_engine_ui::{OperatorFilter, QueryBundle, QueryFilter};
use quent_simulator_ui::EntityRef;
use quent_ui::entities::{request::EntityListRequest, response::EntityListResponse};
use quent_ui::timeline::{
    categorical::CategoricalTimelineRequest,
    request::{BulkTimelineRequest, SingleTimelineRequest},
    response::{BulkTimelinesResponse, SingleTimelineResponse},
};
use ts_rs::{Config, TS};

fn sync_bindings(generated_dir: &Path, output_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(output_dir)?;
    let mut generated_files = BTreeSet::new();

    for entry in std::fs::read_dir(generated_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        generated_files.insert(file_name.clone());
        let generated = std::fs::read(entry.path())?;
        let destination = output_dir.join(file_name);

        // Avoid notifying TypeScript tooling when generated content is unchanged.
        if !std::fs::read(&destination).is_ok_and(|current| current == generated) {
            std::fs::write(destination, generated)?;
        }
    }

    for entry in std::fs::read_dir(output_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "ts")
            && !generated_files.contains(&entry.file_name())
        {
            std::fs::remove_file(entry.path())?;
        }
    }

    Ok(())
}

/// Generates simulator UI TypeScript bindings in `output_dir`.
///
/// Unchanged files are preserved, and stale TypeScript files are removed.
pub fn generate(output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let generated_dir = tempfile::tempdir()?;
    let cfg = Config::new().with_out_dir(generated_dir.path());

    <QueryBundle<EntityRef> as TS>::export_all(&cfg)?;

    <SingleTimelineRequest<QueryFilter, OperatorFilter> as TS>::export_all(&cfg)?;
    <SingleTimelineResponse as TS>::export_all(&cfg)?;
    <BulkTimelineRequest<QueryFilter, OperatorFilter> as TS>::export_all(&cfg)?;
    <BulkTimelinesResponse as TS>::export_all(&cfg)?;
    <CategoricalTimelineRequest<QueryFilter> as TS>::export_all(&cfg)?;
    <DataFlowTimelineBinned as TS>::export_all(&cfg)?;

    <EntityListRequest<QueryFilter, OperatorFilter> as TS>::export_all(&cfg)?;
    <EntityListResponse as TS>::export_all(&cfg)?;

    sync_bindings(generated_dir.path(), output_dir)?;

    Ok(())
}
