// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Exporter dumping events as newline-delimited JSON objects into a file.
use std::{
    io::{BufRead, BufReader},
    marker::PhantomData,
    num::NonZeroUsize,
    path::PathBuf,
    time::{Duration, Instant},
};

use quent_events::{EntityEvent, Event};
use quent_io_types::{Exporter, ExporterError, ExporterResult, Importer, ImporterResult};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncWriteExt, BufWriter},
};
use tracing::{debug, error, warn};
use uuid::Uuid;

/// File extension for ndjson event files.
const EXTENSION: &str = "ndjson";

/// Options for the ndjson exporter.
///
/// A human-readable format useful for debugging and manual / LLM-based
/// inspection.
///
/// Writes events as newline-delimited JSON (one JSON object per line) under
/// `dir`, in a per-entity subdirectory holding a UUIDv7-named `.ndjson` file.
#[derive(Debug, Clone)]
pub struct NdjsonExporterOptions {
    pub dir: PathBuf,
}

#[derive(Debug)]
pub struct NdjsonExporter {
    /// `None` once [`shutdown`](Exporter::shutdown) has flushed and released it.
    writer: Option<BufWriter<File>>,
    /// Line buffer reused across [`drain_events`](Exporter::drain_events).
    batch: Vec<u8>,
    chunks: Vec<Vec<u8>>,
    parallel: bool,
    serial_batches: u8,
}

impl NdjsonExporter {
    pub async fn try_new<T: EntityEvent>(options: NdjsonExporterOptions) -> ExporterResult<Self> {
        let dir = options.dir.join(T::NAME);
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{}.{EXTENSION}", Uuid::now_v7()));
        debug!("exporting to \"{}\"", path.display());
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;

        Ok(Self {
            writer: Some(BufWriter::new(file)),
            batch: Vec::new(),
            chunks: Vec::new(),
            parallel: false,
            serial_batches: 0,
        })
    }
}

#[async_trait::async_trait]
impl<T> Exporter<T> for NdjsonExporter
where
    T: Serialize + Send + EntityEvent + 'static,
{
    fn batch_size_hint(&self) -> NonZeroUsize {
        NonZeroUsize::new(if self.parallel { 1024 } else { 256 }).unwrap()
    }

    async fn push(&mut self, event: Event<T>) -> ExporterResult<()> {
        let Self { writer, batch, .. } = self;
        let writer = writer.as_mut().ok_or(ExporterError::Shutdown)?;
        batch.clear();
        serde_json::to_writer(&mut *batch, &event).map_err(ExporterError::other)?;
        batch.push(b'\n');
        writer.write_all(batch).await?;
        Ok(())
    }

    async fn drain_events(&mut self, events: &mut Vec<Event<T>>) -> ExporterResult<()> {
        let Self {
            writer,
            batch,
            chunks,
            parallel,
            serial_batches,
        } = self;
        let Some(writer) = writer.as_mut() else {
            events.clear();
            return Err(ExporterError::Shutdown);
        };
        let event_count = events.len();
        if *parallel && event_count > 1 {
            let mut worker_events = std::mem::take(events);
            let mut worker_batch = std::mem::take(batch);
            let mut worker_chunks = std::mem::take(chunks);
            let (sender, receiver) = tokio::sync::oneshot::channel();
            rayon::spawn_fifo(move || {
                let workers = rayon::current_num_threads().min(event_count);
                let chunk_len = event_count.div_ceil(workers);
                let chunk_count = event_count.div_ceil(chunk_len);
                worker_chunks.resize_with(chunk_count, Vec::new);
                worker_events
                    .par_chunks_mut(chunk_len)
                    .zip(worker_chunks[..chunk_count].par_iter_mut())
                    .for_each(|(events, buffer)| {
                        buffer.clear();
                        for event in events {
                            let start = buffer.len();
                            if let Err(e) = serde_json::to_writer(&mut *buffer, event) {
                                buffer.truncate(start);
                                warn!("unable to serialize event: {e}");
                                continue;
                            }
                            buffer.push(b'\n');
                        }
                    });
                worker_batch.clear();
                for chunk in &worker_chunks[..chunk_count] {
                    worker_batch.extend_from_slice(chunk);
                }
                worker_events.clear();
                let _ = sender.send((worker_events, worker_batch, worker_chunks));
            });
            let (worker_events, worker_batch, worker_chunks) =
                receiver.await.map_err(ExporterError::other)?;
            *events = worker_events;
            *batch = worker_batch;
            *chunks = worker_chunks;
        } else {
            let started = Instant::now();
            batch.clear();
            for event in events.drain(..) {
                let start = batch.len();
                if let Err(e) = serde_json::to_writer(&mut *batch, &event) {
                    batch.truncate(start);
                    warn!("unable to serialize event: {e}");
                    continue;
                }
                batch.push(b'\n');
            }
            *serial_batches = serial_batches.saturating_add(1);
            let elapsed = started.elapsed();
            // Ignore allocation warm-up and require enough work to amortize the handoff.
            *parallel =
                *serial_batches >= 2 && event_count >= 128 && elapsed >= Duration::from_micros(250);
        }
        writer.write_all(batch).await?;
        Ok(())
    }

    async fn shutdown(mut self: Box<Self>) -> ExporterResult<()> {
        let Some(mut writer) = self.writer.take() else {
            return Ok(());
        };
        writer.flush().await?;
        Ok(())
    }
}

/// Options for the ndjson importer. `path` is either the directory containing
/// the event file (located by its `.ndjson` extension) or the file itself.
#[derive(Debug, Clone)]
pub struct NdjsonImporterOptions {
    pub path: PathBuf,
}

pub struct NdjsonImporter<T> {
    reader: BufReader<std::fs::File>,
    _phantom: PhantomData<T>,
}

impl<T> NdjsonImporter<T> {
    pub fn try_new(options: &NdjsonImporterOptions) -> ImporterResult<Self> {
        let path = quent_io_types::resolve_import_path(&options.path, "ndjson")?;
        let file = std::fs::File::open(&path)?;
        Ok(Self {
            reader: BufReader::new(file),
            _phantom: Default::default(),
        })
    }
}

impl<T> Importer<T> for NdjsonImporter<T> where T: for<'de> Deserialize<'de> {}

impl<T> Iterator for NdjsonImporter<T>
where
    T: for<'de> Deserialize<'de>,
{
    type Item = Event<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => {
                let trimmed = line.trim_end();
                match serde_json::from_str::<Event<T>>(trimmed) {
                    Ok(event) => Some(event),
                    Err(e) => {
                        error!("failed to parse ndjson line: {e}");
                        None
                    }
                }
            }
            Err(e) => {
                error!("failed to read ndjson: {e}");
                None
            }
        }
    }
}
