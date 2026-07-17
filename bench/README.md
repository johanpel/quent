<!--
SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
SPDX-License-Identifier: Apache-2.0
-->
# quent vs OpenTelemetry vs tracing throughput benchmark

Throughput comparison of quent's instrumentation against the OpenTelemetry 0.32
Rust SDK and `tracing`, each exercised as it is natively built. Each cell spams a
fixed number of operations (one op = one log or one span) as fast as possible
across the producer threads, then reports two rates:

- **offered** — the raw API-call rate: ops / emit time.
- **throughput** — the sustained rate. quent is lossless, so it is measured by
  emitting then dropping the context (a blocking flush) and dividing ops by the
  full time to deliver everything. OpenTelemetry and `tracing` are lossy (bounded
  queue), so this is goodput: what actually reached the sink, the rest dropped.

`offered > throughput` because the caller outruns the pipeline; for quent that
unbounded queue simply drains (0% loss), whereas OTel/tracing drop the excess.

Not part of default builds: the crate is a workspace member but excluded from
`default-members`, so its OpenTelemetry deps never touch a normal `cargo build`.

## Backends (native to each library)

- quent: `noop`, `ndjson`, `msgpack`, `postcard` (filesystem), `grpc` (collector).
- OpenTelemetry: `noop`, `grpc` (OTLP). No file exporter ships with OTel.
- tracing: `file` (JSON via a non-blocking appender).

gRPC/OTLP variants stream to a live in-process receiver; the tracing writer counts
newlines. `noop` discards everything (caller floor only).

## Run

Real loopback sockets are required (in-process gRPC receivers), so run it in a
normal shell, not a sandbox.

```sh
BENCH_ATTRS=0,64 BENCH_THREADS=1,4,16 BENCH_OPS=2000000 \
  BENCH_CSV=bench.csv cargo run -p quent-bench --release | tee run.txt
```

Knobs (env vars):

| var | default | meaning |
|--|--|--|
| `BENCH_OPS` | `2000000` | operations spammed per cell |
| `BENCH_ATTRS` | `0,64` | attribute counts to sweep (comma list) |
| `BENCH_THREADS` | `1,4,8` | producer thread counts to sweep (comma list) |
| `BENCH_REPS` | `1` | repetitions averaged per cell |
| `BENCH_CSV` | (unset) | if set, write raw results to this CSV path |

The table (stdout) reports `offered/s`, `tput/s`, `loss%` per variant, plus a
PEAK summary across the thread sweep; live progress goes to stderr.

## Plot

With `BENCH_CSV` set, plot one figure — a grid of (attributes x threads) cells,
each split into a throughput panel and an offered panel, bars colored by library
and the dropped fraction overlaid in gray:

```sh
uv run --with matplotlib bench/plot.py bench.csv          # -> plots/bench.png
# or: python bench/plot.py bench.csv myfig.png
```
