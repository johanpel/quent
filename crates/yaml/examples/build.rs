// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use quent_instrumentation_build::{Options, generate};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    for relative_path in [
        "01-minimal-model/model.yaml",
        "02-event-data/model.yaml",
        "03-repeated-events/model.yaml",
        "04-records/model.yaml",
        "05-entity-references/model.yaml",
        "06-scoped-references/model.yaml",
        "07-finite-state-machine/model.yaml",
        "08-fsm-self-loop/model.yaml",
        "09-unit-resource/model.yaml",
        "10-resource-capacity/model.yaml",
        "11-bounded-resource/model.yaml",
        "12-job-workload/model.yaml",
    ] {
        let model = root.join(relative_path);
        println!("cargo:rerun-if-changed={}", model.display());

        let parsed = quent_yaml::parse_from_file(&model)?;
        for warning in parsed.warnings {
            println!("cargo:warning={warning}");
        }

        let generated = generate(&parsed.schema, &Options::default())?;
        for warning in generated.warnings {
            println!("cargo:warning={warning}");
        }
    }

    Ok(())
}
