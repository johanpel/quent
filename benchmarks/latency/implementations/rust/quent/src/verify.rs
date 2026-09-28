// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use quent_instrumentation::Uuid;

use quent_latency_common::BatchConfig;

use crate::{BenchResult, Exporter};

pub fn exported_events(
    exporter: Exporter,
    export_root: Option<&Path>,
    context_id: Uuid,
    threads: usize,
    config: BatchConfig,
    preflight_call: bool,
) -> BenchResult<()> {
    let (extension, count_file): (&str, fn(File) -> BenchResult<u64>) = match exporter {
        Exporter::Noop => return Ok(()),
        Exporter::Ndjson => ("ndjson", count_ndjson_records),
        Exporter::Postcard => ("postcard", count_postcard_records),
    };
    let expected_per_thread = u64::try_from(config.num_warmup_batches + config.num_batches)?
        .checked_mul(config.batch_size)
        .ok_or("expected exported event count overflows u64")?
        .checked_add(u64::from(preflight_call))
        .ok_or("expected exported event count overflows u64")?;
    let expected = u64::try_from(threads)?
        .checked_mul(expected_per_thread)
        .ok_or("expected exported event count overflows u64")?;
    let root = export_root.ok_or("filesystem export root is missing")?;
    let actual = count_records(&root.join(context_id.to_string()), extension, count_file)?;
    if actual != expected {
        return Err(format!("{extension} event count: expected {expected}, found {actual}").into());
    }
    Ok(())
}

fn count_records(
    root: &Path,
    extension: &str,
    count_file: fn(File) -> BenchResult<u64>,
) -> BenchResult<u64> {
    let mut count = 0;
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|value| value == extension) {
                count += count_file(File::open(path)?)?;
            }
        }
    }
    Ok(count)
}

fn count_ndjson_records(file: File) -> BenchResult<u64> {
    let mut count = 0;
    for line in BufReader::new(file).lines() {
        line?;
        count += 1;
    }
    Ok(count)
}

fn count_postcard_records(file: File) -> BenchResult<u64> {
    let mut reader = BufReader::new(file);
    let mut count = 0;
    loop {
        let mut length = [0; 4];
        if reader.read(&mut length[..1])? == 0 {
            break;
        }
        reader.read_exact(&mut length[1..])?;
        let length = u32::from_be_bytes(length) as u64;
        if std::io::copy(&mut reader.by_ref().take(length), &mut std::io::sink())? != length {
            return Err("truncated Postcard record".into());
        }
        count += 1;
    }
    Ok(count)
}
