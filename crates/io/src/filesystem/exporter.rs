// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use quent_events::EntityEvent;
use quent_io_types::{Exporter, ExporterProvider, ExporterResult};
use serde::Serialize;
use uuid::Uuid;

use crate::filesystem::Format;

/// Options for exporting events to the filesystem in the given `format`, under
/// the directory `root`, together with a `model.qmi` provenance sidecar.
#[derive(Debug, Clone)]
pub struct Options {
    pub format: Format,
    pub root: PathBuf,
}

impl Options {
    pub fn resolve(mut self, id: Uuid) -> Self {
        self.root = self.root.join(id.to_string());
        self
    }
}

#[async_trait::async_trait]
impl<T> ExporterProvider<T> for Options
where
    T: Send + EntityEvent + 'static,
    T: Serialize,
{
    async fn create_exporter(&self) -> ExporterResult<Box<dyn Exporter<T>>> {
        match self.format {
            #[cfg(feature = "ndjson")]
            Format::Ndjson => Ok(Box::new(
                quent_io_ndjson::NdjsonExporter::try_new::<T>(
                    quent_io_ndjson::NdjsonExporterOptions {
                        dir: self.root.clone(),
                    },
                )
                .await?,
            ) as Box<dyn Exporter<T>>),
            #[cfg(feature = "msgpack")]
            Format::Msgpack => Ok(Box::new(
                quent_io_msgpack::MsgpackExporter::try_new::<T>(
                    quent_io_msgpack::MsgpackExporterOptions {
                        dir: self.root.clone(),
                    },
                )
                .await?,
            ) as Box<dyn Exporter<T>>),
            #[cfg(feature = "postcard")]
            Format::Postcard => Ok(Box::new(
                quent_io_postcard::PostcardExporter::try_new::<T>(
                    quent_io_postcard::PostcardExporterOptions {
                        dir: self.root.clone(),
                    },
                )
                .await?,
            ) as Box<dyn Exporter<T>>),
        }
    }
}
