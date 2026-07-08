// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use quent_events::EntityEvent;
use quent_io_types::{Exporter, ExporterError, ExporterProvider, ExporterResult};
use serde::Serialize;
use uuid::Uuid;

use crate::filesystem::Format;

/// Options for exporting events to the filesystem in the given `format`, under
/// `root/<context_id>`, together with a `model.qmi` provenance sidecar.
#[derive(Debug, Clone)]
pub struct Options {
    format: Format,
    root: PathBuf,
    context_id: Uuid,
}

impl Options {
    /// New options with an unset (nil) context id; set it with
    /// [`Self::with_context_id`] before building an exporter.
    pub fn new(format: Format, root: PathBuf) -> Self {
        Self {
            format,
            root,
            context_id: Uuid::nil(),
        }
    }

    /// Scope the output directory to the context `id`.
    pub fn with_context_id(mut self, id: Uuid) -> Self {
        self.context_id = id;
        self
    }

    /// The per-context output directory, `root/<context_id>`.
    pub(crate) fn dir(&self) -> PathBuf {
        self.root.join(self.context_id.to_string())
    }
}

#[async_trait::async_trait]
impl<T> ExporterProvider<T> for Options
where
    T: Send + EntityEvent + 'static,
    T: Serialize,
{
    async fn create_exporter(&self) -> ExporterResult<Box<dyn Exporter<T>>> {
        if self.context_id.is_nil() {
            return Err(ExporterError::Other(
                "filesystem exporter requires a context id; call `with_context_id` first".into(),
            ));
        }
        let dir = self.dir();
        match self.format {
            #[cfg(feature = "ndjson")]
            Format::Ndjson => Ok(Box::new(
                quent_io_ndjson::NdjsonExporter::try_new::<T>(
                    quent_io_ndjson::NdjsonExporterOptions { dir },
                )
                .await?,
            ) as Box<dyn Exporter<T>>),
            #[cfg(feature = "msgpack")]
            Format::Msgpack => Ok(Box::new(
                quent_io_msgpack::MsgpackExporter::try_new::<T>(
                    quent_io_msgpack::MsgpackExporterOptions { dir },
                )
                .await?,
            ) as Box<dyn Exporter<T>>),
            #[cfg(feature = "postcard")]
            Format::Postcard => Ok(Box::new(
                quent_io_postcard::PostcardExporter::try_new::<T>(
                    quent_io_postcard::PostcardExporterOptions { dir },
                )
                .await?,
            ) as Box<dyn Exporter<T>>),
        }
    }
}
