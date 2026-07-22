// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Batched Bitcode filesystem exporter and importer.
//!
//! Each frame is `[4-byte big-endian payload length][Bitcode Vec<Event<T>>]`.
//! Encoding whole batches lets Bitcode apply its column-oriented encoding across
//! multiple events while the importer still exposes the normal event iterator.

use std::{
    collections::VecDeque, io::BufReader, marker::PhantomData, num::NonZeroUsize, path::PathBuf,
};

use quent_events::{EntityEvent, Event};
use quent_io_types::{
    Exporter, ExporterError, ExporterProvider, ExporterResult, Importer, ImporterResult,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncWriteExt, BufWriter},
};
use tracing::{debug, error, warn};
use uuid::Uuid;

const EXTENSION: &str = "bitcode";
const TARGET_BATCH_SIZE: usize = 4096;
const DEFAULT_PARALLEL_CHUNKS: usize = 8;
const DEFAULT_IN_FLIGHT_BATCHES: usize = 8;

thread_local! {
    static NATIVE_CODEC: std::cell::RefCell<bitcode::Buffer> =
        std::cell::RefCell::new(bitcode::Buffer::new());
}

#[derive(Debug, Clone)]
pub struct BitcodeExporterOptions {
    pub dir: PathBuf,
}

#[derive(Debug)]
pub struct BitcodeExporter<T> {
    writer: Option<BufWriter<File>>,
    batches: Vec<Vec<u8>>,
    pending: Vec<Event<T>>,
    target_batch_size: usize,
}

impl<T: EntityEvent> BitcodeExporter<T> {
    pub async fn try_new(options: BitcodeExporterOptions) -> ExporterResult<Self> {
        let dir = options.dir.join(T::NAME);
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{}.{EXTENSION}", Uuid::now_v7()));
        debug!("exporting to \"{}\"", path.display());
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        let target_batch_size = std::env::var("QUENT_BITCODE_BATCH_SIZE")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|&size| size > 0)
            .unwrap_or(TARGET_BATCH_SIZE);
        Ok(Self {
            writer: Some(BufWriter::new(file)),
            batches: Vec::new(),
            pending: Vec::new(),
            target_batch_size,
        })
    }
}

async fn write_frame(writer: &mut BufWriter<File>, payload: &[u8]) -> ExporterResult<()> {
    let len = u32::try_from(payload.len()).map_err(|_| {
        ExporterError::other(std::io::Error::other(
            "Bitcode frame exceeds u32::MAX bytes",
        ))
    })?;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(payload).await?;
    Ok(())
}

#[async_trait::async_trait]
impl<T> Exporter<T> for BitcodeExporter<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    fn batch_size_hint(&self) -> NonZeroUsize {
        NonZeroUsize::new(self.target_batch_size).unwrap()
    }

    async fn push(&mut self, event: Event<T>) -> ExporterResult<()> {
        if self.writer.is_none() {
            return Err(ExporterError::Shutdown);
        }
        self.pending.push(event);
        if self.pending.len() >= self.target_batch_size {
            self.flush_pending().await?;
        }
        Ok(())
    }

    async fn drain_events(&mut self, events: &mut Vec<Event<T>>) -> ExporterResult<()> {
        if self.writer.is_none() {
            events.clear();
            return Err(ExporterError::Shutdown);
        }
        self.pending.append(events);
        if self.pending.len() >= self.target_batch_size {
            self.flush_pending().await?;
        }
        Ok(())
    }

    async fn shutdown(mut self: Box<Self>) -> ExporterResult<()> {
        self.flush_pending().await?;
        let Some(mut writer) = self.writer.take() else {
            return Ok(());
        };
        writer.flush().await?;
        Ok(())
    }
}

impl<T> BitcodeExporter<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    async fn flush_pending(&mut self) -> ExporterResult<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let Self {
            writer,
            batches,
            pending,
            target_batch_size: _,
        } = self;
        let Some(writer) = writer.as_mut() else {
            pending.clear();
            return Err(ExporterError::Shutdown);
        };
        let event_count = pending.len();
        let max_parallel_chunks = std::env::var("QUENT_BITCODE_PARALLEL_CHUNKS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|&chunks| chunks > 0)
            .unwrap_or(DEFAULT_PARALLEL_CHUNKS);
        let mut worker_events = std::mem::take(pending);
        let mut worker_batches = std::mem::take(batches);
        let (encoded_sender, encoded_receiver) = tokio::sync::oneshot::channel();
        let (dropped_sender, dropped_receiver) = tokio::sync::oneshot::channel();
        rayon::spawn_fifo(move || {
            let workers = rayon::current_num_threads()
                .min(max_parallel_chunks)
                .min(event_count);
            let chunk_len = event_count.div_ceil(workers);
            let chunk_count = event_count.div_ceil(chunk_len);
            worker_batches.resize_with(chunk_count, Vec::new);
            worker_events
                .par_chunks_mut(chunk_len)
                .zip(worker_batches[..chunk_count].par_iter_mut())
                .for_each(|(events, payload)| match bitcode::serialize(events) {
                    Ok(encoded) => *payload = encoded,
                    Err(e) => {
                        payload.clear();
                        warn!("unable to serialize Bitcode batch: {e}");
                    }
                });
            let _ = encoded_sender.send(worker_batches);
            worker_events
                .par_drain(..)
                .with_min_len(chunk_len)
                .for_each(drop);
            let _ = dropped_sender.send(worker_events);
        });
        *batches = encoded_receiver.await.map_err(ExporterError::other)?;
        let write_result = async {
            for payload in batches.iter().filter(|payload| !payload.is_empty()) {
                write_frame(writer, payload).await?;
            }
            Ok(())
        }
        .await;
        *pending = dropped_receiver.await.map_err(ExporterError::other)?;
        write_result
    }
}

/// Native Bitcode provider for event types deriving [`bitcode::Encode`] and
/// [`bitcode::Decode`]. Unlike the Serde-compatible exporter, this path reuses
/// Bitcode's typed encoder state between batches.
#[derive(Debug, Clone)]
pub struct NativeBitcodeExporterOptions {
    pub root: PathBuf,
}

#[async_trait::async_trait]
impl<T> ExporterProvider<T> for NativeBitcodeExporterOptions
where
    T: bitcode::Encode + Send + EntityEvent + 'static,
{
    async fn create_exporter(&self, context_id: Uuid) -> ExporterResult<Box<dyn Exporter<T>>> {
        Ok(Box::new(
            NativeBitcodeExporter::<T>::try_new(BitcodeExporterOptions {
                dir: self.root.join(context_id.to_string()),
            })
            .await?,
        ))
    }
}

struct NativeBitcodeExporter<T> {
    writer: Option<BufWriter<File>>,
    in_flight: VecDeque<tokio::sync::oneshot::Receiver<Vec<Vec<u8>>>>,
    payload_pool: std::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    pending: Vec<Event<T>>,
    target_batch_size: usize,
    max_in_flight: usize,
}

impl<T: EntityEvent> NativeBitcodeExporter<T> {
    async fn try_new(options: BitcodeExporterOptions) -> ExporterResult<Self> {
        let dir = options.dir.join(T::NAME);
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{}.native.{EXTENSION}", Uuid::now_v7()));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        let target_batch_size = std::env::var("QUENT_BITCODE_BATCH_SIZE")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|&size| size > 0)
            .unwrap_or(TARGET_BATCH_SIZE);
        let max_in_flight = std::env::var("QUENT_BITCODE_IN_FLIGHT_BATCHES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|&count| count > 0)
            .unwrap_or(DEFAULT_IN_FLIGHT_BATCHES);
        Ok(Self {
            writer: Some(BufWriter::new(file)),
            in_flight: VecDeque::new(),
            payload_pool: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            pending: Vec::new(),
            target_batch_size,
            max_in_flight,
        })
    }
}

#[async_trait::async_trait]
impl<T> Exporter<T> for NativeBitcodeExporter<T>
where
    T: bitcode::Encode + Send + EntityEvent + 'static,
{
    fn batch_size_hint(&self) -> NonZeroUsize {
        NonZeroUsize::new(self.target_batch_size).unwrap()
    }

    async fn push(&mut self, event: Event<T>) -> ExporterResult<()> {
        if self.writer.is_none() {
            return Err(ExporterError::Shutdown);
        }
        self.pending.push(event);
        if self.pending.len() >= self.target_batch_size {
            self.flush_pending().await?;
        }
        Ok(())
    }

    async fn drain_events(&mut self, events: &mut Vec<Event<T>>) -> ExporterResult<()> {
        if self.writer.is_none() {
            events.clear();
            return Err(ExporterError::Shutdown);
        }
        self.pending.append(events);
        if self.pending.len() >= self.target_batch_size {
            self.flush_pending().await?;
        }
        Ok(())
    }

    async fn shutdown(mut self: Box<Self>) -> ExporterResult<()> {
        self.flush_pending().await?;
        while !self.in_flight.is_empty() {
            self.write_next().await?;
        }
        let Some(mut writer) = self.writer.take() else {
            return Ok(());
        };
        writer.flush().await?;
        Ok(())
    }
}

impl<T> NativeBitcodeExporter<T>
where
    T: bitcode::Encode + Send + EntityEvent + 'static,
{
    async fn flush_pending(&mut self) -> ExporterResult<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        if self.writer.is_none() {
            self.pending.clear();
            return Err(ExporterError::Shutdown);
        }
        let event_count = self.pending.len();
        let max_parallel_chunks = std::env::var("QUENT_BITCODE_PARALLEL_CHUNKS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|&chunks| chunks > 0)
            .unwrap_or(DEFAULT_PARALLEL_CHUNKS);
        let workers = rayon::current_num_threads()
            .min(max_parallel_chunks)
            .min(event_count);
        let chunk_len = event_count.div_ceil(workers);
        let mut events = std::mem::take(&mut self.pending);
        let payload_pool = self.payload_pool.clone();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        rayon::spawn_fifo(move || {
            let payloads = events
                .par_chunks_mut(chunk_len)
                .map(|chunk| {
                    NATIVE_CODEC.with(|codec| {
                        let mut codec = codec.borrow_mut();
                        let encoded = codec.encode(chunk);
                        let mut payload = payload_pool.lock().unwrap().pop().unwrap_or_default();
                        payload.clear();
                        payload.extend_from_slice(encoded);
                        payload
                    })
                })
                .collect();
            events.par_drain(..).for_each(drop);
            let _ = sender.send(payloads);
        });
        self.in_flight.push_back(receiver);
        if self.in_flight.len() >= self.max_in_flight {
            self.write_next().await?;
        }
        Ok(())
    }

    async fn write_next(&mut self) -> ExporterResult<()> {
        let receiver = self.in_flight.pop_front().ok_or_else(|| {
            ExporterError::other(std::io::Error::other("no Bitcode batch is in flight"))
        })?;
        let mut payloads = receiver.await.map_err(ExporterError::other)?;
        let writer = self.writer.as_mut().ok_or(ExporterError::Shutdown)?;
        for payload in &payloads {
            write_frame(writer, payload).await?;
        }
        self.payload_pool.lock().unwrap().append(&mut payloads);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct BitcodeImporterOptions {
    pub path: PathBuf,
}

pub struct BitcodeImporter<T> {
    reader: BufReader<std::fs::File>,
    pending: VecDeque<Event<T>>,
    _phantom: PhantomData<T>,
}

impl<T> BitcodeImporter<T> {
    pub fn try_new(options: &BitcodeImporterOptions) -> ImporterResult<Self> {
        let path = quent_io_types::resolve_import_path(&options.path, EXTENSION)?;
        Ok(Self {
            reader: BufReader::new(std::fs::File::open(path)?),
            pending: VecDeque::new(),
            _phantom: PhantomData,
        })
    }
}

impl<T> Importer<T> for BitcodeImporter<T> where T: for<'de> Deserialize<'de> {}

impl<T> Iterator for BitcodeImporter<T>
where
    T: for<'de> Deserialize<'de>,
{
    type Item = Event<T>;

    fn next(&mut self) -> Option<Self::Item> {
        use std::io::Read;
        if let Some(event) = self.pending.pop_front() {
            return Some(event);
        }
        let mut len_buf = [0u8; 4];
        match self.reader.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return None,
            Err(e) => {
                error!("failed to read Bitcode length: {e}");
                return None;
            }
        }
        let mut payload = vec![0u8; u32::from_be_bytes(len_buf) as usize];
        if let Err(e) = self.reader.read_exact(&mut payload) {
            error!("failed to read Bitcode payload: {e}");
            return None;
        }
        match bitcode::deserialize::<Vec<Event<T>>>(&payload) {
            Ok(events) => self.pending.extend(events),
            Err(e) => {
                error!("failed to deserialize Bitcode batch: {e}");
                return None;
            }
        }
        self.pending.pop_front()
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
        const NAME: &'static str = "bitcode_test";
    }

    #[tokio::test]
    async fn parallel_batches_roundtrip_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut exporter = BitcodeExporter::<TestEvent>::try_new(BitcodeExporterOptions {
            dir: dir.path().to_owned(),
        })
        .await
        .unwrap();
        let id = Uuid::nil();
        let mut expected = Vec::new();
        for size in [256, 256, TARGET_BATCH_SIZE + 17] {
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
        <BitcodeExporter<TestEvent> as Exporter<TestEvent>>::shutdown(Box::new(exporter))
            .await
            .unwrap();

        let path = dir.path().join(TestEvent::NAME);
        let actual = BitcodeImporter::<TestEvent>::try_new(&BitcodeImporterOptions { path })
            .unwrap()
            .map(|event| event.data.sequence)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
