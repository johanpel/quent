// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_instrumentation::{ExporterOptions, FileSystemExporterOptions, FileSystemFormat};

use crate::demo::{ConnectionHandle, ConnectionObserver, DemoContext, Uuid};

#[allow(unused)]
mod demo {
    include!(concat!(env!("OUT_DIR"), "/demo.rs"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = tempfile::tempdir()?;
    let exporter = ExporterOptions::FileSystem(FileSystemExporterOptions::new(
        FileSystemFormat::Parquet,
        output.path().to_path_buf(),
    ));
    let context: DemoContext = demo::DemoContext::try_new(Some(exporter))?;
    let context_id = context.id();
    let observer: ConnectionObserver = context.connection_observer();

    // The handle (may) hold per-instance state that enforces once-cardinality,
    // hence it is mut so it can update its state after producing a once-event.
    let mut conn: ConnectionHandle = observer.handle();

    conn.opened(
        demo::Endpoint {
            host: "localhost".to_owned(),
            port: 8080,
        },
        demo::EntityRef {
            target: Uuid::nil(),
            data: 42,
        },
        demo::EntityRef {
            target: Uuid::nil(),
            data: (),
        },
    )?;
    conn.data(1234, None)?;
    conn.data(5678, None)?;
    conn.closed()?;

    // Emitting a once-event a second time fails.
    assert!(conn.closed_emitted());
    assert!(conn.closed().is_err());

    drop(conn);
    drop(observer);
    drop(context);

    let files = std::fs::read_dir(
        output
            .path()
            .join(context_id.to_string())
            .join("connection"),
    )?
    .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(files.len(), 1);
    assert_eq!(
        files[0]
            .path()
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("parquet")
    );

    Ok(())
}
