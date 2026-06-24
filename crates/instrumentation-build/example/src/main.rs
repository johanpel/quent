// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

pub mod demo {
    include!(concat!(env!("OUT_DIR"), "/demo.rs"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The context builds one observer per entity. No exporter configured here,
    // so events go to a noop sink.
    let context = demo::DemoContext::try_new(None)?;
    let observer = context.connection_observer();
    let mut conn = observer.handle();

    // `opened` and `closed` are once-events (take `&mut`); `data` is multi.
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
