// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::{net::ToSocketAddrs, path::PathBuf};

use clap::Parser;
use quent_io::ExporterOptions;
use quent_io::filesystem::{self, Format};
use quent_query_engine_server::{
    analyzer_cache::{EngineIndex, EngineIndexEntry},
    analyzer_service_router, collector_service, initialize_tracing,
};
use quent_simulator_analyzer::SimulatorUiAnalyzer;
use quent_simulator_instrumentation::{
    Context, EngineEvent, Simulator, SimulatorEvent, WorkerEvent,
};
use quent_store::{ModelEventStore, filesystem::Store};
use tokio::net::TcpListener;

type SimulatorContext = Context<Simulator>;

fn classify_index_event(entity_id: uuid::Uuid, event: &SimulatorEvent) -> Option<EngineIndexEntry> {
    match event {
        SimulatorEvent::Engine(EngineEvent::Init { .. }) => Some(EngineIndexEntry::Engine {
            engine_id: entity_id,
        }),
        SimulatorEvent::Worker(WorkerEvent::Init {
            parent_engine_id, ..
        }) => Some(EngineIndexEntry::Worker {
            engine_id: parent_engine_id.target,
            worker_id: entity_id,
        }),
        _ => None,
    }
}

mod defaults {
    /// Default collector socket address to listen on.
    pub(crate) const QUENT_COLLECTOR_ADDRESS: &str = "[::]:7836";
    /// Default analyzer socket address to listen on.
    pub(crate) const QUENT_ANALYZER_ADDRESS: &str = "[::]:8080";
}

mod env {
    /// Collector socket address environment variable name.
    pub(crate) const QUENT_COLLECTOR_ADDRESS: &str = "QUENT_COLLECTOR_ADDRESS";
    /// Collector output directory environment variable name.
    pub(crate) const QUENT_COLLECTOR_OUTPUT_DIR: &str = "QUENT_COLLECTOR_OUTPUT_DIR";
    /// Exporter type environment variable name.
    pub(crate) const QUENT_COLLECTOR_EXPORTER: &str = "QUENT_COLLECTOR_EXPORTER";
    /// Analyzer socket address environment variable name.
    pub(crate) const QUENT_ANALYZER_ADDRESS: &str = "QUENT_ANALYZER_ADDRESS";
    /// Optional CORS address environment variable name.
    pub(crate) const QUENT_ANALYZER_CORS_ADDRESS: &str = "QUENT_ANALYZER_CORS_ADDRESS";
}

#[derive(Parser)]
struct Args {
    /// Log level filter (e.g. "debug", "info", "warn", "error").
    /// Overridden by the RUST_LOG environment variable if set.
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Socket address for the collector gRPC server (e.g. `[::]:7836`).
    /// Overridden by the QUENT_COLLECTOR_ADDRESS environment variable if set.
    #[arg(long, default_value = defaults::QUENT_COLLECTOR_ADDRESS, env = env::QUENT_COLLECTOR_ADDRESS)]
    collector_address: String,

    /// Exporter format for collected event data (ndjson, msgpack, postcard).
    /// Overridden by the QUENT_COLLECTOR_EXPORTER environment variable if set.
    #[arg(long, default_value = "ndjson", env = env::QUENT_COLLECTOR_EXPORTER)]
    exporter: String,

    /// Output directory for collected event data.
    /// Overridden by the QUENT_COLLECTOR_OUTPUT_DIR environment variable if set.
    #[arg(long, default_value = "events", env = env::QUENT_COLLECTOR_OUTPUT_DIR)]
    output_dir: PathBuf,

    /// Socket address for the analyzer HTTP server (e.g. `[::]:8080`).
    /// Overridden by the QUENT_ANALYZER_ADDRESS environment variable if set.
    #[arg(long, default_value = defaults::QUENT_ANALYZER_ADDRESS, env = env::QUENT_ANALYZER_ADDRESS)]
    analyzer_address: String,

    /// Address to allow CORS requests from (e.g. "http://localhost:5173").
    /// If not set, CORS is disabled.
    /// Overridden by the QUENT_CORS_ADDRESS environment variable if set.
    #[arg(long, env = env::QUENT_ANALYZER_CORS_ADDRESS)]
    cors_address: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        log_level,
        cors_address,
        collector_address,
        exporter,
        output_dir,
        analyzer_address,
    } = Args::parse();

    initialize_tracing(&log_level);

    let collector_addr = collector_address
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| format!("unable to resolve socket address: {collector_address}"))?;

    let importer_output_dir = output_dir.clone();
    let format = match exporter.as_str() {
        "ndjson" => Format::Ndjson,
        "msgpack" => Format::Msgpack,
        "postcard" => Format::Postcard,
        other => return Err(format!("unknown exporter: {other}").into()),
    };
    let exporter_kind = ExporterOptions::FileSystem(filesystem::exporter::Options::new(
        format,
        output_dir.clone(),
    ));

    let collector = async {
        collector_service::<SimulatorContext, _>(move |id| {
            SimulatorContext::try_with_id(id, exporter_kind.clone()).map_err(|e| e.to_string())
        })?
        .serve(collector_addr)
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })
    };

    let analyzer_addr = analyzer_address
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| format!("unable to resolve socket address: {analyzer_address}"))?;

    // Index the exported contexts by engine instance: each engine's telemetry is
    // the engine's own context plus its workers' contexts.
    let lister_store = Store::<Simulator>::new(output_dir.clone());
    let lister = move || EngineIndex::from_event_store(&lister_store, classify_index_event);

    // Reconstruct one context's umbrella event stream from its per-entity
    // subdirectories; the analyzer cache chains this across all the contexts that
    // make up an engine instance.
    let importer_store = Store::<Simulator>::new(importer_output_dir);
    let importer = move |context_id| Ok(importer_store.events(context_id)?);

    let analyzer = async {
        axum::serve(
            TcpListener::bind(analyzer_addr).await?,
            analyzer_service_router::<SimulatorUiAnalyzer>(
                Box::new(importer),
                Box::new(lister),
                cors_address,
            )?
            .into_make_service(),
        )
        .await?;
        Ok::<(), Box<dyn std::error::Error>>(())
    };

    tracing::info!("listening on {collector_addr} and {analyzer_addr}");

    tokio::try_join!(collector, analyzer)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quent_simulator_instrumentation::{
        DynamicAttributes, Engine, EngineImplementationAttributes, Worker,
    };
    use uuid::Uuid;

    #[test]
    fn indexes_schema_events_from_filesystem_store() {
        let output = tempfile::tempdir().unwrap();
        let context_id = Uuid::now_v7();
        let engine_id = Uuid::now_v7();
        let worker_id = Uuid::now_v7();
        {
            let context = SimulatorContext::try_with_id(
                context_id,
                ExporterOptions::FileSystem(filesystem::exporter::Options::new(
                    Format::Ndjson,
                    output.path().to_owned(),
                )),
            )
            .unwrap();
            let mut engine = context.observer::<Engine>().handle_with_id(engine_id);
            engine
                .init(
                    EngineImplementationAttributes {
                        name: Some("test".to_owned()),
                        version: None,
                        custom_attributes: DynamicAttributes::default(),
                    },
                    Some("engine".to_owned()),
                )
                .unwrap();
            let mut worker = context.observer::<Worker>().handle_with_id(worker_id);
            worker
                .init(engine.as_entity_ref(), "worker".to_owned())
                .unwrap();
        }

        let store = Store::<Simulator>::new(output.path());
        let index = EngineIndex::from_event_store(&store, classify_index_event).unwrap();

        assert_eq!(index.contexts_of(engine_id), vec![context_id]);
        assert_eq!(index.workers_of(engine_id), &[(worker_id, context_id)]);
    }
}
