// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_store_build::{EntityStoreImplementations, Options, generate};

mod schema;

fn main() {
    println!("cargo:rerun-if-changed=schema.rs");
    for (file_name, native) in [("model.rs", true), ("events_only.rs", false)] {
        generate(
            &schema::schema(),
            &Options {
                entity_store: EntityStoreImplementations { native },
                debug: native,
                combined_event: false,
                filesystem: false,
                file_name: Some(file_name.to_owned()),
                ..Options::default()
            },
        )
        .unwrap();
    }
}
