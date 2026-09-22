// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::hint::black_box;
use std::io::{BufWriter, Write as _};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Instant, SystemTime};

use opentelemetry::logs::{LogRecord as _, Logger as _, LoggerProvider as _};
use opentelemetry_sdk::error::{OTelSdkError, OTelSdkResult};
use opentelemetry_sdk::logs::{LogBatch, LogExporter, SdkLoggerProvider};
use quent_bench_rust::instrumentation::{Context, Entity, InstrumentationLatency, Noop};
use quent_instrumentation::{ExporterOptions, FileSystemExporterOptions, FileSystemFormat};
use slog::{Drain as _, KV as _};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context as LayerContext, SubscriberExt as _};
use tracing_subscriber::{Layer, Registry};

#[derive(Clone, Copy)]
enum OutputMode {
    Noop,
    Text,
}

impl OutputMode {
    fn parse(value: &str) -> Self {
        match value {
            "noop" => Self::Noop,
            "text" => Self::Text,
            _ => panic!("unknown output mode: {value}"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::Text => "text",
        }
    }
}

#[derive(Debug)]
struct BenchmarkLogExporter {
    writer: Option<Mutex<BufWriter<std::fs::File>>>,
}

impl LogExporter for BenchmarkLogExporter {
    fn export(&self, batch: LogBatch<'_>) -> impl Future<Output = OTelSdkResult> + Send {
        if let Some(writer) = &self.writer {
            let mut writer = writer
                .lock()
                .expect("OpenTelemetry output lock should be usable");
            for (record, scope) in batch.iter() {
                writeln!(writer, "{record:?} {scope:?}")
                    .expect("OpenTelemetry text record should be written");
            }
        } else {
            black_box(batch.iter().count());
        }
        std::future::ready(Ok(()))
    }

    fn shutdown(&self) -> OTelSdkResult {
        if let Some(writer) = &self.writer {
            writer
                .lock()
                .expect("OpenTelemetry output lock should be usable")
                .flush()
                .map_err(|error| OTelSdkError::InternalFailure(error.to_string()))?;
        }
        Ok(())
    }
}

struct BenchmarkLog {
    writer: Option<Mutex<BufWriter<std::fs::File>>>,
}

struct DiscardLogVisitor;

impl<'kvs> log::kv::VisitSource<'kvs> for DiscardLogVisitor {
    fn visit_pair(
        &mut self,
        key: log::kv::Key<'kvs>,
        value: log::kv::Value<'kvs>,
    ) -> Result<(), log::kv::Error> {
        black_box(key);
        black_box(value);
        Ok(())
    }
}

impl log::Log for BenchmarkLog {
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        let timestamp = SystemTime::now();
        black_box(record.metadata());
        if let Some(writer) = &self.writer {
            let mut writer = writer.lock().expect("log output lock should be usable");
            write!(
                writer,
                "{timestamp:?} {} {} ",
                record.level(),
                record.target()
            )
            .expect("log text prefix should be written");
            record
                .key_values()
                .visit(&mut TextLogVisitor(&mut writer))
                .expect("text visitor should accept entity ID");
            writeln!(writer, "{}", record.args()).expect("log text record should be written");
        } else {
            black_box(timestamp);
            record
                .key_values()
                .visit(&mut DiscardLogVisitor)
                .expect("discard visitor should accept entity ID");
        }
    }

    fn flush(&self) {
        if let Some(writer) = &self.writer {
            writer
                .lock()
                .expect("log output lock should be usable")
                .flush()
                .expect("log output should flush");
        }
    }
}

struct TextLogVisitor<'a>(&'a mut BufWriter<std::fs::File>);

impl<'kvs> log::kv::VisitSource<'kvs> for TextLogVisitor<'_> {
    fn visit_pair(
        &mut self,
        key: log::kv::Key<'kvs>,
        value: log::kv::Value<'kvs>,
    ) -> Result<(), log::kv::Error> {
        write!(self.0, "{key}={value} ")
            .map_err(|_| log::kv::Error::msg("unable to write structured field"))
    }
}

struct DiscardLayer;

struct DiscardTracingVisitor;

impl Visit for DiscardTracingVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        black_box(field);
        black_box(value);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        black_box(field);
        black_box(value.as_bytes());
    }
}

impl<S> Layer<S> for DiscardLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _context: LayerContext<'_, S>) {
        black_box(SystemTime::now());
        black_box(event.metadata());
        event.record(&mut DiscardTracingVisitor);
    }
}

struct BenchmarkSlogDrain {
    writer: Option<Mutex<BufWriter<std::fs::File>>>,
}

struct DiscardSlogSerializer;

impl slog::Serializer for DiscardSlogSerializer {
    fn emit_arguments(&mut self, key: slog::Key, value: &std::fmt::Arguments<'_>) -> slog::Result {
        black_box(key);
        black_box(value);
        Ok(())
    }

    fn emit_str(&mut self, key: slog::Key, value: &str) -> slog::Result {
        black_box(key);
        black_box(value.as_bytes());
        Ok(())
    }
}

struct TextSlogSerializer<'a>(&'a mut BufWriter<std::fs::File>);

impl slog::Serializer for TextSlogSerializer<'_> {
    fn emit_arguments(&mut self, key: slog::Key, value: &std::fmt::Arguments<'_>) -> slog::Result {
        write!(self.0, "{key}={value} ").map_err(slog::Error::Io)
    }

    fn emit_str(&mut self, key: slog::Key, value: &str) -> slog::Result {
        write!(self.0, "{key}={value} ").map_err(slog::Error::Io)
    }
}

impl slog::Drain for BenchmarkSlogDrain {
    type Ok = ();
    type Err = slog::Never;

    fn log(
        &self,
        record: &slog::Record<'_>,
        values: &slog::OwnedKVList,
    ) -> Result<Self::Ok, Self::Err> {
        let timestamp = SystemTime::now();
        black_box(record.location());
        if let Some(writer) = &self.writer {
            let mut writer = writer.lock().expect("slog output lock should be usable");
            write!(writer, "{timestamp:?} {} ", record.level()).expect("slog prefix should write");
            values
                .serialize(record, &mut TextSlogSerializer(&mut writer))
                .expect("text serializer should accept bound entity ID");
            record
                .kv()
                .serialize(record, &mut TextSlogSerializer(&mut writer))
                .expect("text serializer should accept event entity ID");
            writeln!(writer, "{}", record.msg()).expect("slog record should write");
        } else {
            black_box(timestamp);
            values
                .serialize(record, &mut DiscardSlogSerializer)
                .expect("discard serializer should accept bound entity ID");
            record
                .kv()
                .serialize(record, &mut DiscardSlogSerializer)
                .expect("discard serializer should accept event entity ID");
        }
        Ok(())
    }
}

fn measure(calls: u64, mut operation: impl FnMut()) -> f64 {
    let start = Instant::now();
    for _ in 0..calls {
        operation();
    }
    start.elapsed().as_secs_f64() * 1_000_000_000.0 / calls as f64
}

fn entity_id() -> String {
    let context = Context::<InstrumentationLatency>::try_new(Noop)
        .expect("Quent no-op context should initialize");
    let entity = context.observer::<Entity>().handle();
    entity.uuid().to_string()
}

fn export_path(name: &str) -> PathBuf {
    std::env::var_os("QUENT_BENCH_EXPORT_DIR")
        .map(PathBuf::from)
        .expect("QUENT_BENCH_EXPORT_DIR must be set")
        .join(name)
}

fn text_writer(name: &str) -> Mutex<BufWriter<std::fs::File>> {
    Mutex::new(BufWriter::new(
        std::fs::File::create(export_path(name)).expect("text output should be created"),
    ))
}

fn benchmark(implementation: &str, output_mode: OutputMode, calls: u64) -> f64 {
    match implementation {
        "clock" => measure(calls, || {
            black_box(SystemTime::now());
        }),
        "quent" => {
            let context = match output_mode {
                OutputMode::Noop => Context::<InstrumentationLatency>::try_new(Noop),
                OutputMode::Text => Context::<InstrumentationLatency>::try_new(
                    ExporterOptions::FileSystem(FileSystemExporterOptions::new(
                        FileSystemFormat::Ndjson,
                        export_path("quent"),
                    )),
                ),
            }
            .expect("Quent context should initialize");
            let entity = context.observer::<Entity>().handle();
            entity
                .tick()
                .expect("Quent preflight emission should succeed");
            entity
                .tick()
                .expect("Quent multi-event should be repeatable");
            measure(calls, || {
                black_box(entity.tick()).expect("Quent emission should succeed");
            })
        }
        "tracing" => {
            match output_mode {
                OutputMode::Noop => {
                    tracing::subscriber::set_global_default(Registry::default().with(DiscardLayer))
                        .expect("global tracing subscriber should be unset");
                }
                OutputMode::Text => {
                    let output = std::fs::File::create(export_path("tracing.log"))
                        .expect("tracing output should be created");
                    tracing::subscriber::set_global_default(
                        Registry::default().with(
                            tracing_subscriber::fmt::layer()
                                .with_ansi(false)
                                .with_writer(output),
                        ),
                    )
                    .expect("global tracing subscriber should be unset");
                }
            }
            let entity_id = entity_id();
            tracing::event!(name: "tick", tracing::Level::INFO, entity_id = entity_id.as_str());
            measure(calls, || {
                tracing::event!(name: "tick", tracing::Level::INFO, entity_id = entity_id.as_str());
            })
        }
        "log" => {
            let logger = Box::leak(Box::new(BenchmarkLog {
                writer: match output_mode {
                    OutputMode::Noop => None,
                    OutputMode::Text => Some(text_writer("log.log")),
                },
            }));
            log::set_logger(logger).expect("global log facade should be unset");
            log::set_max_level(log::LevelFilter::Trace);
            let entity_id = entity_id();
            log::log!(target: "quent-bench", log::Level::Info, entity_id = entity_id.as_str(); "");
            let average = measure(calls, || {
                log::log!(target: "quent-bench", log::Level::Info, entity_id = entity_id.as_str(); "");
            });
            log::logger().flush();
            average
        }
        "slog" => {
            let slog = slog::Logger::root(
                BenchmarkSlogDrain {
                    writer: match output_mode {
                        OutputMode::Noop => None,
                        OutputMode::Text => Some(text_writer("slog.log")),
                    },
                }
                .fuse(),
                slog::o!("entity_id" => entity_id()),
            );
            slog::info!(slog, "");
            measure(calls, || slog::info!(slog, ""))
        }
        "opentelemetry" => {
            let entity_id = entity_id();
            let provider = SdkLoggerProvider::builder()
                .with_simple_exporter(BenchmarkLogExporter {
                    writer: match output_mode {
                        OutputMode::Noop => None,
                        OutputMode::Text => Some(text_writer("opentelemetry.log")),
                    },
                })
                .build();
            let logger = provider.logger("quent-bench");
            let mut preflight = logger.create_log_record();
            preflight.add_attribute("entity.id", entity_id.clone());
            logger.emit(preflight);
            let average = measure(calls, || {
                let mut record = logger.create_log_record();
                record.add_attribute("entity.id", entity_id.clone());
                logger.emit(record);
            });
            drop(logger);
            provider
                .shutdown()
                .expect("OpenTelemetry provider should shut down");
            average
        }
        _ => panic!("unknown implementation: {implementation}"),
    }
}

fn main() {
    let mut calls = None;
    let mut implementation = None;
    let mut output_mode = None;
    let mut output = None;
    let mut arguments = std::env::args().skip(1);
    if arguments.len() == 0 {
        return;
    }
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--bench" => {}
            "--implementation" => {
                implementation = Some(arguments.next().expect("--implementation requires a value"));
            }
            "--output-mode" => {
                output_mode = Some(OutputMode::parse(
                    &arguments.next().expect("--output-mode requires a value"),
                ));
            }
            "--calls" => {
                calls = Some(
                    arguments
                        .next()
                        .expect("--calls requires a value")
                        .parse::<u64>()
                        .expect("--calls must be an integer"),
                );
            }
            "--output" => {
                output = Some(PathBuf::from(
                    arguments.next().expect("--output requires a path"),
                ));
            }
            _ => panic!("unknown argument: {argument}"),
        }
    }
    let calls = calls.expect("--calls is required");
    assert!(calls > 0, "--calls must be positive");
    let implementation = implementation.expect("--implementation is required");
    let output_mode = output_mode.expect("--output-mode is required");
    let output = output.expect("--output is required");
    let average_ns = benchmark(&implementation, output_mode, calls);
    std::fs::write(
        output,
        serde_json::to_string_pretty(&serde_json::json!({
            "benchmarks": [{
                "name": implementation,
                "output_mode": output_mode.name(),
                "average_ns": average_ns,
                "calls": calls,
            }]
        }))
        .expect("result should serialize")
            + "\n",
    )
    .expect("result should be written");
}
