// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Umbrella crate providing unified exporter/importer creation.

use std::path::PathBuf;

use quent_events::EntityEvent;
use quent_exporter_types::{Exporter, ExporterError, ExporterResult, Importer};
use serde::{Deserialize, Serialize};

// Part of the public API: `create_importer` returns `ImporterResult`, so callers
// must be able to name it (and its error).
pub use quent_exporter_types::{ImporterError, ImporterResult};
use uuid::Uuid;

#[cfg(not(any(
    feature = "ndjson",
    feature = "msgpack",
    feature = "postcard",
    feature = "collector"
)))]
compile_error!("at least one exporter feature must be enabled");

#[cfg(feature = "collector")]
pub use quent_exporter_collector::CollectorExporterOptions;

/// Where events go: local files (filesystem) or a collector service.
#[derive(Debug, Clone)]
pub enum ExporterOptions {
    FileSystem(FileSystemExporterOptions),
    #[cfg(feature = "collector")]
    Collector(CollectorExporterOptions),
}

/// Like [`ExporterOptions`], but the collector variant also carries the source
/// context id and a filesystem `root` is the per-context directory.
#[derive(Debug, Clone)]
pub enum ResolvedExporterOptions {
    FileSystem(FileSystemExporterOptions),
    #[cfg(feature = "collector")]
    Collector {
        address: String,
        source_context_id: Uuid,
    },
}

impl ResolvedExporterOptions {
    /// Filesystem output directory for filesystem exporters; `None` for
    /// exporters (e.g. the collector) that do not write a local directory.
    /// Used to locate where a provenance sidecar should be written.
    pub fn filesystem_root(&self) -> Option<&std::path::Path> {
        match self {
            ResolvedExporterOptions::FileSystem(options) => Some(&options.root),
            #[cfg(feature = "collector")]
            ResolvedExporterOptions::Collector { .. } => None,
        }
    }
}

/// Serialization format for the filesystem exporter and importer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSystemFormat {
    #[cfg(feature = "ndjson")]
    Ndjson,
    #[cfg(feature = "msgpack")]
    Msgpack,
    #[cfg(feature = "postcard")]
    Postcard,
}

/// Options for exporting events to the filesystem in the given `format`, under
/// the directory `root`, together with a `model.qmi` provenance sidecar.
#[derive(Debug, Clone)]
pub struct FileSystemExporterOptions {
    pub format: FileSystemFormat,
    pub root: PathBuf,
}

impl ExporterOptions {
    /// Bind these options to the context `id`: scope a filesystem `root` to the
    /// context directory, or set the collector's source context id.
    pub fn resolve(self, id: Uuid) -> ResolvedExporterOptions {
        match self {
            ExporterOptions::FileSystem(mut options) => {
                options.root = options.root.join(id.to_string());
                ResolvedExporterOptions::FileSystem(options)
            }
            #[cfg(feature = "collector")]
            ExporterOptions::Collector(options) => ResolvedExporterOptions::Collector {
                address: options.address,
                source_context_id: id,
            },
        }
    }
}

/// Selects an importer and its options.
#[derive(Debug, Clone)]
pub enum ImporterOptions {
    FileSystem(FileSystemImporterOptions),
}

/// Options for importing events from the filesystem in the given `format`.
/// `path` is either a directory containing the event file (located by the
/// format's extension) or a direct file path.
#[derive(Debug, Clone)]
pub struct FileSystemImporterOptions {
    pub format: FileSystemFormat,
    pub path: PathBuf,
}

/// Construct an importer from [`ImporterOptions`].
pub fn create_importer<T>(kind: &ImporterOptions) -> ImporterResult<Box<dyn Importer<T>>>
where
    T: for<'de> Deserialize<'de> + 'static,
{
    match kind {
        ImporterOptions::FileSystem(FileSystemImporterOptions { format, path }) => match format {
            #[cfg(feature = "ndjson")]
            FileSystemFormat::Ndjson => {
                Ok(Box::new(quent_exporter_ndjson::NdjsonImporter::try_new(
                    &quent_exporter_ndjson::NdjsonImporterOptions { path: path.clone() },
                )?) as Box<dyn Importer<T>>)
            }
            #[cfg(feature = "msgpack")]
            FileSystemFormat::Msgpack => {
                Ok(Box::new(quent_exporter_msgpack::MsgpackImporter::try_new(
                    &quent_exporter_msgpack::MsgpackImporterOptions { path: path.clone() },
                )?) as Box<dyn Importer<T>>)
            }
            #[cfg(feature = "postcard")]
            FileSystemFormat::Postcard => {
                Ok(Box::new(quent_exporter_postcard::PostcardImporter::try_new(
                    &quent_exporter_postcard::PostcardImporterOptions { path: path.clone() },
                )?) as Box<dyn Importer<T>>)
            }
        },
    }
}

/// Construct an exporter from [`ResolvedExporterOptions`].
pub async fn create_exporter<T>(
    kind: ResolvedExporterOptions,
) -> ExporterResult<Box<dyn Exporter<T>>>
where
    T: Serialize + Send + EntityEvent + 'static,
{
    match kind {
        ResolvedExporterOptions::FileSystem(FileSystemExporterOptions { format, root }) => {
            match format {
                #[cfg(feature = "ndjson")]
                FileSystemFormat::Ndjson => Ok(Box::new(
                    quent_exporter_ndjson::NdjsonExporter::try_new::<T>(
                        quent_exporter_ndjson::NdjsonExporterOptions { dir: root },
                    )
                    .await?,
                ) as Box<dyn Exporter<T>>),
                #[cfg(feature = "msgpack")]
                FileSystemFormat::Msgpack => Ok(Box::new(
                    quent_exporter_msgpack::MsgpackExporter::try_new::<T>(
                        quent_exporter_msgpack::MsgpackExporterOptions { dir: root },
                    )
                    .await?,
                ) as Box<dyn Exporter<T>>),
                #[cfg(feature = "postcard")]
                FileSystemFormat::Postcard => Ok(Box::new(
                    quent_exporter_postcard::PostcardExporter::try_new::<T>(
                        quent_exporter_postcard::PostcardExporterOptions { dir: root },
                    )
                    .await?,
                ) as Box<dyn Exporter<T>>),
            }
        }
        #[cfg(feature = "collector")]
        ResolvedExporterOptions::Collector {
            address,
            source_context_id,
        } => {
            let address: http::Uri = address.parse().map_err(ExporterError::other)?;
            Ok(Box::new(
                quent_exporter_collector::CollectorExporter::<T>::try_new(
                    address,
                    source_context_id,
                )
                .await
                .map_err(ExporterError::Other)?,
            ) as Box<dyn Exporter<T>>)
        }
    }
}
