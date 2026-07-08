// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Umbrella crate providing unified exporter/importer creation.

use quent_events::EntityEvent;

use uuid::Uuid;

// Error out compilation if no exporter is selected at all.
#[cfg(not(any(
    feature = "ndjson",
    feature = "msgpack",
    feature = "postcard",
    feature = "collector",
    feature = "callback"
)))]
compile_error!("at least one exporter feature must be enabled");

// Re-exports.
pub use quent_io_types::{
    Exporter, ExporterProvider, ExporterResult, ImporterError, ImporterProvider, ImporterResult,
};

// Feature-gated re-exports for convenience.
#[cfg(filesystem)]
pub use crate::filesystem::{
    Format as FileSystemFormat, exporter::Options as FileSystemExporterOptions,
};
#[cfg(feature = "callback")]
pub use quent_io_callback::EventCallback;
#[cfg(feature = "collector")]
pub use quent_io_collector::Options as CollectorExporterOptions;

// Featue-gated mods.
#[cfg(feature = "clap")]
pub mod clap;
#[cfg(filesystem)]
pub mod filesystem;

/// Where events go: local files (filesystem), a collector service, or a
/// caller-supplied callback (e.g. an in-memory collector for tests).
#[derive(Debug, Clone)]
pub enum ExporterOptions {
    #[cfg(filesystem)]
    FileSystem(FileSystemExporterOptions),
    #[cfg(feature = "collector")]
    Collector(CollectorExporterOptions),
    #[cfg(feature = "callback")]
    Callback(EventCallback),
}

/// Like [`ExporterOptions`], but the collector variant also carries the source
/// context id and a filesystem `root` is the per-context directory.
#[derive(Debug, Clone)]
pub enum ResolvedExporterOptions {
    #[cfg(filesystem)]
    FileSystem(crate::filesystem::exporter::Options),
    #[cfg(feature = "collector")]
    Collector(quent_io_collector::Options),
    #[cfg(feature = "callback")]
    Callback(quent_io_callback::EventCallback),
}

impl ResolvedExporterOptions {
    /// Filesystem output directory for filesystem exporters; `None` for
    /// exporters (e.g. the collector) that do not write a local directory.
    /// Used to locate where a provenance sidecar should be written.
    pub fn filesystem_root(&self) -> Option<&std::path::Path> {
        match self {
            #[cfg(filesystem)]
            ResolvedExporterOptions::FileSystem(options) => Some(&options.root),
            #[cfg(feature = "collector")]
            ResolvedExporterOptions::Collector { .. } => None,
            #[cfg(feature = "callback")]
            ResolvedExporterOptions::Callback(_) => None,
        }
    }
}

impl ExporterOptions {
    pub fn resolve(self, id: Uuid) -> ResolvedExporterOptions {
        match self {
            #[cfg(filesystem)]
            ExporterOptions::FileSystem(options) => {
                ResolvedExporterOptions::FileSystem(options.resolve(id))
            }
            #[cfg(feature = "collector")]
            ExporterOptions::Collector(options) => {
                ResolvedExporterOptions::Collector(options.resolve(id))
            }
            #[cfg(feature = "callback")]
            ExporterOptions::Callback(callback) => ResolvedExporterOptions::Callback(callback),
        }
    }
}

#[cfg(any(filesystem, feature = "collector"))]
#[async_trait::async_trait]
impl<T> ExporterProvider<T> for ResolvedExporterOptions
where
    T: serde::Serialize + Send + EntityEvent + 'static,
{
    async fn create_exporter(&self) -> ExporterResult<Box<dyn Exporter<T>>> {
        match self {
            #[cfg(filesystem)]
            ResolvedExporterOptions::FileSystem(options) => options.create_exporter().await,
            #[cfg(feature = "collector")]
            ResolvedExporterOptions::Collector(options) => options.create_exporter().await,
            #[cfg(feature = "callback")]
            ResolvedExporterOptions::Callback(callback) => callback.create_exporter().await,
        }
    }
}

#[cfg(not(any(filesystem, feature = "collector")))]
#[async_trait::async_trait]
impl<T> ExporterProvider<T> for ResolvedExporterOptions
where
    T: Send + EntityEvent + 'static,
{
    async fn create_exporter(&self) -> ExporterResult<Box<dyn Exporter<T>>> {
        match self {
            #[cfg(feature = "callback")]
            ResolvedExporterOptions::Callback(callback) => callback.create_exporter().await,
        }
    }
}

/// Selects an importer and its options.
#[derive(Debug, Clone)]
pub enum ImporterOptions {
    #[cfg(filesystem)]
    FileSystem(crate::filesystem::importer::Options),
}

#[cfg(filesystem)]
impl<T> ImporterProvider<T> for ImporterOptions
where
    T: Send + EntityEvent + 'static,
    for<'a> T: serde::Deserialize<'a>,
{
    fn create_importer(&self) -> ImporterResult<Box<dyn quent_io_types::Importer<T>>> {
        match self {
            #[cfg(filesystem)]
            ImporterOptions::FileSystem(options) => options.create_importer(),
        }
    }
}
