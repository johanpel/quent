// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Exporter writing generated columnar event rows to Parquet files.

use std::{fs::File, path::PathBuf, sync::Arc};

pub use narrow;
use narrow::arrow_array::RecordBatch;
use parquet::arrow::ArrowWriter;
use quent_events::{EntityEvent, Event};
use quent_io_types::Exporter;
pub use quent_io_types::{ExporterError, ExporterResult};
use tracing::debug;
use uuid::Uuid;

const EXTENSION: &str = "parquet";

/// Converts an event value into its generated Parquet representation.
pub trait ParquetValue {
    type Value;

    fn into_parquet(self) -> Self::Value;
}

macro_rules! identity_value {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ParquetValue for $ty {
                type Value = Self;

                fn into_parquet(self) -> Self::Value {
                    self
                }
            }
        )*
    };
}

identity_value!(
    (),
    bool,
    u8,
    u16,
    u32,
    u64,
    i8,
    i16,
    i32,
    i64,
    f32,
    f64,
    String,
    Uuid,
);

impl<T: ParquetValue> ParquetValue for Option<T> {
    type Value = Option<T::Value>;

    fn into_parquet(self) -> Self::Value {
        self.map(ParquetValue::into_parquet)
    }
}

impl<T: ParquetValue> ParquetValue for Vec<T> {
    type Value = Vec<T::Value>;

    fn into_parquet(self) -> Self::Value {
        self.into_iter().map(ParquetValue::into_parquet).collect()
    }
}

/// Builds Arrow record batches for one generated entity event stream.
pub trait ParquetEvent: EntityEvent {
    fn parquet_schema() -> ExporterResult<Arc<narrow::arrow_schema::Schema>> {
        Err(unsupported_event::<Self>())
    }

    fn into_record_batch(events: Vec<Event<Self>>) -> ExporterResult<RecordBatch>
    where
        Self: Sized,
    {
        drop(events);
        Err(unsupported_event::<Self>())
    }
}

pub struct ParquetDescriptor<T> {
    schema: fn() -> ExporterResult<Arc<narrow::arrow_schema::Schema>>,
    record_batch: fn(Vec<Event<T>>) -> ExporterResult<RecordBatch>,
}

impl<T> ParquetDescriptor<T> {
    pub const fn new(
        schema: fn() -> ExporterResult<Arc<narrow::arrow_schema::Schema>>,
        record_batch: fn(Vec<Event<T>>) -> ExporterResult<RecordBatch>,
    ) -> Self {
        Self {
            schema,
            record_batch,
        }
    }
}

pub const EXPORTER_METADATA: &str = "quent.parquet.v1";

fn descriptor<T: EntityEvent + 'static>() -> ExporterResult<&'static ParquetDescriptor<T>> {
    T::exporter_metadata(EXPORTER_METADATA)
        .and_then(|metadata| metadata.downcast_ref::<ParquetDescriptor<T>>())
        .ok_or_else(unsupported_event::<T>)
}

fn unsupported_event<T: EntityEvent + ?Sized>() -> ExporterError {
    ExporterError::other(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("Parquet schema is not implemented for {}", T::NAME),
    ))
}

/// Options for the Parquet exporter.
#[derive(Debug, Clone)]
pub struct ParquetExporterOptions {
    pub dir: PathBuf,
}

pub struct ParquetExporter {
    writer: Option<ArrowWriter<File>>,
}

impl ParquetExporter {
    pub fn try_new<T: EntityEvent + 'static>(
        options: ParquetExporterOptions,
    ) -> ExporterResult<Self> {
        let schema = (descriptor::<T>()?.schema)()?;
        let dir = options.dir.join(T::NAME);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.{EXTENSION}", Uuid::now_v7()));
        debug!("exporting to \"{}\"", path.display());
        let file = File::create(path)?;
        let writer = ArrowWriter::try_new(file, schema, None).map_err(ExporterError::other)?;
        Ok(Self {
            writer: Some(writer),
        })
    }

    fn write<T: EntityEvent + 'static>(&mut self, events: Vec<Event<T>>) -> ExporterResult<()> {
        let writer = self.writer.as_mut().ok_or(ExporterError::Shutdown)?;
        if events.is_empty() {
            return Ok(());
        }
        writer
            .write(&(descriptor::<T>()?.record_batch)(events)?)
            .map_err(ExporterError::other)
    }
}

#[async_trait::async_trait]
impl<T> Exporter<T> for ParquetExporter
where
    T: EntityEvent + Send + 'static,
{
    async fn push(&mut self, event: Event<T>) -> ExporterResult<()> {
        self.write(vec![event])
    }

    async fn drain_events(&mut self, events: &mut Vec<Event<T>>) -> ExporterResult<()> {
        self.write(std::mem::take(events))
    }

    async fn shutdown(mut self: Box<Self>) -> ExporterResult<()> {
        if let Some(writer) = self.writer.take() {
            writer.close().map_err(ExporterError::other)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(narrow::ArrayType)]
    struct TestRow {
        id: Uuid,
        timestamp: u64,
        value: u64,
    }

    struct TestEvent {
        value: u64,
    }

    impl EntityEvent for TestEvent {
        const NAME: &'static str = "test";

        fn exporter_metadata(name: &str) -> Option<&'static (dyn std::any::Any + Send + Sync)> {
            if name != EXPORTER_METADATA {
                return None;
            }
            static DESCRIPTOR: ParquetDescriptor<TestEvent> = ParquetDescriptor::new(
                <TestEvent as ParquetEvent>::parquet_schema,
                <TestEvent as ParquetEvent>::into_record_batch,
            );
            Some(&DESCRIPTOR)
        }
    }

    impl ParquetEvent for TestEvent {
        fn parquet_schema() -> ExporterResult<Arc<narrow::arrow_schema::Schema>> {
            Ok(Arc::new(narrow::array::StructArray::<TestRow>::schema()))
        }

        fn into_record_batch(events: Vec<Event<Self>>) -> ExporterResult<RecordBatch> {
            let rows = events.into_iter().map(|event| TestRow {
                id: event.id,
                timestamp: event.timestamp,
                value: event.data.value,
            });
            Ok(RecordBatch::from(
                rows.collect::<narrow::array::StructArray<TestRow>>(),
            ))
        }
    }

    struct UnsupportedEvent;

    impl EntityEvent for UnsupportedEvent {
        const NAME: &'static str = "unsupported";
    }

    #[tokio::test]
    async fn writes_registered_event_stream() {
        let temp = tempfile::tempdir().unwrap();
        let mut exporter = ParquetExporter::try_new::<TestEvent>(ParquetExporterOptions {
            dir: temp.path().to_path_buf(),
        })
        .unwrap();
        let mut events = vec![Event::new(Uuid::nil(), 10, TestEvent { value: 20 })];

        exporter.drain_events(&mut events).await.unwrap();
        <ParquetExporter as Exporter<TestEvent>>::shutdown(Box::new(exporter))
            .await
            .unwrap();

        let files = std::fs::read_dir(temp.path().join(TestEvent::NAME))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(files.len(), 1);
        let bytes = std::fs::read(files[0].path()).unwrap();
        assert!(bytes.starts_with(b"PAR1"));
        assert!(bytes.ends_with(b"PAR1"));
    }

    #[test]
    fn rejects_unregistered_event_without_creating_output() {
        let temp = tempfile::tempdir().unwrap();
        let result = ParquetExporter::try_new::<UnsupportedEvent>(ParquetExporterOptions {
            dir: temp.path().to_path_buf(),
        });

        assert!(result.is_err());
        assert!(!temp.path().join(UnsupportedEvent::NAME).exists());
    }
}
