// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Exporter dumping events as length-prefixed postcard records into a file.
//!
//! File format: sequence of length-prefixed records.
//! Each record: `[4 bytes: payload length as u32 BE][payload: postcard-encoded Event<T>]`
use std::{
    io::BufReader,
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

/// File extension for Postcard event files.
const EXTENSION: &str = "postcard";
const PARALLEL_BATCH_SIZE: usize = 8192;
const MAX_PARALLEL_CHUNKS: usize = 8;

/// Options for the Postcard exporter.
///
/// A compact row-oriented binary format, which is not self-describing.
///
/// Writes events in Postcard format under `dir`, in a per-entity subdirectory
/// holding a UUIDv7-named `.postcard` file.
#[derive(Debug, Clone)]
pub struct PostcardExporterOptions {
    pub dir: PathBuf,
}

#[derive(Debug)]
pub struct PostcardExporter {
    /// `None` once [`shutdown`](Exporter::shutdown) has flushed and released it.
    writer: Option<BufWriter<File>>,
    /// Framing buffer reused across [`drain_events`](Exporter::drain_events).
    batch: Vec<u8>,
    chunks: Vec<Vec<u8>>,
    parallel: bool,
    serial_batches: u8,
}

impl PostcardExporter {
    pub async fn try_new<T: EntityEvent>(options: PostcardExporterOptions) -> ExporterResult<Self> {
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

struct VecExtend<'a>(&'a mut Vec<u8>);

impl Extend<u8> for VecExtend<'_> {
    fn extend<I: IntoIterator<Item = u8>>(&mut self, iter: I) {
        self.0.extend(iter);
    }
}

fn encode_frame<T: Serialize>(buffer: &mut Vec<u8>, event: &T) -> postcard::Result<()> {
    let start = buffer.len();
    buffer.extend_from_slice(&[0; size_of::<u32>()]);
    if let Err(error) = postcard::to_extend(event, VecExtend(buffer)) {
        buffer.truncate(start);
        return Err(error);
    }
    let len = (buffer.len() - start - size_of::<u32>()) as u32;
    buffer[start..start + size_of::<u32>()].copy_from_slice(&len.to_be_bytes());
    Ok(())
}

#[async_trait::async_trait]
impl<T> Exporter<T> for PostcardExporter
where
    T: Serialize + Send + EntityEvent + 'static,
{
    fn batch_size_hint(&self) -> NonZeroUsize {
        NonZeroUsize::new(if self.parallel {
            PARALLEL_BATCH_SIZE
        } else {
            256
        })
        .unwrap()
    }

    async fn push(&mut self, event: Event<T>) -> ExporterResult<()> {
        let Self { writer, batch, .. } = self;
        let writer = writer.as_mut().ok_or(ExporterError::Shutdown)?;
        batch.clear();
        encode_frame(batch, &event).map_err(ExporterError::other)?;
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
        let mut write_chunks = false;
        let mut dropped_events = None;
        if *parallel && event_count > 1 {
            let mut worker_events = std::mem::take(events);
            let mut worker_batch = std::mem::take(batch);
            let mut worker_chunks = std::mem::take(chunks);
            let (encoded_sender, encoded_receiver) = tokio::sync::oneshot::channel();
            let (dropped_sender, dropped_receiver) = tokio::sync::oneshot::channel();
            rayon::spawn_fifo(move || {
                let workers = rayon::current_num_threads()
                    .min(MAX_PARALLEL_CHUNKS)
                    .min(event_count);
                let chunk_len = event_count.div_ceil(workers);
                let chunk_count = event_count.div_ceil(chunk_len);
                worker_chunks.resize_with(chunk_count, Vec::new);
                worker_events
                    .par_chunks_mut(chunk_len)
                    .zip(worker_chunks[..chunk_count].par_iter_mut())
                    .for_each(|(events, buffer)| {
                        buffer.clear();
                        for event in events {
                            if let Err(e) = encode_frame(buffer, event) {
                                warn!("unable to serialize event: {e}");
                            }
                        }
                    });
                worker_batch.clear();
                let _ = encoded_sender.send((worker_batch, worker_chunks));
                worker_events
                    .par_drain(..)
                    .with_min_len(chunk_len)
                    .for_each(drop);
                let _ = dropped_sender.send(worker_events);
            });
            let (worker_batch, worker_chunks) =
                encoded_receiver.await.map_err(ExporterError::other)?;
            *batch = worker_batch;
            *chunks = worker_chunks;
            write_chunks = true;
            dropped_events = Some(dropped_receiver);
        } else {
            let started = Instant::now();
            batch.clear();
            for event in events.drain(..) {
                if let Err(e) = encode_frame(batch, &event) {
                    warn!("unable to serialize event: {e}");
                }
            }
            *serial_batches = serial_batches.saturating_add(1);
            let elapsed = started.elapsed();
            // Ignore allocation warm-up and require enough work to amortize the handoff.
            *parallel =
                *serial_batches >= 2 && event_count >= 128 && elapsed >= Duration::from_micros(250);
        }
        let write_result = async {
            if write_chunks {
                for chunk in chunks.iter() {
                    writer.write_all(chunk).await?;
                }
            } else {
                writer.write_all(batch).await?;
            }
            Ok(())
        }
        .await;
        if let Some(receiver) = dropped_events {
            *events = receiver.await.map_err(ExporterError::other)?;
        }
        write_result
    }

    async fn shutdown(mut self: Box<Self>) -> ExporterResult<()> {
        let Some(mut writer) = self.writer.take() else {
            return Ok(());
        };
        writer.flush().await?;
        Ok(())
    }
}

/// Options for the Postcard importer. `path` is either the directory containing
/// the event file (located by its `.postcard` extension) or the file itself.
#[derive(Debug, Clone)]
pub struct PostcardImporterOptions {
    pub path: PathBuf,
}

pub struct PostcardImporter<T> {
    reader: BufReader<std::fs::File>,
    _phantom: PhantomData<T>,
}

impl<T> PostcardImporter<T> {
    pub fn try_new(options: &PostcardImporterOptions) -> ImporterResult<Self> {
        let path = quent_io_types::resolve_import_path(&options.path, "postcard")?;
        let file = std::fs::File::open(&path)?;
        Ok(Self {
            reader: BufReader::new(file),
            _phantom: Default::default(),
        })
    }
}

impl<T> Importer<T> for PostcardImporter<T> where T: for<'de> Deserialize<'de> {}

impl<T> Iterator for PostcardImporter<T>
where
    T: for<'de> Deserialize<'de>,
{
    type Item = Event<T>;

    fn next(&mut self) -> Option<Self::Item> {
        use std::io::Read;
        let mut len_buf = [0u8; 4];
        match self.reader.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return None,
            Err(e) => {
                error!("failed to read postcard length: {e}");
                return None;
            }
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        if let Err(e) = self.reader.read_exact(&mut payload) {
            error!("failed to read postcard payload: {e}");
            return None;
        }
        match postcard::from_bytes::<Event<T>>(&payload) {
            Ok(event) => Some(event),
            Err(e) => {
                error!("failed to deserialize postcard event: {e}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct TestEvent {
        sequence: u64,
        payload: String,
    }

    impl EntityEvent for TestEvent {
        const NAME: &'static str = "postcard_test";
    }

    #[tokio::test]
    async fn parallel_batches_roundtrip_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut exporter = PostcardExporter::try_new::<TestEvent>(PostcardExporterOptions {
            dir: dir.path().to_owned(),
        })
        .await
        .unwrap();
        let id = Uuid::nil();
        let mut expected = Vec::new();
        for size in [256, 256, PARALLEL_BATCH_SIZE + 17] {
            let mut events = (0..size)
                .map(|_| {
                    let sequence = expected.len() as u64;
                    expected.push(sequence);
                    Event::new(
                        id,
                        sequence,
                        TestEvent {
                            sequence,
                            payload: format!("payload-{sequence}"),
                        },
                    )
                })
                .collect();
            exporter.drain_events(&mut events).await.unwrap();
            assert!(events.is_empty());
        }
        <PostcardExporter as Exporter<TestEvent>>::shutdown(Box::new(exporter))
            .await
            .unwrap();

        let path = dir.path().join(TestEvent::NAME);
        let actual = PostcardImporter::<TestEvent>::try_new(&PostcardImporterOptions { path })
            .unwrap()
            .map(|event| event.data.sequence)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
