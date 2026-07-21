// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Throughput comparison: quent vs OpenTelemetry 0.32 vs `tracing`, each as it
//! is natively built.
//!
//! Per (signal, backend, attribute-count, threads) cell, spam `BENCH_OPS`
//! operations flat out across the producer threads, then report:
//! - `offered`  the raw API-call rate (ops / emit time), one op = one log or one
//!   span. A quent span emits two transitions on a persistent per-thread FSM.
//! - `tput`     sustained throughput. quent is lossless -- every op is delivered,
//!   measured by emitting then dropping the context (a blocking flush) and
//!   dividing ops by total time INCLUDING the flush. OTel/`tracing` are lossy
//!   (bounded queue) -- goodput is counted at the sink, the rest dropped.
//! - `call_pXX` sampled caller-side operation latency under the same load.
//! - `drain`     time from the last caller operation until the pipeline drains.
//!
//! Backends (native to each library): quent {noop, ndjson, msgpack, postcard,
//! grpc collector}; OTel {noop, OTLP/gRPC}; tracing {JSON file}. gRPC/OTLP go to
//! a live in-process receiver; the tracing writer counts newlines.
//!
//! Knobs: `BENCH_OPS` (ops/cell, default 2,000,000), `BENCH_ATTRS`,
//! `BENCH_THREADS` (comma lists), `BENCH_REPS`, `BENCH_CSV` (raw CSV path),
//! `BENCH_JSON` (self-describing JSON path), `BENCH_LATENCY_EVERY` (caller
//! latency sampling interval, default 1,024 operations).

use std::fs::File;
use std::io::Seek;
use std::net::TcpListener as StdTcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use quent_collector::{CollectorSink, server::CollectorService};
use quent_collector_proto::collector_server::CollectorServer;
use quent_dynamic_attributes::{DynamicAttribute, DynamicAttributes};
use quent_model::io::{
    CollectorExporterOptions, ExporterOptions, FileSystemExporterOptions, FileSystemFormat,
};
use quent_model::{Attributes, entity, fsm, instrumentation, model, state};

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

// The bench model: one persistent log entity and one cyclic span FSM per worker.
entity! {
    Root: ResourceGroup<Root = true> {}
}

#[derive(Debug, Attributes, serde::Deserialize, serde::Serialize)]
pub struct BenchLogRecord {
    attrs: DynamicAttributes,
}

#[derive(Debug, Attributes, serde::Deserialize, serde::Serialize)]
pub struct LogClosed;

entity! {
    LogEvent {
        events: {
            record: BenchLogRecord,
            closed: LogClosed,
        },
    }
}

state! {
    Idle {}
}

state! {
    Active {
        attributes: { attrs: DynamicAttributes },
    }
}

fsm! {
    Span {
        states: { idle: Idle, active: Active },
        entry: idle,
        exit_from: { idle },
        transitions: { idle => active, active => idle },
    }
}

model! {
    name: Bench,
    root: Root,
    entities: { LogEvent, Span },
}
instrumentation!(Bench);

// -------- attribute construction (cycles string/i64/f64) --------

fn mk_keys(n: usize) -> Vec<&'static str> {
    const KEYS: [&str; 64] = [
        "k0", "k1", "k2", "k3", "k4", "k5", "k6", "k7", "k8", "k9", "k10", "k11", "k12", "k13",
        "k14", "k15", "k16", "k17", "k18", "k19", "k20", "k21", "k22", "k23", "k24", "k25", "k26",
        "k27", "k28", "k29", "k30", "k31", "k32", "k33", "k34", "k35", "k36", "k37", "k38", "k39",
        "k40", "k41", "k42", "k43", "k44", "k45", "k46", "k47", "k48", "k49", "k50", "k51", "k52",
        "k53", "k54", "k55", "k56", "k57", "k58", "k59", "k60", "k61", "k62", "k63",
    ];
    KEYS[..n].to_vec()
}

fn quent_attrs(keys: &[&'static str]) -> DynamicAttributes {
    let mut attrs = DynamicAttributes::new();
    for (i, k) in keys.iter().enumerate() {
        match i % 3 {
            0 => attrs.add(DynamicAttribute::string(*k, "val")),
            1 => attrs.add(DynamicAttribute::i64(*k, i as i64)),
            _ => attrs.add(DynamicAttribute::f64(*k, i as f64)),
        }
    }
    attrs
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

fn tracing_attrs(keys: &[&'static str]) -> Vec<(&'static str, AttrVal)> {
    keys.iter()
        .enumerate()
        .map(|(i, k)| {
            let v = match i % 3 {
                0 => AttrVal::S("val"),
                1 => AttrVal::I(i as i64),
                _ => AttrVal::F(i as f64),
            };
            (*k, v)
        })
        .collect()
}

/// A `Write` wrapper counting delivered records (one per newline) so `tracing`'s
/// off-thread appender has a countable sink, like the other backends.
struct CountingWriter {
    inner: File,
    counter: Arc<AtomicU64>,
    bytes_written: usize,
}
impl std::io::Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = std::io::Write::write(&mut self.inner, buf)?;
        let lines = buf[..written].iter().filter(|&&b| b == b'\n').count() as u64;
        self.counter.fetch_add(lines, Ordering::Relaxed);
        self.bytes_written += written;
        if self.bytes_written >= 64 * 1024 * 1024 {
            self.inner.set_len(0)?;
            self.inner.seek(std::io::SeekFrom::Start(0))?;
            self.bytes_written = 0;
        }
        Ok(written)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.inner)
    }
}

fn clear_temp_output(path: &Path) {
    for entry in std::fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            std::fs::remove_dir_all(path).unwrap();
        } else {
            std::fs::remove_file(path).unwrap();
        }
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

// -------- OTLP receiver (counts logs and spans) --------

#[derive(Clone)]
struct OtlpReceiver {
    logs: Arc<AtomicU64>,
    spans: Arc<AtomicU64>,
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
    call_p50_ns: Option<u64>,
    call_p95_ns: Option<u64>,
    call_p99_ns: Option<u64>,
    drain_ms: Option<f64>,
}

struct Measurement {
    offered_recs_s: f64,
    delivered_recs_s: Option<f64>,
    drain_ms: Option<f64>,
    call_latency_ns: Vec<u64>,
}

fn timer_overhead_ns() -> u64 {
    let mut samples = Vec::with_capacity(10_000);
    for _ in 0..10_000 {
        let started = Instant::now();
        samples.push(started.elapsed().as_nanos() as u64);
    }
    percentile(&mut samples, 0.5).unwrap()
}

fn percentile(samples: &mut [u64], quantile: f64) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    samples.sort_unstable();
    let index = ((quantile * samples.len() as f64).ceil() as usize)
        .saturating_sub(1)
        .min(samples.len() - 1);
    Some(samples[index])
}

fn run_sampled<F: FnMut()>(
    mut operations: u64,
    sample_every: u64,
    timer_overhead_ns: u64,
    op: &mut F,
) -> Vec<u64> {
    let sample_every = sample_every.max(1);
    let mut samples = Vec::with_capacity(operations.div_ceil(sample_every) as usize);
    while operations > 0 {
        let unmeasured = operations.min(sample_every - 1);
        for _ in 0..unmeasured {
            op();
        }
        operations -= unmeasured;
        if operations == 0 {
            break;
        }
        let started = Instant::now();
        op();
        let elapsed = started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        samples.push(elapsed.saturating_sub(timer_overhead_ns));
        operations -= 1;
    }
    samples
}

fn take_samples(samples: &Mutex<Vec<u64>>) -> Vec<u64> {
    std::mem::take(&mut *samples.lock().unwrap())
}

/// Measures caller rate, delivered rate, caller latency, and pipeline drain time.
///
/// Delivery waits until the counter stops advancing or a 30-second limit expires.
fn measure_goodput(
    threads: usize,
    k: u64,
    counter: Option<&AtomicU64>,
    factory: &dyn Fn() -> Box<dyn FnMut() + Send>,
    sample_every: u64,
    timer_overhead_ns: u64,
) -> Measurement {
    let per = (k / threads as u64).max(1);
    let total = per * threads as u64;
    let r0 = counter.map_or(0, |c| c.load(Ordering::Relaxed));
    let samples = Arc::new(Mutex::new(Vec::new()));
    let t0 = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..threads {
            let mut op = factory();
            let samples = samples.clone();
            s.spawn(move || {
                let local = run_sampled(per, sample_every, timer_overhead_ns, &mut op);
                samples.lock().unwrap().extend(local);
            });
        }
    });
    let emit_end = Instant::now();
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
        let drain_ms = last_change
            .saturating_duration_since(emit_end)
            .as_secs_f64()
            * 1_000.0;
        ((c.load(Ordering::Relaxed) - r0) as f64 / total_t, drain_ms)
    });

    Measurement {
        offered_recs_s: total as f64 / emit_t,
        delivered_recs_s: goodput.map(|(rate, _)| rate),
        drain_ms: goodput.map(|(_, drain_ms)| drain_ms),
        call_latency_ns: take_samples(&samples),
    }
}

/// Measure lossless throughput with one entity per worker.
fn measure_lossless(
    backend: Option<ExporterOptions>,
    signal: Signal,
    keys: &Arc<Vec<&'static str>>,
    threads: usize,
    k: u64,
    sample_every: u64,
    timer_overhead_ns: u64,
) -> Measurement {
    let ctx = BenchContext::try_new(backend).unwrap();
    let per = (k / threads as u64).max(1);
    let total = per * threads as u64;
    let samples = Arc::new(Mutex::new(Vec::new()));
    let t0 = Instant::now();
    let emit_t = match signal {
        Signal::Log => {
            let obs = ctx.log_event_observer();
            std::thread::scope(|s| {
                for _ in 0..threads {
                    let o = obs.clone();
                    let keys = keys.clone();
                    let samples = samples.clone();
                    s.spawn(move || {
                        let entity = o.create(Uuid::new_v4());
                        let mut op = || {
                            entity.record(BenchLogRecord {
                                attrs: quent_attrs(&keys),
                            });
                        };
                        let local = run_sampled(per, sample_every, timer_overhead_ns, &mut op);
                        samples.lock().unwrap().extend(local);
                        entity.closed(LogClosed);
                    });
                }
            });
            let emit_t = t0.elapsed().as_secs_f64();
            drop(obs);
            emit_t
        }
        Signal::Span => {
            let obs = ctx.span_observer();
            std::thread::scope(|s| {
                for _ in 0..threads {
                    let o = obs.clone();
                    let keys = keys.clone();
                    let samples = samples.clone();
                    s.spawn(move || {
                        let mut span = o.idle(Uuid::new_v4());
                        let local = {
                            let mut op = || {
                                span.active("span", quent_attrs(&keys));
                                span.idle();
                            };
                            run_sampled(per, sample_every, timer_overhead_ns, &mut op)
                        };
                        samples.lock().unwrap().extend(local);
                        span.exit();
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
    let emit_end = Instant::now();
    drop(ctx);
    let total_t = t0.elapsed().as_secs_f64();
    let ops = total as f64;
    Measurement {
        offered_recs_s: ops / emit_t,
        delivered_recs_s: Some(ops / total_t),
        drain_ms: Some(emit_end.elapsed().as_secs_f64() * 1_000.0),
        call_latency_ns: take_samples(&samples),
    }
}

fn otel_log_op<L: Logger>(logger: &L, keys: &[&'static str]) {
    let mut rec = logger.create_log_record();
    rec.set_severity_number(Severity::Info);
    for (i, k) in keys.iter().enumerate() {
        match i % 3 {
            0 => rec.add_attribute(*k, "val"),
            1 => rec.add_attribute(*k, i as i64),
            _ => rec.add_attribute(*k, i as f64),
        }
    }
    logger.emit(rec);
}

fn otel_span_op<T: Tracer>(tracer: &T, keys: &[&'static str]) {
    let mut span = tracer.start("op");
    for (i, k) in keys.iter().enumerate() {
        let kv = match i % 3 {
            0 => KeyValue::new(*k, "val"),
            1 => KeyValue::new(*k, i as i64),
            _ => KeyValue::new(*k, i as f64),
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
    let p99 = row.call_p99_ns.map_or("-".to_string(), duration_ns);
    let drain = row.drain_ms.map_or("-".to_string(), duration_ms);
    eprintln!(
        "[{:>4}/{total}] {:<18} attrs={:<3} thr={:<2} offered={:<9} tput={tput:<9} loss={loss:<4} p99={p99:<8} drain={drain}",
        *idx,
        row.label,
        row.n,
        row.threads,
        si(row.offered_recs_s),
    );
    rows.push(row);
}

/// Average rates and drain time across repetitions and combine latency samples.
fn average(reps: usize, mut f: impl FnMut() -> Measurement) -> Measurement {
    let (mut offered, mut delivered, mut drain_ms) = (0.0, 0.0, 0.0);
    let (mut has_delivered, mut has_drain) = (false, false);
    let mut call_latency_ns = Vec::new();
    for _ in 0..reps {
        let measurement = f();
        offered += measurement.offered_recs_s;
        if let Some(value) = measurement.delivered_recs_s {
            delivered += value;
            has_delivered = true;
        }
        if let Some(value) = measurement.drain_ms {
            drain_ms += value;
            has_drain = true;
        }
        call_latency_ns.extend(measurement.call_latency_ns);
    }
    Measurement {
        offered_recs_s: offered / reps as f64,
        delivered_recs_s: has_delivered.then(|| delivered / reps as f64),
        drain_ms: has_drain.then(|| drain_ms / reps as f64),
        call_latency_ns,
    }
}

fn latency_percentiles(mut samples: Vec<u64>) -> (Option<u64>, Option<u64>, Option<u64>) {
    if samples.is_empty() {
        return (None, None, None);
    }
    samples.sort_unstable();
    let at = |quantile: f64| {
        let index = ((quantile * samples.len() as f64).ceil() as usize)
            .saturating_sub(1)
            .min(samples.len() - 1);
        Some(samples[index])
    };
    (at(0.50), at(0.95), at(0.99))
}

fn duration_ns(ns: u64) -> String {
    if ns >= 1_000_000_000 {
        format!("{:.2}s", ns as f64 / 1_000_000_000.0)
    } else if ns >= 1_000_000 {
        format!("{:.2}ms", ns as f64 / 1_000_000.0)
    } else if ns >= 1_000 {
        format!("{:.1}us", ns as f64 / 1_000.0)
    } else {
        format!("{ns}ns")
    }
}

fn duration_ms(ms: f64) -> String {
    duration_ns((ms * 1_000_000.0).min(u64::MAX as f64) as u64)
}

fn main() {
    let attrs = env_attrs();
    let thread_list = env_usizes("BENCH_THREADS", vec![1, 4, 8]);
    let reps = env_usize("BENCH_REPS", 1).max(1);
    let k = env_usize("BENCH_OPS", 2_000_000) as u64;
    let latency_every = env_usize("BENCH_LATENCY_EVERY", 1024).max(1) as u64;
    let timer_overhead_ns = timer_overhead_ns();

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
        bytes_written: 0,
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
        "max throughput: threads={thread_list:?}, {reps} rep(s), {k} ops/cell, latency sample 1/{latency_every}, {} cell-runs...",
        cells * reps
    );

    // quent backend options; a fresh context per lossless cell (drop = flush).
    let quent_backends: Vec<(&str, Option<ExporterOptions>, Option<&Path>)> = vec![
        ("noop", None, None),
        (
            "ndjson",
            Some(fs_opts(FileSystemFormat::Ndjson, ndjson_dir.path())),
            Some(ndjson_dir.path()),
        ),
        (
            "msgpack",
            Some(fs_opts(FileSystemFormat::Msgpack, msgpack_dir.path())),
            Some(msgpack_dir.path()),
        ),
        (
            "postcard",
            Some(fs_opts(FileSystemFormat::Postcard, postcard_dir.path())),
            Some(postcard_dir.path()),
        ),
        (
            "grpc",
            Some(ExporterOptions::Collector(CollectorExporterOptions::new(
                quent_addr.clone(),
            ))),
            None,
        ),
    ];

    let mut rows = Vec::new();
    let mut idx = 0usize;
    for &n in &attrs {
        let keys = Arc::new(mk_keys(n));
        for &threads in &thread_list {
            // quent: lossless — emit k, drop (flush); throughput folds in the flush.
            for (bname, opts, output_dir) in &quent_backends {
                // noop discards everything (no sink), so it has no delivered
                // throughput -- only the caller floor, like otel-noop.
                let is_noop = opts.is_none();
                for (signal, sl) in [(Signal::Log, "log"), (Signal::Span, "span")] {
                    let Measurement {
                        offered_recs_s,
                        delivered_recs_s,
                        drain_ms,
                        call_latency_ns,
                    } = average(reps, || {
                        measure_lossless(
                            opts.clone(),
                            signal,
                            &keys,
                            threads,
                            k,
                            latency_every,
                            timer_overhead_ns,
                        )
                    });
                    let (call_p50_ns, call_p95_ns, call_p99_ns) =
                        latency_percentiles(call_latency_ns);
                    record(
                        &mut rows,
                        &mut idx,
                        cells,
                        Row {
                            label: format!("quent-{sl}/{bname}"),
                            n,
                            threads,
                            offered_recs_s,
                            tput_recs_s: if is_noop { None } else { delivered_recs_s },
                            loss_pct: if is_noop { None } else { Some(0.0) },
                            call_p50_ns,
                            call_p95_ns,
                            call_p99_ns,
                            drain_ms: if is_noop { None } else { drain_ms },
                        },
                    );
                    if let Some(output_dir) = output_dir {
                        clear_temp_output(output_dir);
                    }
                }
            }

            // OTel + tracing: lossy — saturate, measure goodput (delivered/s).
            let otel_log = [("noop", None::<&AtomicU64>), ("grpc", Some(&*otlp_logs))];
            for (bname, counter) in otel_log {
                let Measurement {
                    offered_recs_s,
                    delivered_recs_s,
                    drain_ms,
                    call_latency_ns,
                } = average(reps, || {
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
                    measure_goodput(
                        threads,
                        k,
                        counter,
                        &factory,
                        latency_every,
                        timer_overhead_ns,
                    )
                });
                let (call_p50_ns, call_p95_ns, call_p99_ns) = latency_percentiles(call_latency_ns);
                record(
                    &mut rows,
                    &mut idx,
                    cells,
                    Row {
                        label: format!("otel-log/{bname}"),
                        n,
                        threads,
                        offered_recs_s,
                        tput_recs_s: delivered_recs_s,
                        loss_pct: delivered_recs_s
                            .map(|g| (1.0 - g / offered_recs_s).max(0.0) * 100.0),
                        call_p50_ns,
                        call_p95_ns,
                        call_p99_ns,
                        drain_ms,
                    },
                );
            }

            let otel_span = [("noop", None::<&AtomicU64>), ("grpc", Some(&*otlp_spans))];
            for (bname, counter) in otel_span {
                let Measurement {
                    offered_recs_s,
                    delivered_recs_s,
                    drain_ms,
                    call_latency_ns,
                } = average(reps, || {
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
                    measure_goodput(
                        threads,
                        k,
                        counter,
                        &factory,
                        latency_every,
                        timer_overhead_ns,
                    )
                });
                let (call_p50_ns, call_p95_ns, call_p99_ns) = latency_percentiles(call_latency_ns);
                record(
                    &mut rows,
                    &mut idx,
                    cells,
                    Row {
                        label: format!("otel-span/{bname}"),
                        n,
                        threads,
                        offered_recs_s,
                        tput_recs_s: delivered_recs_s,
                        loss_pct: delivered_recs_s
                            .map(|g| (1.0 - g / offered_recs_s).max(0.0) * 100.0),
                        call_p50_ns,
                        call_p95_ns,
                        call_p99_ns,
                        drain_ms,
                    },
                );
            }

            // tracing: JSON to file. NB: serializes on the CALLER thread.
            let Measurement {
                offered_recs_s,
                delivered_recs_s,
                drain_ms,
                call_latency_ns,
            } = average(reps, || {
                let keys = keys.clone();
                let factory = || {
                    let keys = keys.clone();
                    Box::new(move || {
                        tracing::info!(target: "bench_trace", payload = ?tracing_attrs(&keys));
                    }) as Box<dyn FnMut() + Send>
                };
                measure_goodput(
                    threads,
                    k,
                    Some(&trace_delivered),
                    &factory,
                    latency_every,
                    timer_overhead_ns,
                )
            });
            let (call_p50_ns, call_p95_ns, call_p99_ns) = latency_percentiles(call_latency_ns);
            record(
                &mut rows,
                &mut idx,
                cells,
                Row {
                    label: "tracing-log/file".to_string(),
                    n,
                    threads,
                    offered_recs_s,
                    tput_recs_s: delivered_recs_s,
                    loss_pct: delivered_recs_s.map(|g| (1.0 - g / offered_recs_s).max(0.0) * 100.0),
                    call_p50_ns,
                    call_p95_ns,
                    call_p99_ns,
                    drain_ms,
                },
            );

            let Measurement {
                offered_recs_s,
                delivered_recs_s,
                drain_ms,
                call_latency_ns,
            } = average(reps, || {
                let keys = keys.clone();
                let factory = || {
                    let keys = keys.clone();
                    Box::new(move || {
                        let span = tracing::info_span!(target: "bench_trace", "op", payload = ?tracing_attrs(&keys));
                        let _e = span.enter();
                    }) as Box<dyn FnMut() + Send>
                };
                measure_goodput(
                    threads,
                    k,
                    Some(&trace_delivered),
                    &factory,
                    latency_every,
                    timer_overhead_ns,
                )
            });
            let (call_p50_ns, call_p95_ns, call_p99_ns) = latency_percentiles(call_latency_ns);
            record(
                &mut rows,
                &mut idx,
                cells,
                Row {
                    label: "tracing-span/file".to_string(),
                    n,
                    threads,
                    offered_recs_s,
                    tput_recs_s: delivered_recs_s,
                    loss_pct: delivered_recs_s.map(|g| (1.0 - g / offered_recs_s).max(0.0) * 100.0),
                    call_p50_ns,
                    call_p95_ns,
                    call_p99_ns,
                    drain_ms,
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
    println!("columns (all rates are operations/second; 1 op = one log or one span):");
    println!("  offered/s  raw caller API-call rate (before any flush or drop)");
    println!("  tput/s     sustained throughput -- quent: LOSSLESS, ops / time incl. the");
    println!("             full flush on drop;  OTel/tracing: GOODPUT, delivered/s (rest dropped)");
    println!("  loss%      fraction dropped (0% for quent -- lossless; '-' = uncounted floor)");
    println!("  p50/p95/p99 sampled caller-side operation latency under load");
    println!("  drain      time from the end of emission until the pipeline drains");
    println!();
    println!(
        "{:<18} {:>5} {:>4} {:>13} {:>13} {:>7} {:>9} {:>9} {:>9} {:>10}",
        "variant", "attrs", "thr", "offered/s", "tput/s", "loss%", "p50", "p95", "p99", "drain"
    );
    println!("{}", "-".repeat(106));
    for r in &rows {
        let tput = r.tput_recs_s.map_or("-".to_string(), si);
        let loss = r.loss_pct.map_or("-".to_string(), |l| format!("{l:.0}%"));
        let p50 = r.call_p50_ns.map_or("-".to_string(), duration_ns);
        let p95 = r.call_p95_ns.map_or("-".to_string(), duration_ns);
        let p99 = r.call_p99_ns.map_or("-".to_string(), duration_ns);
        let drain = r.drain_ms.map_or("-".to_string(), duration_ms);
        println!(
            "{:<18} {:>5} {:>4} {:>13} {:>13} {:>7} {:>9} {:>9} {:>9} {:>10}",
            r.label,
            r.n,
            r.threads,
            si(r.offered_recs_s),
            tput,
            loss,
            p50,
            p95,
            p99,
            drain,
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
        let mut csv = String::from(
            "variant,attrs,threads,offered_ops_s,tput_ops_s,loss_pct,call_p50_ns,call_p95_ns,call_p99_ns,drain_ms\n",
        );
        for r in &rows {
            let tput = r.tput_recs_s.map_or(String::new(), |v| v.to_string());
            let loss = r.loss_pct.map_or(String::new(), |v| v.to_string());
            let p50 = r.call_p50_ns.map_or(String::new(), |v| v.to_string());
            let p95 = r.call_p95_ns.map_or(String::new(), |v| v.to_string());
            let p99 = r.call_p99_ns.map_or(String::new(), |v| v.to_string());
            let drain = r.drain_ms.map_or(String::new(), |v| v.to_string());
            csv.push_str(&format!(
                "{},{},{},{},{tput},{loss},{p50},{p95},{p99},{drain}\n",
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
        let integer = |o: Option<u64>| o.map_or("null".to_string(), |v| v.to_string());
        let items: Vec<String> = rows
            .iter()
            .map(|r| {
                let family = r.label.split(['-', '/']).next().unwrap_or(&r.label);
                format!(
                    "{{\"variant\":\"{}\",\"family\":\"{}\",\"attrs\":{},\"threads\":{},\
                     \"offered_ops_s\":{},\"throughput_ops_s\":{},\"loss_pct\":{},\
                     \"call_p50_ns\":{},\"call_p95_ns\":{},\"call_p99_ns\":{},\"drain_ms\":{}}}",
                    r.label,
                    family,
                    r.n,
                    r.threads,
                    r.offered_recs_s,
                    num(r.tput_recs_s),
                    num(r.loss_pct),
                    integer(r.call_p50_ns),
                    integer(r.call_p95_ns),
                    integer(r.call_p99_ns),
                    num(r.drain_ms),
                )
            })
            .collect();
        let meta = format!(
            "{{\"unit\":\"operations/second (1 op = one log or one span)\",\"ops_per_cell\":{k},\
             \"metrics\":{{\
             \"offered_ops_s\":\"raw API-call rate = ops / emit time\",\
             \"throughput_ops_s\":\"sustained rate. quent: lossless, ops / full time to flush+deliver everything. otel/tracing: goodput = delivered/s, the rest dropped. null = no counted sink (noop)\",\
             \"loss_pct\":\"percent of offered ops that never reached the sink; 0 for quent (lossless); null = uncounted (noop)\",\
             \"call_p50_ns\":\"sampled caller-side operation latency, 50th percentile\",\
             \"call_p95_ns\":\"sampled caller-side operation latency, 95th percentile\",\
             \"call_p99_ns\":\"sampled caller-side operation latency, 99th percentile\",\
             \"drain_ms\":\"time from the end of caller emission until the counted pipeline drains; null = no counted sink\"}},\
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

    drop(trace_guard);
}
