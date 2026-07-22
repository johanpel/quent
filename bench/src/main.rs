// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#![recursion_limit = "512"]
#![allow(
    clippy::eq_op,
    clippy::large_enum_variant,
    clippy::manual_is_multiple_of,
    clippy::too_many_arguments
)]

//! Throughput comparison: quent vs OpenTelemetry 0.32 vs `tracing`, each as it
//! is natively built.
//!
//! Per (signal, backend, attribute-count, threads) cell, spam `BENCH_OPS`
//! operations flat out across the producer threads, then report:
//! - `offered`  the raw API-call rate (ops / emit time), one op = one log or one
//!   span. Span file pipelines export both lifecycle boundaries: quent emits two
//!   transitions and tracing emits NEW plus CLOSE.
//! - `tput`     sustained throughput. quent is lossless -- every op is delivered,
//!   measured by emitting then dropping the context (a blocking flush) and
//!   dividing ops by total time INCLUDING the flush. `tracing` uses backpressure
//!   for an equivalent lossless file-pipeline comparison. OTel is lossy (bounded
//!   queue), so goodput is counted at the sink and the rest dropped.
//! - `call_pXX` sampled caller-side operation latency under the same load.
//! - `drain`     time from the last caller operation until the pipeline drains.
//!
//! Backends (native to each library): quent {noop, bitcode, ndjson, msgpack, postcard,
//! grpc collector}; OTel {noop, OTLP/gRPC}; tracing {JSON file}. gRPC/OTLP go to
//! a live in-process receiver; the tracing writer counts newlines.
//!
//! Quent runs both generated static schemas and a separately labeled dynamic
//! attribute variant. Attribute payloads deterministically mix strings, i64,
//! f64, bool, and primitive arrays of those types.
//!
//! Knobs: `BENCH_OPS` (ops/cell, default 2,000,000), `BENCH_ATTRS`,
//! `BENCH_THREADS` (comma lists), `BENCH_REPS`, `BENCH_CSV` (raw CSV path),
//! `BENCH_JSON` (self-describing JSON path), `BENCH_LATENCY_EVERY` (caller
//! latency sampling interval, default 1,024 operations), and
//! `QUENT_EVENT_CHANNEL_CAPACITY` (lossless Quent queue capacity, default
//! 4,096 events).

use std::fs::File;
use std::io::Seek;
use std::net::TcpListener as StdTcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use quent_collector::{CollectorSink, server::CollectorService};
use quent_collector_proto::collector_server::CollectorServer;
use quent_dynamic_attributes::{DynamicAttribute, DynamicAttributes, DynamicList};
use quent_instrumentation::{EntityEvent, Event};
use quent_io::{
    CollectorExporterOptions, ExporterOptions, FileSystemExporterOptions, FileSystemFormat,
};

use opentelemetry::logs::{AnyValue, LogRecord, Logger, LoggerProvider, Severity};
use opentelemetry::trace::{Span as OtelSpan, Tracer, TracerProvider};
use opentelemetry::{Array, KeyValue, StringValue, Value};
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
use tracing_appender::non_blocking::NonBlockingBuilder;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{filter, fmt};
#[allow(dead_code, unused_imports)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/bench.rs"));

    pub struct ProducerRawBenchContext {
        bench_log: BenchLogObserver,
        bench_span: BenchSpanObserver,
        _inner: ::quent_instrumentation::Context,
    }

    impl ProducerRawBenchContext {
        pub fn try_new(root: &::std::path::Path) -> Result<Self, Box<dyn ::std::error::Error>> {
            let inner =
                ::quent_instrumentation::Context::try_new(::quent_instrumentation::Uuid::now_v7())?;
            let context_root = root.join(inner.id().to_string());
            let log_sink = ::std::sync::Arc::new(
                ::quent_io_raw::SerdeRawSink::<BenchLogEvent>::try_new(&context_root)?,
            );
            let span_sink = ::std::sync::Arc::new(
                ::quent_io_raw::SerdeRawSink::<BenchSpanEvent>::try_new(&context_root)?,
            );
            let bench_log = ::quent_instrumentation::Observer::from_sink(
                {
                    let sink = log_sink.clone();
                    ::std::sync::Arc::new(move |event| sink.send(event))
                },
                {
                    let sink = log_sink.clone();
                    ::std::sync::Arc::new(move || sink.flush_current())
                },
                move || log_sink.shutdown(),
            );
            let bench_span = ::quent_instrumentation::Observer::from_sink(
                {
                    let sink = span_sink.clone();
                    ::std::sync::Arc::new(move |event| sink.send(event))
                },
                {
                    let sink = span_sink.clone();
                    ::std::sync::Arc::new(move || sink.flush_current())
                },
                move || span_sink.shutdown(),
            );
            Ok(Self {
                bench_log: BenchLogObserver {
                    inner: ::std::sync::Arc::new(bench_log),
                },
                bench_span: BenchSpanObserver {
                    inner: ::std::sync::Arc::new(bench_span),
                },
                _inner: inner,
            })
        }

        pub fn bench_log_observer(&self) -> BenchLogObserver {
            self.bench_log.clone()
        }

        pub fn bench_span_observer(&self) -> BenchSpanObserver {
            self.bench_span.clone()
        }
    }
}
#[allow(dead_code, unused_imports)]
mod native_generated {
    include!(concat!(env!("OUT_DIR"), "/bench_native.rs"));

    pub struct NativeBenchContext {
        bench_log: BenchLogObserver,
        bench_span: BenchSpanObserver,
        _inner: ::quent_instrumentation::Context,
    }

    impl NativeBenchContext {
        fn try_new_with<P>(options: P) -> Result<Self, Box<dyn ::std::error::Error>>
        where
            P: ::quent_io::ExporterProvider<BenchLogEvent>
                + ::quent_io::ExporterProvider<BenchSpanEvent>
                + Clone,
        {
            let inner =
                ::quent_instrumentation::Context::try_new(::quent_instrumentation::Uuid::now_v7())?;
            let (bench_log, bench_span) = inner.block_on(async {
                Ok::<_, Box<dyn ::std::error::Error>>((
                    inner.observer::<BenchLogEvent>(options.clone()).await?,
                    inner.observer::<BenchSpanEvent>(options).await?,
                ))
            })?;
            Ok(Self {
                bench_log: BenchLogObserver {
                    inner: ::std::sync::Arc::new(bench_log),
                },
                bench_span: BenchSpanObserver {
                    inner: ::std::sync::Arc::new(bench_span),
                },
                _inner: inner,
            })
        }

        pub fn try_new_bitcode(
            root: &::std::path::Path,
        ) -> Result<Self, Box<dyn ::std::error::Error>> {
            Self::try_new_with(::quent_io_bitcode::NativeBitcodeExporterOptions {
                root: root.to_owned(),
            })
        }

        pub fn try_new_raw(root: &::std::path::Path) -> Result<Self, Box<dyn ::std::error::Error>> {
            let inner =
                ::quent_instrumentation::Context::try_new(::quent_instrumentation::Uuid::now_v7())?;
            let context_root = root.join(inner.id().to_string());
            let log_sink = ::std::sync::Arc::new(
                ::quent_io_raw::NativeRawSink::<BenchLogEvent>::try_new(&context_root)?,
            );
            let span_sink = ::std::sync::Arc::new(
                ::quent_io_raw::NativeRawSink::<BenchSpanEvent>::try_new(&context_root)?,
            );
            let bench_log = ::quent_instrumentation::Observer::from_sink(
                {
                    let sink = log_sink.clone();
                    ::std::sync::Arc::new(move |event| sink.send(event))
                },
                {
                    let sink = log_sink.clone();
                    ::std::sync::Arc::new(move || sink.flush_current())
                },
                move || log_sink.shutdown(),
            );
            let bench_span = ::quent_instrumentation::Observer::from_sink(
                {
                    let sink = span_sink.clone();
                    ::std::sync::Arc::new(move |event| sink.send(event))
                },
                {
                    let sink = span_sink.clone();
                    ::std::sync::Arc::new(move || sink.flush_current())
                },
                move || span_sink.shutdown(),
            );
            Ok(Self {
                bench_log: BenchLogObserver {
                    inner: ::std::sync::Arc::new(bench_log),
                },
                bench_span: BenchSpanObserver {
                    inner: ::std::sync::Arc::new(bench_span),
                },
                _inner: inner,
            })
        }

        pub fn bench_log_observer(&self) -> BenchLogObserver {
            self.bench_log.clone()
        }

        pub fn bench_span_observer(&self) -> BenchSpanObserver {
            self.bench_span.clone()
        }
    }
}

use generated::{
    BenchContext, BenchLogEvent, BenchLogHandle, BenchSpanEvent, BenchSpanHandle,
    ProducerRawBenchContext,
};
use native_generated::{
    BenchLogHandle as NativeBenchLogHandle, BenchSpanHandle as NativeBenchSpanHandle,
    NativeBenchContext,
};

// -------- deterministic, mixed-type attribute payloads --------

const SUPPORTED_ATTR_COUNTS: &[usize] = &[0, 1, 2, 4, 8, 16, 32, 64];

#[derive(Clone, Copy)]
enum AttrKind {
    String,
    I64,
    F64,
    Bool,
    StringList,
    I64List,
    F64List,
    BoolList,
}

const ATTR_KINDS: [AttrKind; 64] = [
    AttrKind::String,
    AttrKind::I64,
    AttrKind::F64,
    AttrKind::Bool,
    AttrKind::StringList,
    AttrKind::I64List,
    AttrKind::F64List,
    AttrKind::BoolList,
    AttrKind::I64,
    AttrKind::StringList,
    AttrKind::BoolList,
    AttrKind::Bool,
    AttrKind::String,
    AttrKind::I64,
    AttrKind::F64List,
    AttrKind::StringList,
    AttrKind::F64,
    AttrKind::I64,
    AttrKind::StringList,
    AttrKind::F64,
    AttrKind::StringList,
    AttrKind::StringList,
    AttrKind::I64,
    AttrKind::I64,
    AttrKind::I64,
    AttrKind::I64List,
    AttrKind::String,
    AttrKind::I64List,
    AttrKind::BoolList,
    AttrKind::F64,
    AttrKind::BoolList,
    AttrKind::F64List,
    AttrKind::StringList,
    AttrKind::F64,
    AttrKind::I64,
    AttrKind::I64List,
    AttrKind::I64,
    AttrKind::StringList,
    AttrKind::F64List,
    AttrKind::I64List,
    AttrKind::Bool,
    AttrKind::Bool,
    AttrKind::F64,
    AttrKind::Bool,
    AttrKind::String,
    AttrKind::F64List,
    AttrKind::F64,
    AttrKind::BoolList,
    AttrKind::StringList,
    AttrKind::I64List,
    AttrKind::I64List,
    AttrKind::F64List,
    AttrKind::I64List,
    AttrKind::Bool,
    AttrKind::I64,
    AttrKind::StringList,
    AttrKind::Bool,
    AttrKind::I64,
    AttrKind::I64,
    AttrKind::String,
    AttrKind::I64,
    AttrKind::Bool,
    AttrKind::String,
    AttrKind::F64,
];

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

fn string_value(field: usize) -> String {
    const LENGTHS: [usize; 7] = [1, 7, 31, 127, 511, 2_047, 8_191];
    "s".repeat(LENGTHS[(field * 5 + 3) % LENGTHS.len()])
}

fn array_len(field: usize) -> usize {
    const LENGTHS: [usize; 6] = [0, 1, 3, 8, 16, 32];
    LENGTHS[(field * 7 + 1) % LENGTHS.len()]
}

fn string_array(field: usize) -> Vec<String> {
    const LENGTHS: [usize; 4] = [1, 7, 31, 127];
    (0..array_len(field))
        .map(|i| "a".repeat(LENGTHS[(field + i) % LENGTHS.len()]))
        .collect()
}

fn i64_array(field: usize) -> Vec<i64> {
    (0..array_len(field)).map(|i| i as i64).collect()
}

fn f64_array(field: usize) -> Vec<f64> {
    (0..array_len(field)).map(|i| i as f64 + 0.5).collect()
}

fn bool_array(field: usize) -> Vec<bool> {
    (0..array_len(field)).map(|i| i % 2 == 0).collect()
}

fn quent_attrs(keys: &[&'static str]) -> DynamicAttributes {
    let mut attrs = DynamicAttributes::with_capacity(keys.len());
    for (i, key) in keys.iter().enumerate() {
        let attr = match ATTR_KINDS[i] {
            AttrKind::String => DynamicAttribute::string(*key, string_value(i)),
            AttrKind::I64 => DynamicAttribute::i64(*key, i as i64),
            AttrKind::F64 => DynamicAttribute::f64(*key, i as f64 + 0.5),
            AttrKind::Bool => DynamicAttribute::u8(*key, u8::from(i % 2 == 0)),
            AttrKind::StringList => {
                DynamicAttribute::list(*key, DynamicList::String(string_array(i)))
            }
            AttrKind::I64List => DynamicAttribute::list(*key, DynamicList::I64(i64_array(i))),
            AttrKind::F64List => DynamicAttribute::list(*key, DynamicList::F64(f64_array(i))),
            AttrKind::BoolList => DynamicAttribute::list(*key, DynamicList::Bool(bool_array(i))),
        };
        attrs.add(attr);
    }
    attrs
}

#[derive(Clone, Copy)]
enum QuentSchemaMode {
    Static,
    Dynamic,
}

macro_rules! define_static_log_emitter {
    ($name:ident, $handle:ty) => {
        #[inline(always)]
        fn $name(handle: &$handle, n: usize) {
            match n {
                0 => handle.static_0().unwrap(),
                1 => handle.static_1(string_value(0)).unwrap(),
                2 => handle.static_2(string_value(0), 1i64).unwrap(),
                4 => handle
                    .static_4(string_value(0), 1i64, 2.0f64 + 0.5, 3 % 2 == 0)
                    .unwrap(),
                8 => handle
                    .static_8(
                        string_value(0),
                        1i64,
                        2.0f64 + 0.5,
                        3 % 2 == 0,
                        string_array(4),
                        i64_array(5),
                        f64_array(6),
                        bool_array(7),
                    )
                    .unwrap(),
                16 => handle
                    .static_16(
                        string_value(0),
                        1i64,
                        2.0f64 + 0.5,
                        3 % 2 == 0,
                        string_array(4),
                        i64_array(5),
                        f64_array(6),
                        bool_array(7),
                        8i64,
                        string_array(9),
                        bool_array(10),
                        11 % 2 == 0,
                        string_value(12),
                        13i64,
                        f64_array(14),
                        string_array(15),
                    )
                    .unwrap(),
                32 => handle
                    .static_32(
                        string_value(0),
                        1i64,
                        2.0f64 + 0.5,
                        3 % 2 == 0,
                        string_array(4),
                        i64_array(5),
                        f64_array(6),
                        bool_array(7),
                        8i64,
                        string_array(9),
                        bool_array(10),
                        11 % 2 == 0,
                        string_value(12),
                        13i64,
                        f64_array(14),
                        string_array(15),
                        16.0f64 + 0.5,
                        17i64,
                        string_array(18),
                        19.0f64 + 0.5,
                        string_array(20),
                        string_array(21),
                        22i64,
                        23i64,
                        24i64,
                        i64_array(25),
                        string_value(26),
                        i64_array(27),
                        bool_array(28),
                        29.0f64 + 0.5,
                        bool_array(30),
                        f64_array(31),
                    )
                    .unwrap(),
                64 => handle
                    .static_64(
                        string_value(0),
                        1i64,
                        2.0f64 + 0.5,
                        3 % 2 == 0,
                        string_array(4),
                        i64_array(5),
                        f64_array(6),
                        bool_array(7),
                        8i64,
                        string_array(9),
                        bool_array(10),
                        11 % 2 == 0,
                        string_value(12),
                        13i64,
                        f64_array(14),
                        string_array(15),
                        16.0f64 + 0.5,
                        17i64,
                        string_array(18),
                        19.0f64 + 0.5,
                        string_array(20),
                        string_array(21),
                        22i64,
                        23i64,
                        24i64,
                        i64_array(25),
                        string_value(26),
                        i64_array(27),
                        bool_array(28),
                        29.0f64 + 0.5,
                        bool_array(30),
                        f64_array(31),
                        string_array(32),
                        33.0f64 + 0.5,
                        34i64,
                        i64_array(35),
                        36i64,
                        string_array(37),
                        f64_array(38),
                        i64_array(39),
                        40 % 2 == 0,
                        41 % 2 == 0,
                        42.0f64 + 0.5,
                        43 % 2 == 0,
                        string_value(44),
                        f64_array(45),
                        46.0f64 + 0.5,
                        bool_array(47),
                        string_array(48),
                        i64_array(49),
                        i64_array(50),
                        f64_array(51),
                        i64_array(52),
                        53 % 2 == 0,
                        54i64,
                        string_array(55),
                        56 % 2 == 0,
                        57i64,
                        58i64,
                        string_value(59),
                        60i64,
                        61 % 2 == 0,
                        string_value(62),
                        63.0f64 + 0.5,
                    )
                    .unwrap(),
                _ => unreachable!("validated attribute count"),
            }
        }
    };
}

define_static_log_emitter!(emit_quent_static_log, BenchLogHandle);
define_static_log_emitter!(emit_quent_native_static_log, NativeBenchLogHandle);

macro_rules! define_static_span_emitter {
    ($name:ident, $handle:ty) => {
        #[inline(always)]
        fn $name(handle: &$handle, n: usize) {
            match n {
                0 => handle.static_0().unwrap(),
                1 => handle.static_1(string_value(0)).unwrap(),
                2 => handle.static_2(string_value(0), 1i64).unwrap(),
                4 => handle
                    .static_4(string_value(0), 1i64, 2.0f64 + 0.5, 3 % 2 == 0)
                    .unwrap(),
                8 => handle
                    .static_8(
                        string_value(0),
                        1i64,
                        2.0f64 + 0.5,
                        3 % 2 == 0,
                        string_array(4),
                        i64_array(5),
                        f64_array(6),
                        bool_array(7),
                    )
                    .unwrap(),
                16 => handle
                    .static_16(
                        string_value(0),
                        1i64,
                        2.0f64 + 0.5,
                        3 % 2 == 0,
                        string_array(4),
                        i64_array(5),
                        f64_array(6),
                        bool_array(7),
                        8i64,
                        string_array(9),
                        bool_array(10),
                        11 % 2 == 0,
                        string_value(12),
                        13i64,
                        f64_array(14),
                        string_array(15),
                    )
                    .unwrap(),
                32 => handle
                    .static_32(
                        string_value(0),
                        1i64,
                        2.0f64 + 0.5,
                        3 % 2 == 0,
                        string_array(4),
                        i64_array(5),
                        f64_array(6),
                        bool_array(7),
                        8i64,
                        string_array(9),
                        bool_array(10),
                        11 % 2 == 0,
                        string_value(12),
                        13i64,
                        f64_array(14),
                        string_array(15),
                        16.0f64 + 0.5,
                        17i64,
                        string_array(18),
                        19.0f64 + 0.5,
                        string_array(20),
                        string_array(21),
                        22i64,
                        23i64,
                        24i64,
                        i64_array(25),
                        string_value(26),
                        i64_array(27),
                        bool_array(28),
                        29.0f64 + 0.5,
                        bool_array(30),
                        f64_array(31),
                    )
                    .unwrap(),
                64 => handle
                    .static_64(
                        string_value(0),
                        1i64,
                        2.0f64 + 0.5,
                        3 % 2 == 0,
                        string_array(4),
                        i64_array(5),
                        f64_array(6),
                        bool_array(7),
                        8i64,
                        string_array(9),
                        bool_array(10),
                        11 % 2 == 0,
                        string_value(12),
                        13i64,
                        f64_array(14),
                        string_array(15),
                        16.0f64 + 0.5,
                        17i64,
                        string_array(18),
                        19.0f64 + 0.5,
                        string_array(20),
                        string_array(21),
                        22i64,
                        23i64,
                        24i64,
                        i64_array(25),
                        string_value(26),
                        i64_array(27),
                        bool_array(28),
                        29.0f64 + 0.5,
                        bool_array(30),
                        f64_array(31),
                        string_array(32),
                        33.0f64 + 0.5,
                        34i64,
                        i64_array(35),
                        36i64,
                        string_array(37),
                        f64_array(38),
                        i64_array(39),
                        40 % 2 == 0,
                        41 % 2 == 0,
                        42.0f64 + 0.5,
                        43 % 2 == 0,
                        string_value(44),
                        f64_array(45),
                        46.0f64 + 0.5,
                        bool_array(47),
                        string_array(48),
                        i64_array(49),
                        i64_array(50),
                        f64_array(51),
                        i64_array(52),
                        53 % 2 == 0,
                        54i64,
                        string_array(55),
                        56 % 2 == 0,
                        57i64,
                        58i64,
                        string_value(59),
                        60i64,
                        61 % 2 == 0,
                        string_value(62),
                        63.0f64 + 0.5,
                    )
                    .unwrap(),
                _ => unreachable!("validated attribute count"),
            }
            handle.idle().unwrap();
        }
    };
}

define_static_span_emitter!(emit_quent_static_span, BenchSpanHandle);
define_static_span_emitter!(emit_quent_native_static_span, NativeBenchSpanHandle);

#[inline(always)]
fn emit_quent_dynamic_log(handle: &BenchLogHandle, keys: &[&'static str]) {
    handle.dynamic(quent_attrs(keys)).unwrap();
}

#[inline(always)]
fn emit_quent_dynamic_span(handle: &BenchSpanHandle, keys: &[&'static str]) {
    handle.dynamic(quent_attrs(keys)).unwrap();
    handle.idle().unwrap();
}

#[inline(always)]
fn tracing_log_static(n: usize) {
    match n {
        0 => tracing::info!(target: "bench_trace", operation = "log"),
        1 => tracing::info!(target: "bench_trace", operation = "log", k0 = %string_value(0)),
        2 => {
            tracing::info!(target: "bench_trace", operation = "log", k0 = %string_value(0), k1 = 1i64)
        }
        4 => {
            tracing::info!(target: "bench_trace", operation = "log", k0 = %string_value(0), k1 = 1i64, k2 = 2.0f64 + 0.5, k3 = 3 % 2 == 0)
        }
        8 => {
            tracing::info!(target: "bench_trace", operation = "log", k0 = %string_value(0), k1 = 1i64, k2 = 2.0f64 + 0.5, k3 = 3 % 2 == 0, k4 = ?string_array(4), k5 = ?i64_array(5), k6 = ?f64_array(6), k7 = ?bool_array(7))
        }
        16 => {
            tracing::info!(target: "bench_trace", operation = "log", k0 = %string_value(0), k1 = 1i64, k2 = 2.0f64 + 0.5, k3 = 3 % 2 == 0, k4 = ?string_array(4), k5 = ?i64_array(5), k6 = ?f64_array(6), k7 = ?bool_array(7), k8 = 8i64, k9 = ?string_array(9), k10 = ?bool_array(10), k11 = 11 % 2 == 0, k12 = %string_value(12), k13 = 13i64, k14 = ?f64_array(14), k15 = ?string_array(15))
        }
        32 => {
            tracing::info!(target: "bench_trace", operation = "log", k0 = %string_value(0), k1 = 1i64, k2 = 2.0f64 + 0.5, k3 = 3 % 2 == 0, k4 = ?string_array(4), k5 = ?i64_array(5), k6 = ?f64_array(6), k7 = ?bool_array(7), k8 = 8i64, k9 = ?string_array(9), k10 = ?bool_array(10), k11 = 11 % 2 == 0, k12 = %string_value(12), k13 = 13i64, k14 = ?f64_array(14), k15 = ?string_array(15), k16 = 16.0f64 + 0.5, k17 = 17i64, k18 = ?string_array(18), k19 = 19.0f64 + 0.5, k20 = ?string_array(20), k21 = ?string_array(21), k22 = 22i64, k23 = 23i64, k24 = 24i64, k25 = ?i64_array(25), k26 = %string_value(26), k27 = ?i64_array(27), k28 = ?bool_array(28), k29 = 29.0f64 + 0.5, k30 = ?bool_array(30), k31 = ?f64_array(31))
        }
        64 => {
            tracing::info!(target: "bench_trace", operation = "log", k0 = %string_value(0), k1 = 1i64, k2 = 2.0f64 + 0.5, k3 = 3 % 2 == 0, k4 = ?string_array(4), k5 = ?i64_array(5), k6 = ?f64_array(6), k7 = ?bool_array(7), k8 = 8i64, k9 = ?string_array(9), k10 = ?bool_array(10), k11 = 11 % 2 == 0, k12 = %string_value(12), k13 = 13i64, k14 = ?f64_array(14), k15 = ?string_array(15), k16 = 16.0f64 + 0.5, k17 = 17i64, k18 = ?string_array(18), k19 = 19.0f64 + 0.5, k20 = ?string_array(20), k21 = ?string_array(21), k22 = 22i64, k23 = 23i64, k24 = 24i64, k25 = ?i64_array(25), k26 = %string_value(26), k27 = ?i64_array(27), k28 = ?bool_array(28), k29 = 29.0f64 + 0.5, k30 = ?bool_array(30), k31 = ?f64_array(31), k32 = ?string_array(32), k33 = 33.0f64 + 0.5, k34 = 34i64, k35 = ?i64_array(35), k36 = 36i64, k37 = ?string_array(37), k38 = ?f64_array(38), k39 = ?i64_array(39), k40 = 40 % 2 == 0, k41 = 41 % 2 == 0, k42 = 42.0f64 + 0.5, k43 = 43 % 2 == 0, k44 = %string_value(44), k45 = ?f64_array(45), k46 = 46.0f64 + 0.5, k47 = ?bool_array(47), k48 = ?string_array(48), k49 = ?i64_array(49), k50 = ?i64_array(50), k51 = ?f64_array(51), k52 = ?i64_array(52), k53 = 53 % 2 == 0, k54 = 54i64, k55 = ?string_array(55), k56 = 56 % 2 == 0, k57 = 57i64, k58 = 58i64, k59 = %string_value(59), k60 = 60i64, k61 = 61 % 2 == 0, k62 = %string_value(62), k63 = 63.0f64 + 0.5)
        }
        _ => unreachable!("validated attribute count"),
    }
}

#[inline(always)]
fn tracing_span_static(n: usize) {
    match n {
        0 => {
            let span = tracing::info_span!(target: "bench_trace", "op");
            let _entered = span.enter();
        }
        1 => {
            let span = tracing::info_span!(target: "bench_trace", "op", k0 = %string_value(0));
            let _entered = span.enter();
        }
        2 => {
            let span =
                tracing::info_span!(target: "bench_trace", "op", k0 = %string_value(0), k1 = 1i64);
            let _entered = span.enter();
        }
        4 => {
            let span = tracing::info_span!(target: "bench_trace", "op", k0 = %string_value(0), k1 = 1i64, k2 = 2.0f64 + 0.5, k3 = 3 % 2 == 0);
            let _entered = span.enter();
        }
        8 => {
            let span = tracing::info_span!(target: "bench_trace", "op", k0 = %string_value(0), k1 = 1i64, k2 = 2.0f64 + 0.5, k3 = 3 % 2 == 0, k4 = ?string_array(4), k5 = ?i64_array(5), k6 = ?f64_array(6), k7 = ?bool_array(7));
            let _entered = span.enter();
        }
        16 => {
            let span = tracing::info_span!(target: "bench_trace", "op", k0 = %string_value(0), k1 = 1i64, k2 = 2.0f64 + 0.5, k3 = 3 % 2 == 0, k4 = ?string_array(4), k5 = ?i64_array(5), k6 = ?f64_array(6), k7 = ?bool_array(7), k8 = 8i64, k9 = ?string_array(9), k10 = ?bool_array(10), k11 = 11 % 2 == 0, k12 = %string_value(12), k13 = 13i64, k14 = ?f64_array(14), k15 = ?string_array(15));
            let _entered = span.enter();
        }
        32 => {
            let span = tracing::info_span!(target: "bench_trace", "op", k0 = %string_value(0), k1 = 1i64, k2 = 2.0f64 + 0.5, k3 = 3 % 2 == 0, k4 = ?string_array(4), k5 = ?i64_array(5), k6 = ?f64_array(6), k7 = ?bool_array(7), k8 = 8i64, k9 = ?string_array(9), k10 = ?bool_array(10), k11 = 11 % 2 == 0, k12 = %string_value(12), k13 = 13i64, k14 = ?f64_array(14), k15 = ?string_array(15), k16 = 16.0f64 + 0.5, k17 = 17i64, k18 = ?string_array(18), k19 = 19.0f64 + 0.5, k20 = ?string_array(20), k21 = ?string_array(21), k22 = 22i64, k23 = 23i64, k24 = 24i64, k25 = ?i64_array(25), k26 = %string_value(26), k27 = ?i64_array(27), k28 = ?bool_array(28), k29 = 29.0f64 + 0.5, k30 = ?bool_array(30), k31 = ?f64_array(31));
            let _entered = span.enter();
        }
        64 => {
            let span = tracing::info_span!(target: "bench_trace", "op", k0 = %string_value(0), k1 = 1i64, k2 = 2.0f64 + 0.5, k3 = 3 % 2 == 0, k4 = ?string_array(4), k5 = ?i64_array(5), k6 = ?f64_array(6), k7 = ?bool_array(7), k8 = 8i64, k9 = ?string_array(9), k10 = ?bool_array(10), k11 = 11 % 2 == 0, k12 = %string_value(12), k13 = 13i64, k14 = ?f64_array(14), k15 = ?string_array(15), k16 = 16.0f64 + 0.5, k17 = 17i64, k18 = ?string_array(18), k19 = 19.0f64 + 0.5, k20 = ?string_array(20), k21 = ?string_array(21), k22 = 22i64, k23 = 23i64, k24 = 24i64, k25 = ?i64_array(25), k26 = %string_value(26), k27 = ?i64_array(27), k28 = ?bool_array(28), k29 = 29.0f64 + 0.5, k30 = ?bool_array(30), k31 = ?f64_array(31), k32 = ?string_array(32), k33 = 33.0f64 + 0.5, k34 = 34i64, k35 = ?i64_array(35), k36 = 36i64, k37 = ?string_array(37), k38 = ?f64_array(38), k39 = ?i64_array(39), k40 = 40 % 2 == 0, k41 = 41 % 2 == 0, k42 = 42.0f64 + 0.5, k43 = 43 % 2 == 0, k44 = %string_value(44), k45 = ?f64_array(45), k46 = 46.0f64 + 0.5, k47 = ?bool_array(47), k48 = ?string_array(48), k49 = ?i64_array(49), k50 = ?i64_array(50), k51 = ?f64_array(51), k52 = ?i64_array(52), k53 = 53 % 2 == 0, k54 = 54i64, k55 = ?string_array(55), k56 = 56 % 2 == 0, k57 = 57i64, k58 = 58i64, k59 = %string_value(59), k60 = 60i64, k61 = 61 % 2 == 0, k62 = %string_value(62), k63 = 63.0f64 + 0.5);
            let _entered = span.enter();
        }
        _ => unreachable!("validated attribute count"),
    }
}

/// A `Write` wrapper counting delivered records (one per newline) so `tracing`'s
/// off-thread appender has a countable sink, like the other backends.
struct CountingWriter {
    inner: Arc<Mutex<File>>,
    counter: Arc<AtomicU64>,
}
impl std::io::Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        let written = std::io::Write::write(&mut *inner, buf)?;
        let lines = buf[..written].iter().filter(|&&b| b == b'\n').count() as u64;
        self.counter.fetch_add(lines, Ordering::Relaxed);
        Ok(written)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut *self.inner.lock().unwrap())
    }
}

fn clear_trace_output(file: &Mutex<File>) {
    let mut file = file.lock().unwrap();
    file.set_len(0).unwrap();
    file.seek(std::io::SeekFrom::Start(0)).unwrap();
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

fn byte_rate(v: f64) -> String {
    if v >= 999_950_000.0 {
        format!("{:.2}GB/s", v / 1e9)
    } else if v >= 999_950.0 {
        format!("{:.1}MB/s", v / 1e6)
    } else if v >= 999.95 {
        format!("{:.1}kB/s", v / 1e3)
    } else {
        format!("{v:.0}B/s")
    }
}

// -------- quent collector receiver (decode + drop) --------

struct QuentSink {
    received: Arc<AtomicU64>,
}
impl CollectorSink for QuentSink {
    fn ingest(&self, entity: &str, event: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        if entity == <BenchLogEvent as EntityEvent>::NAME {
            let _: Event<BenchLogEvent> = quent_collector_client::deserialize_event(event)?;
        } else if entity == <BenchSpanEvent as EntityEvent>::NAME {
            let _: Event<BenchSpanEvent> = quent_collector_client::deserialize_event(event)?;
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
    /// Sustained throughput: quent/tracing = lossless (records / time incl. flush);
    /// OTel = goodput (delivered/s). `None` for uncounted floors.
    tput_recs_s: Option<f64>,
    loss_pct: Option<f64>,
    call_p50_ns: Option<u64>,
    call_p95_ns: Option<u64>,
    call_p99_ns: Option<u64>,
    drain_ms: Option<f64>,
    bytes_written: Option<u64>,
    write_bytes_s: Option<f64>,
}

struct Measurement {
    offered_recs_s: f64,
    delivered_recs_s: Option<f64>,
    loss_pct: Option<f64>,
    drain_ms: Option<f64>,
    bytes_written: Option<u64>,
    write_bytes_s: Option<f64>,
    call_latency_ns: Vec<u64>,
}

fn directory_bytes(root: &Path) -> u64 {
    fn visit(path: &Path) -> u64 {
        let Ok(metadata) = std::fs::metadata(path) else {
            return 0;
        };
        if metadata.is_file() {
            return metadata.len();
        }
        std::fs::read_dir(path)
            .map(|entries| entries.flatten().map(|entry| visit(&entry.path())).sum())
            .unwrap_or(0)
    }
    visit(root)
}

fn attach_file_bytes(measurement: &mut Measurement, bytes: u64, operations: u64) {
    measurement.bytes_written = Some(bytes);
    measurement.write_bytes_s = measurement
        .delivered_recs_s
        .map(|ops_s| bytes as f64 * ops_s / operations as f64);
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
    sink_records_per_op: u64,
    counter: Option<&AtomicU64>,
    factory: &dyn Fn() -> Box<dyn FnMut() + Send>,
    sample_every: u64,
    timer_overhead_ns: u64,
) -> Measurement {
    let per = (k / threads as u64).max(1);
    let total_ops = per * threads as u64;
    let expected_records = total_ops * sink_records_per_op;
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

    let delivery = counter.map(|c| {
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
            if cur - r0 >= expected_records || Instant::now() >= deadline {
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
        let delivered = c.load(Ordering::Relaxed) - r0;
        (
            delivered as f64 / sink_records_per_op as f64 / total_t,
            (1.0 - delivered as f64 / expected_records as f64).max(0.0) * 100.0,
            drain_ms,
        )
    });

    Measurement {
        offered_recs_s: total_ops as f64 / emit_t,
        delivered_recs_s: delivery.map(|(rate, _, _)| rate),
        loss_pct: delivery.map(|(_, loss_pct, _)| loss_pct),
        drain_ms: delivery.map(|(_, _, drain_ms)| drain_ms),
        bytes_written: None,
        write_bytes_s: None,
        call_latency_ns: take_samples(&samples),
    }
}

/// Measure lossless throughput with one entity per worker.
fn measure_lossless(
    runtime: &Runtime,
    backend: Option<ExporterOptions>,
    schema_mode: QuentSchemaMode,
    signal: Signal,
    keys: &Arc<Vec<&'static str>>,
    threads: usize,
    k: u64,
    sample_every: u64,
    timer_overhead_ns: u64,
) -> Measurement {
    // Exercise Quent's ambient-runtime path. The benchmark already owns this
    // runtime for its loopback receivers; constructing a synchronous context
    // outside it would spawn a second, machine-sized Tokio pool for one
    // forwarder and make the result depend primarily on host CPU count.
    let _runtime_guard = runtime.enter();
    let ctx = BenchContext::try_new(backend).unwrap();
    let per = (k / threads as u64).max(1);
    let total = per * threads as u64;
    let samples = Arc::new(Mutex::new(Vec::new()));
    let t0 = Instant::now();
    let emit_t = match signal {
        Signal::Log => {
            let obs = ctx.bench_log_observer();
            std::thread::scope(|s| {
                for _ in 0..threads {
                    let o = obs.clone();
                    let keys = keys.clone();
                    let samples = samples.clone();
                    s.spawn(move || {
                        let entity = o.handle();
                        let mut op = || match schema_mode {
                            QuentSchemaMode::Static => emit_quent_static_log(&entity, keys.len()),
                            QuentSchemaMode::Dynamic => emit_quent_dynamic_log(&entity, &keys),
                        };
                        let local = run_sampled(per, sample_every, timer_overhead_ns, &mut op);
                        samples.lock().unwrap().extend(local);
                    });
                }
            });
            let emit_t = t0.elapsed().as_secs_f64();
            drop(obs);
            emit_t
        }
        Signal::Span => {
            let obs = ctx.bench_span_observer();
            std::thread::scope(|s| {
                for _ in 0..threads {
                    let o = obs.clone();
                    let keys = keys.clone();
                    let samples = samples.clone();
                    s.spawn(move || {
                        let span = o.handle();
                        span.idle().unwrap();
                        let mut op = || match schema_mode {
                            QuentSchemaMode::Static => emit_quent_static_span(&span, keys.len()),
                            QuentSchemaMode::Dynamic => emit_quent_dynamic_span(&span, &keys),
                        };
                        let local = run_sampled(per, sample_every, timer_overhead_ns, &mut op);
                        samples.lock().unwrap().extend(local);
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
        loss_pct: Some(0.0),
        drain_ms: Some(emit_end.elapsed().as_secs_f64() * 1_000.0),
        bytes_written: None,
        write_bytes_s: None,
        call_latency_ns: take_samples(&samples),
    }
}

/// Measure the generated static schema through native Bitcode derives.
#[derive(Clone, Copy)]
enum NativeBinaryFormat {
    Bitcode,
    Raw,
}

fn measure_native_binary(
    runtime: &Runtime,
    root: &Path,
    format: NativeBinaryFormat,
    signal: Signal,
    attrs: usize,
    threads: usize,
    k: u64,
    sample_every: u64,
    timer_overhead_ns: u64,
) -> Measurement {
    let _runtime_guard = runtime.enter();
    let ctx = match format {
        NativeBinaryFormat::Bitcode => NativeBenchContext::try_new_bitcode(root),
        NativeBinaryFormat::Raw => NativeBenchContext::try_new_raw(root),
    }
    .unwrap();
    let per = (k / threads as u64).max(1);
    let total = per * threads as u64;
    let samples = Arc::new(Mutex::new(Vec::new()));
    let t0 = Instant::now();
    let emit_t = match signal {
        Signal::Log => {
            let obs = ctx.bench_log_observer();
            std::thread::scope(|s| {
                for _ in 0..threads {
                    let observer = obs.clone();
                    let samples = samples.clone();
                    s.spawn(move || {
                        let entity = observer.handle();
                        let mut op = || emit_quent_native_static_log(&entity, attrs);
                        let local = run_sampled(per, sample_every, timer_overhead_ns, &mut op);
                        samples.lock().unwrap().extend(local);
                    });
                }
            });
            let elapsed = t0.elapsed().as_secs_f64();
            drop(obs);
            elapsed
        }
        Signal::Span => {
            let obs = ctx.bench_span_observer();
            std::thread::scope(|s| {
                for _ in 0..threads {
                    let observer = obs.clone();
                    let samples = samples.clone();
                    s.spawn(move || {
                        let span = observer.handle();
                        span.idle().unwrap();
                        let mut op = || emit_quent_native_static_span(&span, attrs);
                        let local = run_sampled(per, sample_every, timer_overhead_ns, &mut op);
                        samples.lock().unwrap().extend(local);
                    });
                }
            });
            let elapsed = t0.elapsed().as_secs_f64();
            drop(obs);
            elapsed
        }
    };
    let emit_end = Instant::now();
    drop(ctx);
    let total_t = t0.elapsed().as_secs_f64();
    let operations = total as f64;
    Measurement {
        offered_recs_s: operations / emit_t,
        delivered_recs_s: Some(operations / total_t),
        loss_pct: Some(0.0),
        drain_ms: Some(emit_end.elapsed().as_secs_f64() * 1_000.0),
        bytes_written: None,
        write_bytes_s: None,
        call_latency_ns: take_samples(&samples),
    }
}

fn measure_dynamic_raw(
    runtime: &Runtime,
    root: &Path,
    signal: Signal,
    keys: &Arc<Vec<&'static str>>,
    threads: usize,
    k: u64,
    sample_every: u64,
    timer_overhead_ns: u64,
) -> Measurement {
    let _runtime_guard = runtime.enter();
    let ctx = ProducerRawBenchContext::try_new(root).unwrap();
    let per = (k / threads as u64).max(1);
    let total = per * threads as u64;
    let samples = Arc::new(Mutex::new(Vec::new()));
    let t0 = Instant::now();
    let emit_t = match signal {
        Signal::Log => {
            let obs = ctx.bench_log_observer();
            std::thread::scope(|s| {
                for _ in 0..threads {
                    let observer = obs.clone();
                    let keys = keys.clone();
                    let samples = samples.clone();
                    s.spawn(move || {
                        let entity = observer.handle();
                        let mut op = || emit_quent_dynamic_log(&entity, &keys);
                        let local = run_sampled(per, sample_every, timer_overhead_ns, &mut op);
                        samples.lock().unwrap().extend(local);
                    });
                }
            });
            let elapsed = t0.elapsed().as_secs_f64();
            drop(obs);
            elapsed
        }
        Signal::Span => {
            let obs = ctx.bench_span_observer();
            std::thread::scope(|s| {
                for _ in 0..threads {
                    let observer = obs.clone();
                    let keys = keys.clone();
                    let samples = samples.clone();
                    s.spawn(move || {
                        let span = observer.handle();
                        span.idle().unwrap();
                        let mut op = || emit_quent_dynamic_span(&span, &keys);
                        let local = run_sampled(per, sample_every, timer_overhead_ns, &mut op);
                        samples.lock().unwrap().extend(local);
                    });
                }
            });
            let elapsed = t0.elapsed().as_secs_f64();
            drop(obs);
            elapsed
        }
    };
    let emit_end = Instant::now();
    drop(ctx);
    let total_t = t0.elapsed().as_secs_f64();
    let operations = total as f64;
    Measurement {
        offered_recs_s: operations / emit_t,
        delivered_recs_s: Some(operations / total_t),
        loss_pct: Some(0.0),
        drain_ms: Some(emit_end.elapsed().as_secs_f64() * 1_000.0),
        bytes_written: None,
        write_bytes_s: None,
        call_latency_ns: take_samples(&samples),
    }
}

fn otel_value(index: usize) -> Value {
    match ATTR_KINDS[index] {
        AttrKind::String => Value::String(string_value(index).into()),
        AttrKind::I64 => Value::I64(index as i64),
        AttrKind::F64 => Value::F64(index as f64 + 0.5),
        AttrKind::Bool => Value::Bool(index % 2 == 0),
        AttrKind::StringList => Value::Array(Array::String(
            string_array(index)
                .into_iter()
                .map(StringValue::from)
                .collect(),
        )),
        AttrKind::I64List => Value::Array(Array::I64(i64_array(index))),
        AttrKind::F64List => Value::Array(Array::F64(f64_array(index))),
        AttrKind::BoolList => Value::Array(Array::Bool(bool_array(index))),
    }
}

fn otel_log_value(index: usize) -> AnyValue {
    match ATTR_KINDS[index] {
        AttrKind::String => string_value(index).into(),
        AttrKind::I64 => (index as i64).into(),
        AttrKind::F64 => (index as f64 + 0.5).into(),
        AttrKind::Bool => (index % 2 == 0).into(),
        AttrKind::StringList => string_array(index).into_iter().collect(),
        AttrKind::I64List => i64_array(index).into_iter().collect(),
        AttrKind::F64List => f64_array(index).into_iter().collect(),
        AttrKind::BoolList => bool_array(index).into_iter().collect(),
    }
}

fn otel_log_op<L: Logger>(logger: &L, keys: &[&'static str]) {
    let mut rec = logger.create_log_record();
    rec.set_severity_number(Severity::Info);
    for (i, key) in keys.iter().enumerate() {
        rec.add_attribute(*key, otel_log_value(i));
    }
    logger.emit(rec);
}

fn otel_span_op<T: Tracer>(tracer: &T, keys: &[&'static str]) {
    let mut span = tracer.start("op");
    for (i, key) in keys.iter().enumerate() {
        span.set_attribute(KeyValue::new(*key, otel_value(i)));
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
    let attrs = std::env::var("BENCH_ATTRS")
        .ok()
        .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| SUPPORTED_ATTR_COUNTS.to_vec());
    for &count in &attrs {
        assert!(
            SUPPORTED_ATTR_COUNTS.contains(&count),
            "unsupported BENCH_ATTRS value {count}; expected one of {SUPPORTED_ATTR_COUNTS:?}"
        );
    }
    attrs
}

fn env_variants() -> Option<std::collections::HashSet<String>> {
    std::env::var("BENCH_VARIANTS").ok().map(|value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|variant| !variant.is_empty())
            .map(str::to_owned)
            .collect()
    })
}

fn variant_enabled(filter: &Option<std::collections::HashSet<String>>, variant: &str) -> bool {
    filter
        .as_ref()
        .is_none_or(|enabled| enabled.contains(variant))
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
    let write_rate = row.write_bytes_s.map_or("-".to_string(), byte_rate);
    eprintln!(
        "[{:>4}/{total}] {:<26} attrs={:<3} thr={:<2} offered={:<9} tput={tput:<9} write={write_rate:<10} loss={loss:<4} p99={p99:<8} drain={drain}",
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
    let (mut offered, mut delivered, mut loss_pct, mut drain_ms) = (0.0, 0.0, 0.0, 0.0);
    let (mut bytes_written, mut write_bytes_s) = (0u64, 0.0);
    let (mut has_delivered, mut has_loss, mut has_drain, mut has_bytes, mut has_write_rate) =
        (false, false, false, false, false);
    let mut call_latency_ns = Vec::new();
    for _ in 0..reps {
        let measurement = f();
        offered += measurement.offered_recs_s;
        if let Some(value) = measurement.delivered_recs_s {
            delivered += value;
            has_delivered = true;
        }
        if let Some(value) = measurement.loss_pct {
            loss_pct += value;
            has_loss = true;
        }
        if let Some(value) = measurement.drain_ms {
            drain_ms += value;
            has_drain = true;
        }
        if let Some(value) = measurement.bytes_written {
            bytes_written = bytes_written.saturating_add(value);
            has_bytes = true;
        }
        if let Some(value) = measurement.write_bytes_s {
            write_bytes_s += value;
            has_write_rate = true;
        }
        call_latency_ns.extend(measurement.call_latency_ns);
    }
    Measurement {
        offered_recs_s: offered / reps as f64,
        delivered_recs_s: has_delivered.then(|| delivered / reps as f64),
        loss_pct: has_loss.then(|| loss_pct / reps as f64),
        drain_ms: has_drain.then(|| drain_ms / reps as f64),
        bytes_written: has_bytes.then(|| bytes_written / reps as u64),
        write_bytes_s: has_write_rate.then(|| write_bytes_s / reps as f64),
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
    let variant_filter = env_variants();
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
    let bitcode_dir = tempfile::tempdir().unwrap();
    let raw_dir = tempfile::tempdir().unwrap();
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

    // tracing: JSON to a file via a background writer with backpressure, counted
    // by newlines. Lossless mode makes this comparable to quent's file paths.
    // Filtered to the `bench_trace` target so quent/OTel internal logs are neither
    // written nor counted nor charged for.
    let trace_delivered = Arc::new(AtomicU64::new(0));
    let trace_file = Arc::new(Mutex::new(
        File::create(otel_dir.path().join("tracing.jsonl")).unwrap(),
    ));
    let (trace_nb, trace_guard) =
        NonBlockingBuilder::default()
            .lossy(false)
            .finish(CountingWriter {
                inner: trace_file.clone(),
                counter: trace_delivered.clone(),
            });
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .json()
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_writer(trace_nb)
                .with_filter(
                    filter::Targets::new().with_target("bench_trace", tracing::Level::INFO),
                ),
        )
        .init();

    let cells = attrs.len() * thread_list.len() * 30;
    eprintln!(
        "max throughput: threads={thread_list:?}, {reps} rep(s), {k} ops/cell, latency sample 1/{latency_every}, {} cell-runs...",
        cells * reps
    );

    // quent backend options; a fresh context per lossless cell (drop = flush).
    let quent_backends: Vec<(&str, Option<ExporterOptions>, Option<&Path>)> = vec![
        ("noop", None, None),
        (
            "bitcode",
            Some(fs_opts(FileSystemFormat::Bitcode, bitcode_dir.path())),
            Some(bitcode_dir.path()),
        ),
        (
            "raw",
            Some(fs_opts(FileSystemFormat::Raw, raw_dir.path())),
            Some(raw_dir.path()),
        ),
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
                for (schema_mode, schema_label) in [
                    (QuentSchemaMode::Static, "static"),
                    (QuentSchemaMode::Dynamic, "dynamic"),
                ] {
                    for (signal, sl) in [(Signal::Log, "log"), (Signal::Span, "span")] {
                        let label = format!("quent-{schema_label}-{sl}/{bname}");
                        if !variant_enabled(&variant_filter, &label) {
                            continue;
                        }
                        let Measurement {
                            offered_recs_s,
                            delivered_recs_s,
                            loss_pct,
                            drain_ms,
                            bytes_written,
                            write_bytes_s,
                            call_latency_ns,
                        } = average(reps, || {
                            let bytes_before = output_dir.map_or(0, directory_bytes);
                            let mut measurement = if matches!(*bname, "bitcode" | "raw")
                                && matches!(schema_mode, QuentSchemaMode::Static)
                            {
                                measure_native_binary(
                                    &rt,
                                    output_dir.unwrap(),
                                    if *bname == "bitcode" {
                                        NativeBinaryFormat::Bitcode
                                    } else {
                                        NativeBinaryFormat::Raw
                                    },
                                    signal,
                                    n,
                                    threads,
                                    k,
                                    latency_every,
                                    timer_overhead_ns,
                                )
                            } else if *bname == "raw"
                                && matches!(schema_mode, QuentSchemaMode::Dynamic)
                            {
                                measure_dynamic_raw(
                                    &rt,
                                    output_dir.unwrap(),
                                    signal,
                                    &keys,
                                    threads,
                                    k,
                                    latency_every,
                                    timer_overhead_ns,
                                )
                            } else {
                                measure_lossless(
                                    &rt,
                                    opts.clone(),
                                    schema_mode,
                                    signal,
                                    &keys,
                                    threads,
                                    k,
                                    latency_every,
                                    timer_overhead_ns,
                                )
                            };
                            if let Some(output_dir) = output_dir {
                                let bytes =
                                    directory_bytes(output_dir).saturating_sub(bytes_before);
                                let operations = (k / threads as u64).max(1) * threads as u64;
                                attach_file_bytes(&mut measurement, bytes, operations);
                            }
                            measurement
                        });
                        let (call_p50_ns, call_p95_ns, call_p99_ns) =
                            latency_percentiles(call_latency_ns);
                        record(
                            &mut rows,
                            &mut idx,
                            cells,
                            Row {
                                label,
                                n,
                                threads,
                                offered_recs_s,
                                tput_recs_s: if is_noop { None } else { delivered_recs_s },
                                loss_pct: if is_noop { None } else { loss_pct },
                                call_p50_ns,
                                call_p95_ns,
                                call_p99_ns,
                                drain_ms: if is_noop { None } else { drain_ms },
                                bytes_written: if is_noop { None } else { bytes_written },
                                write_bytes_s: if is_noop { None } else { write_bytes_s },
                            },
                        );
                        if let Some(output_dir) = output_dir {
                            clear_temp_output(output_dir);
                        }
                    }
                }
            }

            // OTel: saturate its native lossy queue and measure sink goodput.
            let otel_log = [("noop", None::<&AtomicU64>), ("grpc", Some(&*otlp_logs))];
            for (bname, counter) in otel_log {
                let label = format!("otel-log/{bname}");
                if !variant_enabled(&variant_filter, &label) {
                    continue;
                }
                let Measurement {
                    offered_recs_s,
                    delivered_recs_s,
                    loss_pct,
                    drain_ms,
                    call_latency_ns,
                    ..
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
                        1,
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
                        label,
                        n,
                        threads,
                        offered_recs_s,
                        tput_recs_s: delivered_recs_s,
                        loss_pct,
                        call_p50_ns,
                        call_p95_ns,
                        call_p99_ns,
                        drain_ms,
                        bytes_written: None,
                        write_bytes_s: None,
                    },
                );
            }

            let otel_span = [("noop", None::<&AtomicU64>), ("grpc", Some(&*otlp_spans))];
            for (bname, counter) in otel_span {
                let label = format!("otel-span/{bname}");
                if !variant_enabled(&variant_filter, &label) {
                    continue;
                }
                let Measurement {
                    offered_recs_s,
                    delivered_recs_s,
                    loss_pct,
                    drain_ms,
                    call_latency_ns,
                    ..
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
                        1,
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
                        label,
                        n,
                        threads,
                        offered_recs_s,
                        tput_recs_s: delivered_recs_s,
                        loss_pct,
                        call_p50_ns,
                        call_p95_ns,
                        call_p99_ns,
                        drain_ms,
                        bytes_written: None,
                        write_bytes_s: None,
                    },
                );
            }

            // tracing: lossless JSON file. Formatting happens on caller threads;
            // the bounded appender applies backpressure instead of dropping.
            if variant_enabled(&variant_filter, "tracing-log/file") {
                let Measurement {
                    offered_recs_s,
                    delivered_recs_s,
                    loss_pct,
                    drain_ms,
                    bytes_written,
                    write_bytes_s,
                    call_latency_ns,
                } = average(reps, || {
                    let factory =
                        || Box::new(move || tracing_log_static(n)) as Box<dyn FnMut() + Send>;
                    let mut measurement = measure_goodput(
                        threads,
                        k,
                        1,
                        Some(&trace_delivered),
                        &factory,
                        latency_every,
                        timer_overhead_ns,
                    );
                    let bytes = trace_file.lock().unwrap().metadata().unwrap().len();
                    let operations = (k / threads as u64).max(1) * threads as u64;
                    attach_file_bytes(&mut measurement, bytes, operations);
                    clear_trace_output(&trace_file);
                    measurement
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
                        loss_pct,
                        call_p50_ns,
                        call_p95_ns,
                        call_p99_ns,
                        drain_ms,
                        bytes_written,
                        write_bytes_s,
                    },
                );
            }

            if variant_enabled(&variant_filter, "tracing-span/file") {
                let Measurement {
                    offered_recs_s,
                    delivered_recs_s,
                    loss_pct,
                    drain_ms,
                    bytes_written,
                    write_bytes_s,
                    call_latency_ns,
                } = average(reps, || {
                    let factory =
                        || Box::new(move || tracing_span_static(n)) as Box<dyn FnMut() + Send>;
                    let mut measurement = measure_goodput(
                        threads,
                        k,
                        2,
                        Some(&trace_delivered),
                        &factory,
                        latency_every,
                        timer_overhead_ns,
                    );
                    let bytes = trace_file.lock().unwrap().metadata().unwrap().len();
                    let operations = (k / threads as u64).max(1) * threads as u64;
                    attach_file_bytes(&mut measurement, bytes, operations);
                    clear_trace_output(&trace_file);
                    measurement
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
                        loss_pct,
                        call_p50_ns,
                        call_p95_ns,
                        call_p99_ns,
                        drain_ms,
                        bytes_written,
                        write_bytes_s,
                    },
                );
            }
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
    println!("  tput/s     sustained throughput -- quent/tracing: LOSSLESS, ops / full time");
    println!("             to deliver; OTel: GOODPUT, delivered/s (rest dropped)");
    println!("  loss%      fraction dropped (0% for lossless paths; '-' = uncounted floor)");
    println!("  p50/p95/p99 sampled caller-side operation latency under load");
    println!("  drain      time from the end of emission until the pipeline drains");
    println!("  write/s    logical file bytes / full lossless delivery time (no fsync)");
    println!();
    println!(
        "{:<26} {:>5} {:>4} {:>13} {:>13} {:>12} {:>7} {:>9} {:>9} {:>9} {:>10}",
        "variant",
        "attrs",
        "thr",
        "offered/s",
        "tput/s",
        "write/s",
        "loss%",
        "p50",
        "p95",
        "p99",
        "drain"
    );
    println!("{}", "-".repeat(127));
    for r in &rows {
        let tput = r.tput_recs_s.map_or("-".to_string(), si);
        let loss = r.loss_pct.map_or("-".to_string(), |l| format!("{l:.0}%"));
        let p50 = r.call_p50_ns.map_or("-".to_string(), duration_ns);
        let p95 = r.call_p95_ns.map_or("-".to_string(), duration_ns);
        let p99 = r.call_p99_ns.map_or("-".to_string(), duration_ns);
        let drain = r.drain_ms.map_or("-".to_string(), duration_ms);
        let write_rate = r.write_bytes_s.map_or("-".to_string(), byte_rate);
        println!(
            "{:<26} {:>5} {:>4} {:>13} {:>13} {:>12} {:>7} {:>9} {:>9} {:>9} {:>10}",
            r.label,
            r.n,
            r.threads,
            si(r.offered_recs_s),
            tput,
            write_rate,
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
        "{:<26} {:>5} {:>13} {:>6}",
        "variant", "attrs", "tput/s", "@thr"
    );
    println!("{}", "-".repeat(53));
    for ((n, label), (v, thr)) in &peak {
        println!("{label:<26} {n:>5} {:>13} {thr:>6}", si(*v));
    }

    // Optional raw CSV for plotting (BENCH_CSV=path).
    if let Ok(path) = std::env::var("BENCH_CSV") {
        let mut csv = String::from(
            "variant,attrs,threads,offered_ops_s,tput_ops_s,bytes_written,write_bytes_s,loss_pct,call_p50_ns,call_p95_ns,call_p99_ns,drain_ms\n",
        );
        for r in &rows {
            let tput = r.tput_recs_s.map_or(String::new(), |v| v.to_string());
            let loss = r.loss_pct.map_or(String::new(), |v| v.to_string());
            let p50 = r.call_p50_ns.map_or(String::new(), |v| v.to_string());
            let p95 = r.call_p95_ns.map_or(String::new(), |v| v.to_string());
            let p99 = r.call_p99_ns.map_or(String::new(), |v| v.to_string());
            let drain = r.drain_ms.map_or(String::new(), |v| v.to_string());
            let bytes = r.bytes_written.map_or(String::new(), |v| v.to_string());
            let write_rate = r.write_bytes_s.map_or(String::new(), |v| v.to_string());
            csv.push_str(&format!(
                "{},{},{},{},{tput},{bytes},{write_rate},{loss},{p50},{p95},{p99},{drain}\n",
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
                     \"offered_ops_s\":{},\"throughput_ops_s\":{},\"bytes_written\":{},\"write_bytes_s\":{},\"loss_pct\":{},\
                     \"call_p50_ns\":{},\"call_p95_ns\":{},\"call_p99_ns\":{},\"drain_ms\":{}}}",
                    r.label,
                    family,
                    r.n,
                    r.threads,
                    r.offered_recs_s,
                    num(r.tput_recs_s),
                    integer(r.bytes_written),
                    num(r.write_bytes_s),
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
             \"throughput_ops_s\":\"sustained rate. quent/tracing: lossless, ops / full time to deliver everything. otel: goodput = delivered/s, the rest dropped. null = no counted sink (noop)\",\
             \"bytes_written\":\"logical bytes in files after the drained cell; null for non-file sinks\",\
             \"write_bytes_s\":\"logical file bytes divided by full lossless delivery time; files are flushed but not fsynced, so this is not direct device telemetry\",\
             \"loss_pct\":\"percent of attempted ops that never reached the sink; 0 for lossless paths; null = uncounted (noop)\",\
             \"call_p50_ns\":\"sampled caller-side operation latency, 50th percentile\",\
             \"call_p95_ns\":\"sampled caller-side operation latency, 95th percentile\",\
             \"call_p99_ns\":\"sampled caller-side operation latency, 99th percentile\",\
             \"drain_ms\":\"time from the end of caller emission until the counted pipeline drains; null = no counted sink\"}},\
             \"payload\":{{\"supported_attribute_counts\":[0,1,2,4,8,16,32,64],\
             \"types\":[\"string\",\"i64\",\"f64\",\"bool\",\"list<string>\",\"list<i64>\",\"list<f64>\",\"list<bool>\"],\
             \"string_length_bytes\":[1,7,31,127,511,2047,8191],\"array_lengths\":[0,1,3,8,16,32],\
             \"tracing_array_representation\":\"Debug field because tracing has no typed array field API\"}},\
             \"note\":\"quent-static uses generated schemas; quent-dynamic measures the opt-in dynamic container separately. offered > throughput means the caller outran an asynchronous pipeline; quent drains, tracing applies backpressure and drains, and otel may drop\"}}"
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
