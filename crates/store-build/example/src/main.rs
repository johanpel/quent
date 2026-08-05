// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Loads an existing filesystem-exported context matching the shared example schema.

use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use demo::{Demo, Query, Uuid};
use quent_store::event::{EntityEventStore, ModelEventStore};
use quent_store::event::filesystem::Store;

#[allow(unused)]
mod demo {
    include!(concat!(env!("OUT_DIR"), "/demo.rs"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = PathBuf::from(args.next().ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidInput,
            "usage: quent-store-build-example <event-root> <context-id>",
        )
    })?);
    let context_id = args
        .next()
        .and_then(|value| value.into_string().ok())
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "missing context ID"))?
        .parse::<Uuid>()?;

    let store = Store::<Demo>::new(root);

    // Load events for one entity type.
    for event in store.entity_events::<Query>(context_id)? {
        println!("{event:?}");
    }

    // Load all model events as `DemoEvent`.
    for event in store.events(context_id)? {
        println!("{event:?}");
    }

    Ok(())
}
