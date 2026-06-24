// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

pub mod demo {
    include!(concat!(env!("OUT_DIR"), "/demo.rs"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = demo::DemoContext::try_new(None)?;
    let observer = context.connection_observer();
    let mut conn = observer.handle();

    // The handle (may) hold per-instance state that enforces once-cardinality,
    // hence it is mut so it can update it state after producing a once-event.
    conn.opened(
        demo::Endpoint {
            host: "localhost".to_owned(),
            port: 8080,
        },
        uuid::Uuid::nil(),
    )?;
    conn.data(1234, None)?;
    conn.data(5678, None)?;
    conn.closed()?;

    // Emitting a once-event a second time fails.
    assert!(conn.closed().is_err());

    Ok(())
}
