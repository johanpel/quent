// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Throughput comparison: quent vs OpenTelemetry 0.32 vs `tracing`, each as it
//! is natively built.
//!
//! Per (signal, backend, attribute-count, threads) cell, spam `BENCH_OPS`
//! operations flat out across the producer threads, then report:
//! - `offered`  the raw API-call rate (ops / emit time), one op = one log or one
//!   span (a quent FSM span emits two events but still counts as one span).
//! - `tput`     sustained throughput. quent is lossless -- every op is delivered,
//!   measured by emitting then dropping the context (a blocking flush) and
//!   dividing ops by total time INCLUDING the flush. OTel/`tracing` are lossy
//!   (bounded queue) -- goodput is counted at the sink, the rest dropped.
//!
//! Backends (native to each library): quent {noop, ndjson, msgpack, postcard,
//! grpc collector}; OTel {noop, OTLP/gRPC}; tracing {JSON file}. gRPC/OTLP go to
//! a live in-process receiver; the tracing writer counts newlines.
//!
//! Knobs: `BENCH_OPS` (ops/cell, default 2,000,000), `BENCH_ATTRS`,
//! `BENCH_THREADS` (comma lists), `BENCH_REPS`, `BENCH_CSV` (raw CSV path),
//! `BENCH_JSON` (self-describing JSON path).

use std::fs::File;
use std::net::TcpListener as StdTcpListener;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use quent_dynamic_attributes::{DynamicAttribute, DynamicAttributes};
use quent_collector::{CollectorSink, server::CollectorService};
use quent_collector_proto::collector_server::CollectorServer;
use quent_model::io::{
    CollectorExporterOptions, ExporterOptions, FileSystemExporterOptions, FileSystemFormat,
};
use quent_model::{entity, fsm, instrumentation, model, state};

use opentelemetry::KeyValue;
use opentelemetry::logs::{LogRecord, Logger, LoggerProvider, Severity};
use opentelemetry::trace::{Span as OtelSpan, Tracer, TracerProvider};
use opentelemetry_otlp::{
    LogExporter as OtlpLogExporter, SpanExporter as OtlpSpanExporter, WithExportConfig,
};
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::logs::{LogBatch, SdkLoggerProvider};
use opentelemetry_sdk::trace::{SdkTracerProvider, SpanData};

use opentelemetry_proto::tonic::collector::logs::v1::{
    ExportLogsServiceRequest, ExportLogsServiceResponse,
    logs_service_server::{LogsService, LogsServiceServer},
};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
    trace_service_server::{TraceService, TraceServiceServer},
};

use tokio::runtime::Runtime;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server as GrpcServer;
use tonic::{Request, Response, Status};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{filter, fmt};
use uuid::Uuid;

// The bench model: a single-event log entity and a one-state, two-event span FSM.
entity! {
    Root: ResourceGroup<Root = true> {}
}
entity! {
    LogEvent {
        attributes: { attrs: DynamicAttributes },
    }
}
state! {
    Active {
        attributes: { attrs: DynamicAttributes },
    }
}
fsm! {
    Span {
        states: { active: Active },
        entry: active,
        exit_from: { active },
        transitions: {},
    }
}
model! {
    name: Bench,
    root: Root,
    entities: { LogEvent, Span },
}
instrumentation!(Bench);

// -------- attribute construction (cycles string/i64/f64) --------

fn mk_keys(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("k{i}")).collect()
}

fn quent_attrs(keys: &[String]) -> DynamicAttributes {
    let mut v = Vec::with_capacity(keys.len());
    for (i, k) in keys.iter().enumerate() {
        match i % 3 {
            0 => v.push(DynamicAttribute::string(k.clone(), "val")),
            1 => v.push(DynamicAttribute::i64(k.clone(), i as i64)),
            _ => v.push(DynamicAttribute::f64(k.clone(), i as f64)),
        }
    }
    v.into()
}

/// Mixed-type attribute payload for `tracing` (recorded as one Debug field, since
/// tracing callsites have static fields and cannot take a dynamic attribute count).
// Fields are read only through the derived `Debug` (which dead-code analysis ignores).
#[derive(Debug)]
#[allow(dead_code)]
enum AttrVal {
    S(&'static str),
    I(i64),
    F(f64),
}

fn tracing_attrs(keys: &[String]) -> Vec<(String, AttrVal)> {
    keys.iter()
        .enumerate()
        .map(|(i, k)| {
            let v = match i % 3 {
                0 => AttrVal::S("val"),
                1 => AttrVal::I(i as i64),
                _ => AttrVal::F(i as f64),
            };
            (k.clone(), v)
        })
        .collect()
}

/// A `Write` wrapper counting delivered records (one per newline) so `tracing`'s
/// off-thread appender has a countable sink, like the other backends.
struct CountingWriter<W> {
    inner: W,
    counter: Arc<AtomicU64>,
}
impl<W: std::io::Write> std::io::Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let lines = buf.iter().filter(|&&b| b == b'\n').count() as u64;
        self.counter.fetch_add(lines, Ordering::Relaxed);
        self.inner.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Format a per-second rate with an SI prefix (k / M / G).
fn si(v: f64) -> String {
    // Thresholds are just below each prefix so values that would render as
    // "1000.0k" promote to "1.00M" instead.
    if v >= 999_999_950.0 {
        format!("{:.2}G", v / 1e9)
    } else if v >= 999_950.0 {
        format!("{:.2}M", v / 1e6)
    } else if v >= 999.95 {
        format!("{:.1}k", v / 1e3)
    } else {
        format!("{v:.0}")
    }
}

// -------- quent collector receiver (decode + drop) --------

struct QuentSink {
    received: Arc<AtomicU64>,
}
impl CollectorSink for QuentSink {
    fn ingest(&self, entity: &str, event: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        use quent_model::EntityEvent;
        if entity == <LogEventEvent as EntityEvent>::NAME {
            let _: quent_model::Event<LogEventEvent> = quent_model::deserialize_event(event)?;
        } else if entity == <SpanEvent as EntityEvent>::NAME {
            let _: quent_model::Event<SpanEvent> = quent_model::deserialize_event(event)?;
        } else {
            return Err(format!("unknown entity stream `{entity}`").into());
        }
        self.received.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

fn start_quent_collector(rt: &Runtime, received: Arc<AtomicU64>) -> http::Uri {
    let listener = StdTcpListener::bind("127.0.0.1:0").unwrap();
    let uri: http::Uri = format!("http://{}", listener.local_addr().unwrap())
        .parse()
        .unwrap();
    listener.set_nonblocking(true).unwrap();
    rt.spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        let incoming = TcpListenerStream::new(listener);
        let service = CollectorService::new(move |_id| {
            Ok::<_, String>(QuentSink {
                received: received.clone(),
            })
        });
        let _ = GrpcServer::builder()
            .add_service(CollectorServer::new(service))
            .serve_with_incoming(incoming)
            .await;
    });
    uri
}

// -------- OTLP receiver (logs + traces, counts records) --------

#[derive(Clone)]
struct OtlpReceiver {
    logs: Arc<AtomicU64>,
    spans: Arc<AtomicU64>,
}

#[tonic::async_trait]
impl LogsService for OtlpReceiver {
    async fn export(
        &self,
        req: Request<ExportLogsServiceRequest>,
    ) -> Result<Response<ExportLogsServiceResponse>, Status> {
        let n: u64 = req
            .get_ref()
            .resource_logs
            .iter()
            .flat_map(|rl| rl.scope_logs.iter())
            .map(|sl| sl.log_records.len() as u64)
            .sum();
        self.logs.fetch_add(n, Ordering::Relaxed);
        Ok(Response::new(ExportLogsServiceResponse::default()))
    }
}

#[tonic::async_trait]
impl TraceService for OtlpReceiver {
    async fn export(
        &self,
        req: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, Status> {
        let n: u64 = req
            .get_ref()
            .resource_spans
            .iter()
            .flat_map(|rs| rs.scope_spans.iter())
            .map(|ss| ss.spans.len() as u64)
            .sum();
        self.spans.fetch_add(n, Ordering::Relaxed);
        Ok(Response::new(ExportTraceServiceResponse::default()))
    }
}

fn start_otlp_receiver(rt: &Runtime, receiver: OtlpReceiver) -> String {
    let listener = StdTcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let logs_svc = receiver.clone();
    let trace_svc = receiver;
    rt.spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        let incoming = TcpListenerStream::new(listener);
        let _ = GrpcServer::builder()
            .add_service(LogsServiceServer::new(logs_svc))
            .add_service(TraceServiceServer::new(trace_svc))
            .serve_with_incoming(incoming)
            .await;
    });
    endpoint
}

// -------- drop exporters (OTel noop floor) --------

#[derive(Debug)]
struct DropLogExporter;
impl opentelemetry_sdk::logs::LogExporter for DropLogExporter {
    async fn export(&self, _batch: LogBatch<'_>) -> OTelSdkResult {
        Ok(())
    }
}

#[derive(Debug)]
struct DropSpanExporter;
impl opentelemetry_sdk::trace::SpanExporter for DropSpanExporter {
    async fn export(&self, _batch: Vec<SpanData>) -> OTelSdkResult {
        Ok(())
    }
}

// -------- harness --------

#[derive(Clone, Copy)]
enum Signal {
    Log,
    Span,
}

struct Row {
    label: String,
    n: usize,
    threads: usize,
    offered_recs_s: f64,
    /// Sustained throughput: quent = lossless (records / time incl. flush);
    /// OTel/tracing = goodput (delivered/s). `None` for uncounted floors.
    tput_recs_s: Option<f64>,
    loss_pct: Option<f64>,
}

/// Wait until `counter` stops advancing (pipeline drained) so the next cell's
/// sample starts clean. Bounded so a stuck pipeline cannot hang the run.
/// Spam `k` ops flat out across `threads`, then drain to a stall, and report
/// (offered ops/s = k / emit time, goodput ops/s = delivered / total time incl.
/// drain). One op = one log or one span. For lossy backends (OTel, tracing): the
/// caller emits as fast as it can, the bounded queue sheds the excess, and
/// goodput is what actually reaches the sink. `None` goodput = no counted sink.
fn measure_goodput(
    threads: usize,
    k: u64,
    counter: Option<&AtomicU64>,
    factory: &dyn Fn() -> Box<dyn FnMut() + Send>,
) -> (f64, Option<f64>) {
    let per = (k / threads as u64).max(1);
    let total = per * threads as u64;
    let r0 = counter.map_or(0, |c| c.load(Ordering::Relaxed));
    let t0 = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..threads {
            let mut op = factory();
            s.spawn(move || {
                for _ in 0..per {
                    op();
                }
            });
        }
    });
    let emit_t = t0.elapsed().as_secs_f64();

    let goodput = counter.map(|c| {
        // Wait until everything is delivered; else until delivery stops (a lossy
        // queue dropped the rest); else a long safety timeout. Rate is over the
        // time to the LAST delivery, so the settle window does not dilute it.
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut last = c.load(Ordering::Relaxed);
        let mut last_change = Instant::now();
        loop {
            let cur = c.load(Ordering::Relaxed);
            if cur != last {
                last = cur;
                last_change = Instant::now();
            }
            if cur - r0 >= total || Instant::now() >= deadline {
                break; // everything delivered, or timed out
            }
            if last_change.elapsed() >= Duration::from_millis(500) {
                break; // delivery has stopped -- the rest was dropped
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let total_t = last_change.duration_since(t0).as_secs_f64().max(emit_t);
        (c.load(Ordering::Relaxed) - r0) as f64 / total_t
    });

    (total as f64 / emit_t, goodput)
}

/// Lossless throughput: emit `k` ops across `threads` flat out, then drop the
/// observer + context (a blocking flush/drain). Returns (offered = emit-only
/// rate, throughput = records / total time INCLUDING the flush). For quent,
/// whose unbounded pipeline delivers every event, so the honest max folds in the
/// cost to fully flush.
fn measure_lossless(
    backend: Option<ExporterOptions>,
    signal: Signal,
    keys: &Arc<Vec<String>>,
    threads: usize,
    k: u64,
) -> (f64, f64) {
    let ctx = BenchContext::try_new(backend).unwrap();
    let per = (k / threads as u64).max(1);
    let total = per * threads as u64;
    let t0 = Instant::now();
    // One op = one span lifecycle, even though quent's FSM span emits two events
    // (start + end): the extra pipeline work shows up as lower per-span throughput
    // rather than a doubled count, so it stays comparable to an OTel span.
    let emit_t = match signal {
        Signal::Log => {
            let obs = ctx.log_event_observer();
            std::thread::scope(|s| {
                for _ in 0..threads {
                    let o = obs.clone();
                    let keys = keys.clone();
                    s.spawn(move || {
                        for _ in 0..per {
                            o.log_event(Uuid::now_v7(), quent_attrs(&keys));
                        }
                    });
                }
            });
            let emit_t = t0.elapsed().as_secs_f64();
            drop(obs); // last stream handle → drain + flush (blocking)
            emit_t
        }
        Signal::Span => {
            let obs = ctx.span_observer();
            std::thread::scope(|s| {
                for _ in 0..threads {
                    let o = obs.clone();
                    let keys = keys.clone();
                    s.spawn(move || {
                        for _ in 0..per {
                            let mut span = o.active(Uuid::now_v7(), "span", quent_attrs(&keys));
                            span.exit();
                        }
                    });
                }
            });
            let emit_t = t0.elapsed().as_secs_f64();
            drop(obs);
            emit_t
        }
    };
    // The flush happens when the last stream handle drops -- the context still
    // holds one (observers are clones), so the drain runs on `drop(ctx)`. Time
    // the total AFTER it so throughput folds in the full flush.
    drop(ctx);
    let total_t = t0.elapsed().as_secs_f64();
    let ops = total as f64;
    (ops / emit_t, ops / total_t)
}

fn otel_log_op<L: Logger>(logger: &L, keys: &[String]) {
    let mut rec = logger.create_log_record();
    rec.set_severity_number(Severity::Info);
    for (i, k) in keys.iter().enumerate() {
        match i % 3 {
            0 => rec.add_attribute(k.clone(), "val"),
            1 => rec.add_attribute(k.clone(), i as i64),
            _ => rec.add_attribute(k.clone(), i as f64),
        }
    }
    logger.emit(rec);
}

fn otel_span_op<T: Tracer>(tracer: &T, keys: &[String]) {
    let mut span = tracer.start("op");
    for (i, k) in keys.iter().enumerate() {
        let kv = match i % 3 {
            0 => KeyValue::new(k.clone(), "val"),
            1 => KeyValue::new(k.clone(), i as i64),
            _ => KeyValue::new(k.clone(), i as f64),
        };
        span.set_attribute(kv);
    }
    span.end();
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// A comma-separated list of usize (e.g. thread counts to sweep).
fn env_usizes(key: &str, default: Vec<usize>) -> Vec<usize> {
    std::env::var(key)
        .ok()
        .map(|s| {
            s.split(',')
                .filter_map(|x| x.trim().parse().ok())
                .collect::<Vec<usize>>()
        })
        .filter(|v| !v.is_empty())
        .unwrap_or(default)
}

fn env_attrs() -> Vec<usize> {
    std::env::var("BENCH_ATTRS")
        .ok()
        .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![0, 64])
}

fn fs_opts(format: FileSystemFormat, dir: &Path) -> ExporterOptions {
    ExporterOptions::FileSystem(FileSystemExporterOptions::new(format, dir.to_path_buf()))
}

/// Store a completed row and print a live progress line to stderr.
fn record(rows: &mut Vec<Row>, idx: &mut usize, total: usize, row: Row) {
    *idx += 1;
    let tput = row.tput_recs_s.map_or("-".to_string(), si);
    let loss = row.loss_pct.map_or("-".to_string(), |l| format!("{l:.0}%"));
    eprintln!(
        "[{:>4}/{total}] {:<18} attrs={:<3} thr={:<2} offered={:<9} tput={tput:<9} loss={loss}",
        *idx,
        row.label,
        row.n,
        row.threads,
        si(row.offered_recs_s),
    );
    rows.push(row);
}

/// Average `reps` windows of a cell into (offered/s, delivered/s).
fn average(reps: usize, mut f: impl FnMut() -> (f64, Option<f64>)) -> (f64, Option<f64>) {
    let (mut off, mut del, mut has) = (0.0, 0.0, false);
    for _ in 0..reps {
        let (o, d) = f();
        off += o;
        if let Some(d) = d {
            del += d;
            has = true;
        }
    }
    (off / reps as f64, has.then(|| del / reps as f64))
}

fn main() {
    let attrs = env_attrs();
    let thread_list = env_usizes("BENCH_THREADS", vec![1, 4, 8]);
    let reps = env_usize("BENCH_REPS", 1).max(1);
    let k = env_usize("BENCH_OPS", 2_000_000) as u64;

    let rt = Runtime::new().unwrap();
    let quent_recv = Arc::new(AtomicU64::new(0));
    let quent_addr = start_quent_collector(&rt, quent_recv.clone());
    let otlp_logs = Arc::new(AtomicU64::new(0));
    let otlp_spans = Arc::new(AtomicU64::new(0));
    let otlp_endpoint = start_otlp_receiver(
        &rt,
        OtlpReceiver {
            logs: otlp_logs.clone(),
            spans: otlp_spans.clone(),
        },
    );

    let ndjson_dir = tempfile::tempdir().unwrap();
    let msgpack_dir = tempfile::tempdir().unwrap();
    let postcard_dir = tempfile::tempdir().unwrap();
    let otel_dir = tempfile::tempdir().unwrap();

    // OTel providers (persistent; batch threads live for the whole run).
    let (log_noop, log_grpc, span_noop, span_grpc) = rt.block_on(async {
        let log_noop = SdkLoggerProvider::builder()
            .with_batch_exporter(DropLogExporter)
            .build();
        let log_grpc = SdkLoggerProvider::builder()
            .with_batch_exporter(
                OtlpLogExporter::builder()
                    .with_tonic()
                    .with_endpoint(otlp_endpoint.clone())
                    .build()
                    .unwrap(),
            )
            .build();
        let span_noop = SdkTracerProvider::builder()
            .with_batch_exporter(DropSpanExporter)
            .build();
        let span_grpc = SdkTracerProvider::builder()
            .with_batch_exporter(
                OtlpSpanExporter::builder()
                    .with_tonic()
                    .with_endpoint(otlp_endpoint.clone())
                    .build()
                    .unwrap(),
            )
            .build();
        (log_noop, log_grpc, span_noop, span_grpc)
    });

    // tracing: JSON to a file via a background (non_blocking) writer, counted by
    // newlines. Filtered to the `bench_trace` target so quent/OTel internal logs
    // are neither written nor counted nor charged for.
    let trace_delivered = Arc::new(AtomicU64::new(0));
    let trace_file = File::create(otel_dir.path().join("tracing.jsonl")).unwrap();
    let (trace_nb, trace_guard) = tracing_appender::non_blocking(CountingWriter {
        inner: trace_file,
        counter: trace_delivered.clone(),
    });
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .json()
                .with_span_events(FmtSpan::CLOSE)
                .with_writer(trace_nb)
                .with_filter(
                    filter::Targets::new().with_target("bench_trace", tracing::Level::INFO),
                ),
        )
        .init();

    let cells = attrs.len() * thread_list.len() * 16;
    eprintln!(
        "max throughput: threads={thread_list:?}, {reps} rep(s), {k} ops/cell (spammed flat out), {} cell-runs...",
        cells * reps
    );

    // quent backend options; a fresh context per lossless cell (drop = flush).
    let quent_backends: Vec<(&str, Option<ExporterOptions>)> = vec![
        ("noop", None),
        (
            "ndjson",
            Some(fs_opts(FileSystemFormat::Ndjson, ndjson_dir.path())),
        ),
        (
            "msgpack",
            Some(fs_opts(FileSystemFormat::Msgpack, msgpack_dir.path())),
        ),
        (
            "postcard",
            Some(fs_opts(FileSystemFormat::Postcard, postcard_dir.path())),
        ),
        (
            "grpc",
            Some(ExporterOptions::Collector(CollectorExporterOptions::new(
                quent_addr.clone(),
            ))),
        ),
    ];

    let mut rows = Vec::new();
    let mut idx = 0usize;
    for &n in &attrs {
        let keys = Arc::new(mk_keys(n));
        for &threads in &thread_list {
            // quent: lossless — emit k, drop (flush); throughput folds in the flush.
            for (bname, opts) in &quent_backends {
                // noop discards everything (no sink), so it has no delivered
                // throughput -- only the caller floor, like otel-noop.
                let is_noop = opts.is_none();
                for (signal, sl) in [(Signal::Log, "log"), (Signal::Span, "span")] {
                    let (off, tput) = average(reps, || {
                        let (o, t) = measure_lossless(opts.clone(), signal, &keys, threads, k);
                        (o, Some(t))
                    });
                    record(
                        &mut rows,
                        &mut idx,
                        cells,
                        Row {
                            label: format!("quent-{sl}/{bname}"),
                            n,
                            threads,
                            offered_recs_s: off,
                            tput_recs_s: if is_noop { None } else { tput },
                            loss_pct: if is_noop { None } else { Some(0.0) },
                        },
                    );
                }
            }

            // OTel + tracing: lossy — saturate, measure goodput (delivered/s).
            let otel_log = [("noop", None::<&AtomicU64>), ("grpc", Some(&*otlp_logs))];
            for (bname, counter) in otel_log {
                let (off, good) = average(reps, || {
                    let logger = match bname {
                        "noop" => log_noop.logger("bench"),
                        _ => log_grpc.logger("bench"),
                    };
                    let keys = keys.clone();
                    let factory = || {
                        let logger = logger.clone();
                        let keys = keys.clone();
                        Box::new(move || otel_log_op(&logger, &keys)) as Box<dyn FnMut() + Send>
                    };
                    measure_goodput(threads, k, counter, &factory)
                });
                record(
                    &mut rows,
                    &mut idx,
                    cells,
                    Row {
                        label: format!("otel-log/{bname}"),
                        n,
                        threads,
                        offered_recs_s: off,
                        tput_recs_s: good,
                        loss_pct: good.map(|g| (1.0 - g / off).max(0.0) * 100.0),
                    },
                );
            }

            let otel_span = [("noop", None::<&AtomicU64>), ("grpc", Some(&*otlp_spans))];
            for (bname, counter) in otel_span {
                let (off, good) = average(reps, || {
                    let tracer = match bname {
                        "noop" => span_noop.tracer("bench"),
                        _ => span_grpc.tracer("bench"),
                    };
                    let keys = keys.clone();
                    let factory = || {
                        let tracer = tracer.clone();
                        let keys = keys.clone();
                        Box::new(move || otel_span_op(&tracer, &keys)) as Box<dyn FnMut() + Send>
                    };
                    measure_goodput(threads, k, counter, &factory)
                });
                record(
                    &mut rows,
                    &mut idx,
                    cells,
                    Row {
                        label: format!("otel-span/{bname}"),
                        n,
                        threads,
                        offered_recs_s: off,
                        tput_recs_s: good,
                        loss_pct: good.map(|g| (1.0 - g / off).max(0.0) * 100.0),
                    },
                );
            }

            // tracing: JSON to file. NB: serializes on the CALLER thread.
            let (off, good) = average(reps, || {
                let keys = keys.clone();
                let factory = || {
                    let keys = keys.clone();
                    Box::new(move || {
                        tracing::info!(target: "bench_trace", payload = ?tracing_attrs(&keys));
                    }) as Box<dyn FnMut() + Send>
                };
                measure_goodput(threads, k, Some(&trace_delivered), &factory)
            });
            record(
                &mut rows,
                &mut idx,
                cells,
                Row {
                    label: "tracing-log/file".to_string(),
                    n,
                    threads,
                    offered_recs_s: off,
                    tput_recs_s: good,
                    loss_pct: good.map(|g| (1.0 - g / off).max(0.0) * 100.0),
                },
            );

            let (off, good) = average(reps, || {
                let keys = keys.clone();
                let factory = || {
                    let keys = keys.clone();
                    Box::new(move || {
                        let span = tracing::info_span!(target: "bench_trace", "op", payload = ?tracing_attrs(&keys));
                        let _e = span.enter();
                    }) as Box<dyn FnMut() + Send>
                };
                measure_goodput(threads, k, Some(&trace_delivered), &factory)
            });
            record(
                &mut rows,
                &mut idx,
                cells,
                Row {
                    label: "tracing-span/file".to_string(),
                    n,
                    threads,
                    offered_recs_s: off,
                    tput_recs_s: good,
                    loss_pct: good.map(|g| (1.0 - g / off).max(0.0) * 100.0),
                },
            );
        }
    }

    rows.sort_by(|a, b| {
        a.n.cmp(&b.n)
            .then(a.threads.cmp(&b.threads))
            .then_with(|| a.label.cmp(&b.label))
    });
    println!();
    println!("columns (all rates are operations/second; 1 op = one log, or one span --");
    println!("a quent FSM span emits 2 events internally but still counts as one span):");
    println!("  offered/s  raw caller API-call rate (before any flush or drop)");
    println!("  tput/s     sustained throughput -- quent: LOSSLESS, ops / time incl. the");
    println!("             full flush on drop;  OTel/tracing: GOODPUT, delivered/s (rest dropped)");
    println!("  loss%      fraction dropped (0% for quent -- lossless; '-' = uncounted floor)");
    println!();
    println!(
        "{:<18} {:>5} {:>4} {:>13} {:>13} {:>7}",
        "variant", "attrs", "thr", "offered/s", "tput/s", "loss%"
    );
    println!("{}", "-".repeat(66));
    for r in &rows {
        let tput = r.tput_recs_s.map_or("-".to_string(), si);
        let loss = r.loss_pct.map_or("-".to_string(), |l| format!("{l:.0}%"));
        println!(
            "{:<18} {:>5} {:>4} {:>13} {:>13} {:>7}",
            r.label,
            r.n,
            r.threads,
            si(r.offered_recs_s),
            tput,
            loss
        );
    }

    // PEAK: best throughput per (variant, attrs) across the thread sweep.
    let mut peak: std::collections::BTreeMap<(usize, String), (f64, usize)> =
        std::collections::BTreeMap::new();
    for r in &rows {
        let v = r.tput_recs_s.unwrap_or(r.offered_recs_s);
        let e = peak.entry((r.n, r.label.clone())).or_insert((0.0, 0));
        if v > e.0 {
            *e = (v, r.threads);
        }
    }
    println!();
    println!("PEAK throughput across the thread sweep:");
    println!(
        "{:<18} {:>5} {:>13} {:>6}",
        "variant", "attrs", "tput/s", "@thr"
    );
    println!("{}", "-".repeat(45));
    for ((n, label), (v, thr)) in &peak {
        println!("{label:<18} {n:>5} {:>13} {thr:>6}", si(*v));
    }

    // Optional raw CSV for plotting (BENCH_CSV=path).
    if let Ok(path) = std::env::var("BENCH_CSV") {
        let mut csv = String::from("variant,attrs,threads,offered_ops_s,tput_ops_s,loss_pct\n");
        for r in &rows {
            let tput = r.tput_recs_s.map_or(String::new(), |v| v.to_string());
            let loss = r.loss_pct.map_or(String::new(), |v| v.to_string());
            csv.push_str(&format!(
                "{},{},{},{},{tput},{loss}\n",
                r.label, r.n, r.threads, r.offered_recs_s
            ));
        }
        match std::fs::write(&path, csv) {
            Ok(()) => eprintln!("wrote CSV to {path}"),
            Err(e) => eprintln!("failed to write CSV {path}: {e}"),
        }
    }

    // Optional self-describing JSON (BENCH_JSON=path) for agentic consumers.
    if let Ok(path) = std::env::var("BENCH_JSON") {
        let num = |o: Option<f64>| o.map_or("null".to_string(), |v| v.to_string());
        let items: Vec<String> = rows
            .iter()
            .map(|r| {
                let family = r.label.split(['-', '/']).next().unwrap_or(&r.label);
                format!(
                    "{{\"variant\":\"{}\",\"family\":\"{}\",\"attrs\":{},\"threads\":{},\
                     \"offered_ops_s\":{},\"throughput_ops_s\":{},\"loss_pct\":{}}}",
                    r.label,
                    family,
                    r.n,
                    r.threads,
                    r.offered_recs_s,
                    num(r.tput_recs_s),
                    num(r.loss_pct),
                )
            })
            .collect();
        let meta = format!(
            "{{\"unit\":\"operations/second (1 op = one log or one span)\",\"ops_per_cell\":{k},\
             \"metrics\":{{\
             \"offered_ops_s\":\"raw API-call rate = ops / emit time\",\
             \"throughput_ops_s\":\"sustained rate. quent: lossless, ops / full time to flush+deliver everything. otel/tracing: goodput = delivered/s, the rest dropped. null = no counted sink (noop)\",\
             \"loss_pct\":\"percent of offered ops that never reached the sink; 0 for quent (lossless); null = uncounted (noop)\"}},\
             \"note\":\"offered > throughput means the caller outran the pipeline; for quent (family=quent) the unbounded queue simply drains (0% loss), for otel/tracing the gap is dropped\"}}"
        );
        let json = format!(
            "{{\n  \"meta\": {meta},\n  \"results\": [\n    {}\n  ]\n}}\n",
            items.join(",\n    ")
        );
        match std::fs::write(&path, json) {
            Ok(()) => eprintln!("wrote JSON to {path}"),
            Err(e) => eprintln!("failed to write JSON {path}: {e}"),
        }
    }

    std::mem::forget(trace_guard);
    std::mem::forget(log_noop);
    std::mem::forget(log_grpc);
    std::mem::forget(span_noop);
    std::mem::forget(span_grpc);
    std::mem::forget(rt);
    std::mem::forget(ndjson_dir);
    std::mem::forget(msgpack_dir);
    std::mem::forget(postcard_dir);
    std::mem::forget(otel_dir);
}
