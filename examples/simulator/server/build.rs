// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

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

const TS_OUT_DIR: &str = "./ts-bindings/";

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

        // Preserve mtimes so generated output does not invalidate the next build.
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={TS_OUT_DIR}");

    let generated_dir = PathBuf::from(std::env::var("OUT_DIR")?).join("ts-bindings");
    if generated_dir.exists() {
        std::fs::remove_dir_all(&generated_dir)?;
    }
    std::fs::create_dir_all(&generated_dir)?;
    let cfg = Config::new().with_out_dir(&generated_dir);

    <QueryBundle<EntityRef> as TS>::export_all(&cfg)?;

    <SingleTimelineRequest<QueryFilter, OperatorFilter> as TS>::export_all(&cfg)?;
    <SingleTimelineResponse as TS>::export_all(&cfg)?;
    <BulkTimelineRequest<QueryFilter, OperatorFilter> as TS>::export_all(&cfg)?;
    <BulkTimelinesResponse as TS>::export_all(&cfg)?;
    <CategoricalTimelineRequest<QueryFilter> as TS>::export_all(&cfg)?;
    <DataFlowTimelineBinned as TS>::export_all(&cfg)?;

    <EntityListRequest<QueryFilter, OperatorFilter> as TS>::export_all(&cfg)?;
    <EntityListResponse as TS>::export_all(&cfg)?;

    sync_bindings(&generated_dir, Path::new(TS_OUT_DIR))?;

    Ok(())
}
