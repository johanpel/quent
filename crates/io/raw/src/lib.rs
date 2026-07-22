// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Throughput-oriented row-binary filesystem exporter and importer.
//!
//! The format deliberately does no compression. Each frame is a four-byte
//! big-endian byte length followed by a Bincode batch configured for fixed-width
//! little-endian integers. Strings and arrays retain their lengths and contents.

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

const EXTENSION: &str = "raw";
const BATCH_SIZE: usize = 4096;
const PARALLEL_CHUNKS: usize = 8;

enum SinkMessage {
    Data(Vec<u8>),
    Shutdown,
}

struct ThreadBatch {
    sender: std::sync::mpsc::SyncSender<SinkMessage>,
    bytes: Vec<u8>,
    events: usize,
}

impl ThreadBatch {
    fn flush(&mut self) {
        if self.events == 0 {
            return;
        }
        self.sender
            .send(SinkMessage::Data(std::mem::take(&mut self.bytes)))
            .expect("native raw writer stopped before its producer batch");
        self.events = 0;
    }
}

impl Drop for ThreadBatch {
    fn drop(&mut self) {
        self.flush();
    }
}

thread_local! {
    static PRODUCER_BATCHES: std::cell::RefCell<std::collections::HashMap<usize, ThreadBatch>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// A bounded, lossless producer-side native encoder with one background file
/// writer. Encoding and payload destruction happen on the calling thread; only
/// owned bytes cross the queue.
pub struct NativeRawSink<T> {
    sender: std::sync::mpsc::SyncSender<SinkMessage>,
    writer: std::sync::Mutex<Option<std::thread::JoinHandle<std::io::Result<()>>>>,
    _marker: PhantomData<fn(T)>,
}

/// Producer-batched raw sink for payloads that only implement Serde. This is
/// the opt-in dynamic-schema counterpart to [`NativeRawSink`].
pub struct SerdeRawSink<T> {
    sender: std::sync::mpsc::SyncSender<SinkMessage>,
    writer: std::sync::Mutex<Option<std::thread::JoinHandle<std::io::Result<()>>>>,
    _marker: PhantomData<fn(T)>,
}

impl<T> SerdeRawSink<T>
where
    T: Serialize + EntityEvent,
{
    pub fn try_new(root: &std::path::Path) -> std::io::Result<Self> {
        use std::io::Write;

        let dir = root.join(T::NAME);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.producer-serde.{EXTENSION}", Uuid::now_v7()));
        let file = std::fs::File::create(path)?;
        let (sender, receiver) = std::sync::mpsc::sync_channel(256);
        let writer = std::thread::spawn(move || {
            let mut writer = std::io::BufWriter::with_capacity(1024 * 1024, file);
            while let Ok(message) = receiver.recv() {
                match message {
                    SinkMessage::Data(bytes) => writer.write_all(&bytes)?,
                    SinkMessage::Shutdown => break,
                }
            }
            writer.flush()
        });
        Ok(Self {
            sender,
            writer: std::sync::Mutex::new(Some(writer)),
            _marker: PhantomData,
        })
    }

    #[inline]
    pub fn send(&self, event: Event<T>) {
        let key = self as *const Self as usize;
        PRODUCER_BATCHES.with(|batches| {
            let mut batches = batches.borrow_mut();
            let batch = batches.entry(key).or_insert_with(|| ThreadBatch {
                sender: self.sender.clone(),
                bytes: Vec::with_capacity(1024 * 1024),
                events: 0,
            });
            let start = batch.bytes.len();
            batch.bytes.extend_from_slice(&[0; size_of::<u32>()]);
            bincode::serde::encode_into_std_write(
                &event,
                &mut batch.bytes,
                bincode::config::standard()
                    .with_fixed_int_encoding()
                    .with_little_endian(),
            )
            .expect("Serde raw encoding into Vec cannot fail");
            let len = u32::try_from(batch.bytes.len() - start - size_of::<u32>())
                .expect("Serde raw event exceeds u32::MAX bytes");
            batch.bytes[start..start + size_of::<u32>()].copy_from_slice(&len.to_be_bytes());
            batch.events += 1;
            if batch.events >= 64 || batch.bytes.len() >= 1024 * 1024 {
                batch.flush();
            }
        });
    }

    pub fn flush_current(&self) {
        let key = self as *const Self as usize;
        PRODUCER_BATCHES.with(|batches| {
            if let Some(mut batch) = batches.borrow_mut().remove(&key) {
                batch.flush();
            }
        });
    }

    pub fn shutdown(&self) {
        self.flush_current();
        let mut writer = self.writer.lock().unwrap();
        let Some(writer) = writer.take() else {
            return;
        };
        let _ = self.sender.send(SinkMessage::Shutdown);
        writer
            .join()
            .expect("Serde raw writer panicked")
            .expect("Serde raw writer failed");
    }
}

impl<T> NativeRawSink<T>
where
    T: bincode::Encode + EntityEvent,
{
    pub fn try_new(root: &std::path::Path) -> std::io::Result<Self> {
        use std::io::Write;

        let dir = root.join(T::NAME);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.producer.{EXTENSION}", Uuid::now_v7()));
        let file = std::fs::File::create(path)?;
        let (sender, receiver) = std::sync::mpsc::sync_channel(256);
        let writer = std::thread::spawn(move || {
            let mut writer = std::io::BufWriter::with_capacity(1024 * 1024, file);
            while let Ok(message) = receiver.recv() {
                match message {
                    SinkMessage::Data(bytes) => writer.write_all(&bytes)?,
                    SinkMessage::Shutdown => break,
                }
            }
            writer.flush()
        });
        Ok(Self {
            sender,
            writer: std::sync::Mutex::new(Some(writer)),
            _marker: PhantomData,
        })
    }

    #[inline]
    pub fn send(&self, event: Event<T>) {
        let key = self as *const Self as usize;
        PRODUCER_BATCHES.with(|batches| {
            let mut batches = batches.borrow_mut();
            let batch = batches.entry(key).or_insert_with(|| ThreadBatch {
                sender: self.sender.clone(),
                bytes: Vec::with_capacity(1024 * 1024),
                events: 0,
            });
            let start = batch.bytes.len();
            batch.bytes.extend_from_slice(&[0; size_of::<u32>()]);
            bincode::encode_into_std_write(
                &event,
                &mut batch.bytes,
                bincode::config::standard()
                    .with_fixed_int_encoding()
                    .with_little_endian(),
            )
            .expect("native raw encoding into Vec cannot fail");
            let len = u32::try_from(batch.bytes.len() - start - size_of::<u32>())
                .expect("native raw event exceeds u32::MAX bytes");
            batch.bytes[start..start + size_of::<u32>()].copy_from_slice(&len.to_be_bytes());
            batch.events += 1;
            if batch.events >= 64 || batch.bytes.len() >= 1024 * 1024 {
                batch.flush();
            }
        });
    }

    pub fn shutdown(&self) {
        self.flush_current();
        let mut writer = self.writer.lock().unwrap();
        let Some(writer) = writer.take() else {
            return;
        };
        let _ = self.sender.send(SinkMessage::Shutdown);
        writer
            .join()
            .expect("native raw writer panicked")
            .expect("native raw writer failed");
    }

    pub fn flush_current(&self) {
        let key = self as *const Self as usize;
        PRODUCER_BATCHES.with(|batches| {
            if let Some(mut batch) = batches.borrow_mut().remove(&key) {
                batch.flush();
            }
        });
    }
}

#[derive(Debug, Clone)]
pub struct RawExporterOptions {
    pub dir: PathBuf,
}

#[derive(Debug)]
pub struct RawExporter<T> {
    writer: Option<BufWriter<File>>,
    pending: Vec<Event<T>>,
    batches: Vec<Vec<u8>>,
}

impl<T: EntityEvent> RawExporter<T> {
    pub async fn try_new(options: RawExporterOptions) -> ExporterResult<Self> {
        let dir = options.dir.join(T::NAME);
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{}.{EXTENSION}", Uuid::now_v7()));
        debug!("exporting to \"{}\"", path.display());
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        Ok(Self {
            writer: Some(BufWriter::new(file)),
            pending: Vec::new(),
            batches: Vec::new(),
        })
    }
}

async fn write_frame(writer: &mut BufWriter<File>, payload: &[u8]) -> ExporterResult<()> {
    let len = u32::try_from(payload.len()).map_err(|_| {
        ExporterError::other(std::io::Error::other(
            "raw binary frame exceeds u32::MAX bytes",
        ))
    })?;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(payload).await?;
    Ok(())
}

#[async_trait::async_trait]
impl<T> Exporter<T> for RawExporter<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    fn batch_size_hint(&self) -> NonZeroUsize {
        NonZeroUsize::new(BATCH_SIZE).unwrap()
    }

    async fn push(&mut self, event: Event<T>) -> ExporterResult<()> {
        self.pending.push(event);
        if self.pending.len() >= BATCH_SIZE {
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
        if self.pending.len() >= BATCH_SIZE {
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

impl<T> RawExporter<T>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    async fn flush_pending(&mut self) -> ExporterResult<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let writer = self.writer.as_mut().ok_or(ExporterError::Shutdown)?;
        let event_count = self.pending.len();
        let workers = rayon::current_num_threads()
            .min(PARALLEL_CHUNKS)
            .min(event_count);
        let chunk_len = event_count.div_ceil(workers);
        let chunk_count = event_count.div_ceil(chunk_len);
        self.batches.resize_with(chunk_count, Vec::new);
        self.pending
            .par_chunks_mut(chunk_len)
            .zip(self.batches[..chunk_count].par_iter_mut())
            .for_each(|(events, payload)| {
                payload.clear();
                if let Err(e) = bincode::serde::encode_into_std_write(
                    &*events,
                    payload,
                    bincode::config::standard()
                        .with_fixed_int_encoding()
                        .with_little_endian(),
                ) {
                    payload.clear();
                    warn!("unable to encode raw binary batch: {e}");
                }
            });
        for payload in &self.batches[..chunk_count] {
            if !payload.is_empty() {
                write_frame(writer, payload).await?;
            }
        }
        self.pending.par_drain(..).for_each(drop);
        Ok(())
    }
}

/// Native fixed-width binary provider for generated event types deriving
/// [`bincode::Encode`]. This avoids Serde's compatibility visitor on the static
/// schema path while retaining exactly the same on-disk representation policy.
#[derive(Debug, Clone)]
pub struct NativeRawExporterOptions {
    pub root: PathBuf,
}

#[async_trait::async_trait]
impl<T> ExporterProvider<T> for NativeRawExporterOptions
where
    T: bincode::Encode + Send + EntityEvent + 'static,
{
    async fn create_exporter(&self, context_id: Uuid) -> ExporterResult<Box<dyn Exporter<T>>> {
        Ok(Box::new(
            NativeRawExporter::<T>::try_new(RawExporterOptions {
                dir: self.root.join(context_id.to_string()),
            })
            .await?,
        ))
    }
}

struct NativeRawExporter<T> {
    writer: Option<BufWriter<File>>,
    pending: Vec<Event<T>>,
    batches: Vec<Vec<u8>>,
}

impl<T: EntityEvent> NativeRawExporter<T> {
    async fn try_new(options: RawExporterOptions) -> ExporterResult<Self> {
        let dir = options.dir.join(T::NAME);
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{}.native.{EXTENSION}", Uuid::now_v7()));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        Ok(Self {
            writer: Some(BufWriter::new(file)),
            pending: Vec::new(),
            batches: Vec::new(),
        })
    }
}

#[async_trait::async_trait]
impl<T> Exporter<T> for NativeRawExporter<T>
where
    T: bincode::Encode + Send + EntityEvent + 'static,
{
    fn batch_size_hint(&self) -> NonZeroUsize {
        NonZeroUsize::new(BATCH_SIZE).unwrap()
    }

    async fn push(&mut self, event: Event<T>) -> ExporterResult<()> {
        self.pending.push(event);
        if self.pending.len() >= BATCH_SIZE {
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
        if self.pending.len() >= BATCH_SIZE {
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

impl<T> NativeRawExporter<T>
where
    T: bincode::Encode + Send + EntityEvent + 'static,
{
    async fn flush_pending(&mut self) -> ExporterResult<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let writer = self.writer.as_mut().ok_or(ExporterError::Shutdown)?;
        let event_count = self.pending.len();
        let workers = rayon::current_num_threads()
            .min(PARALLEL_CHUNKS)
            .min(event_count);
        let chunk_len = event_count.div_ceil(workers);
        let chunk_count = event_count.div_ceil(chunk_len);
        self.batches.resize_with(chunk_count, Vec::new);
        self.pending
            .par_chunks_mut(chunk_len)
            .zip(self.batches[..chunk_count].par_iter_mut())
            .for_each(|(events, payload)| {
                payload.clear();
                if let Err(e) = bincode::encode_into_std_write(
                    &*events,
                    payload,
                    bincode::config::standard()
                        .with_fixed_int_encoding()
                        .with_little_endian(),
                ) {
                    payload.clear();
                    warn!("unable to encode native raw binary batch: {e}");
                }
            });
        for payload in &self.batches[..chunk_count] {
            if !payload.is_empty() {
                write_frame(writer, payload).await?;
            }
        }
        self.pending.par_drain(..).for_each(drop);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RawImporterOptions {
    pub path: PathBuf,
}

pub struct RawImporter<T> {
    reader: BufReader<std::fs::File>,
    pending: VecDeque<Event<T>>,
    _marker: PhantomData<T>,
}

impl<T> RawImporter<T> {
    pub fn try_new(options: &RawImporterOptions) -> ImporterResult<Self> {
        let path = quent_io_types::resolve_import_path(&options.path, EXTENSION)?;
        Ok(Self {
            reader: BufReader::new(std::fs::File::open(path)?),
            pending: VecDeque::new(),
            _marker: PhantomData,
        })
    }
}

impl<T> Importer<T> for RawImporter<T> where T: for<'de> Deserialize<'de> {}

impl<T> Iterator for RawImporter<T>
where
    T: for<'de> Deserialize<'de>,
{
    type Item = Event<T>;

    fn next(&mut self) -> Option<Self::Item> {
        use std::io::Read;
        if let Some(event) = self.pending.pop_front() {
            return Some(event);
        }
        let mut len = [0; 4];
        match self.reader.read_exact(&mut len) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return None,
            Err(e) => {
                error!("failed to read raw binary length: {e}");
                return None;
            }
        }
        let mut payload = vec![0; u32::from_be_bytes(len) as usize];
        if let Err(e) = self.reader.read_exact(&mut payload) {
            error!("failed to read raw binary payload: {e}");
            return None;
        }
        match bincode::serde::decode_from_slice::<Vec<Event<T>>, _>(
            &payload,
            bincode::config::standard()
                .with_fixed_int_encoding()
                .with_little_endian(),
        ) {
            Ok((events, consumed)) if consumed == payload.len() => {
                self.pending = events.into();
                self.pending.pop_front()
            }
            Ok((_, consumed)) => {
                error!(
                    "raw binary frame has {} trailing bytes",
                    payload.len() - consumed
                );
                None
            }
            Err(batch_error) => match bincode::serde::decode_from_slice::<Event<T>, _>(
                &payload,
                bincode::config::standard()
                    .with_fixed_int_encoding()
                    .with_little_endian(),
            ) {
                Ok((event, consumed)) if consumed == payload.len() => Some(event),
                Ok((_, consumed)) => {
                    error!(
                        "raw binary event frame has {} trailing bytes",
                        payload.len() - consumed
                    );
                    None
                }
                Err(event_error) => {
                    error!(
                        "failed to decode raw binary frame as a batch ({batch_error}) or event ({event_error})"
                    );
                    None
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(bincode::Decode, bincode::Encode, Debug, Deserialize, PartialEq, Serialize)]
    struct TestEvent {
        text: String,
        values: Vec<i64>,
    }

    impl EntityEvent for TestEvent {
        const NAME: &'static str = "raw_producer_test";
    }

    #[test]
    fn producer_encoded_event_roundtrips_through_serde_importer() {
        let root = tempfile::tempdir().unwrap();
        let sink = std::sync::Arc::new(NativeRawSink::<TestEvent>::try_new(root.path()).unwrap());
        let worker_sink = sink.clone();
        std::thread::spawn(move || {
            worker_sink.send(Event::new(
                Uuid::nil(),
                42,
                TestEvent {
                    text: "hello".to_owned(),
                    values: vec![1, 2, 3],
                },
            ));
        })
        .join()
        .unwrap();
        sink.shutdown();
        let mut importer = RawImporter::<TestEvent>::try_new(&RawImporterOptions {
            path: root.path().join(TestEvent::NAME),
        })
        .unwrap();
        let event = importer.next().unwrap();
        assert_eq!(event.id, Uuid::nil());
        assert_eq!(event.timestamp, 42);
        assert_eq!(
            event.data,
            TestEvent {
                text: "hello".to_owned(),
                values: vec![1, 2, 3],
            }
        );
        assert!(importer.next().is_none());
    }
}
