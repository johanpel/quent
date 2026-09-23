# Instrumentation call latency benchmarks

This directory compares the latency of the smallest Quent instrumentation call
with popular logging and OpenTelemetry APIs in C++, Rust, and Python. It is a
microbenchmark of the caller-visible emission path, not a throughput or storage
benchmark. It contains separate no-op and text-output suites and reports both
the selected main-branch Quent baseline and the active optimized checkout.

Each implementation runs one timed batch of 1,000,000 calls. The reported
latency is the total batch time divided by the call count. This amortizes timer
and harness overhead; it reports an average rather than a per-call latency
distribution.

Every framework/output-mode pair runs in a fresh operating-system process. That
process constructs only the selected framework, performs its preflight calls,
measures the batch, completes framework-specific flushing or shutdown, and
exits before the next measurement starts. Text benchmarks write to a fresh
temporary export directory for each process. Background threads, global
subscribers, handlers, providers, and allocator state therefore cannot carry
from one measurement into another.

## Measured operation

[`model.yaml`](model.yaml) defines one entity and one multi-emitting event named
`tick`. The event has no attributes. Every benchmark constructs the Quent
context and entity handle before timing begins, performs two preflight calls,
and times only `entity.tick()`.

The no-op suite dispatches every framework to a no-op or discard sink. The
text-output suite uses NDJSON for Quent and a native JSON exporter where the
comparison framework provides one; otherwise it uses native or equivalent text
file output. UUID creation and conversion happen before timing. The timestamp
call is included where the API requires the caller to supply a timestamp. A
separate `clock` result gives the platform's timestamp-call cost in each suite.
Frameworks retain their normal dispatch model, so asynchronous frameworks
measure enqueue latency while synchronous frameworks include formatting and
writes in the text-output call. Quent and Quill serialize and write on their
background workers and drain after timing. Quill uses a preallocated bounded
queue and periodically drains it outside timing so queue saturation does not
become part of the measured operation.

Text formats are:

| Language | Framework | Output |
| --- | --- | --- |
| All | Quent | NDJSON |
| Rust | tracing | `tracing-subscriber` formatted text |
| Rust | log, slog | Timestamped structured text |
| Rust | OpenTelemetry | Text exporter |
| C++ | Quill | JSON |
| C++ | spdlog | Default formatted text |
| C++ | OpenTelemetry | OStream exporter |
| Python | structlog, Loguru | JSON |
| Python | logging | Formatted text |
| Python | OpenTelemetry | Console JSON exporter |

Timestamp capture is part of every timed comparison. Quent main uses the clock
implementation at the selected baseline revision. The optimized checkout captures a raw
hardware-counter timestamp on supported CPUs and converts it to Unix nanoseconds on its
forwarder; unsupported architectures use its system-clock path. The Rust
`tracing`, `log`, and `slog` sinks capture timestamps; their core record types do
not contain them. Quill is measured twice: its default TSC timestamp is captured
on the caller thread and converted to epoch time on the backend, while its
system-clock mode calls `system_clock::now()` on the caller thread. spdlog,
Python `logging`, and Loguru capture timestamps while creating their native
records. The structlog processor calls `time_ns()` and includes it in JSON
output. Each OpenTelemetry case leaves both timestamp fields unset so the SDK
captures its native observed timestamp during emission.

Quent carries the UUID as its native 128-bit event-envelope ID. Comparisons use
their native structured context or record field with the UUID string prepared
before timing. The sinks consume that field so it cannot be removed as unused
work.

| Language | Harness | Comparisons |
| --- | --- | --- |
| Rust | `Instant` batch timer | Quent, `tracing`, `log`, `slog`, OpenTelemetry Logs |
| C++ | Google Benchmark | Quent, spdlog, Quill TSC/system clocks, OpenTelemetry Logs |
| Python | pyperf | Quent, standard `logging`, structlog, Loguru, OpenTelemetry Logs |

Only public Quent model-generation and instrumentation APIs are used. The same
model generates the Rust API and the public C++ and Python bindings.

## Run

Install [Pixi](https://pixi.sh/), then run from the repository root:

```console
pixi run --manifest-path experimental/vibe/bench/pixi.toml bench
```

The task resolves `upstream/main`, archives that revision into a temporary
directory, copies the same benchmark harness into it, and measures its Quent
implementation before rebuilding and measuring the active checkout. Override
the baseline with `python experimental/vibe/bench/report.py run --baseline-ref
REF`. Neither build modifies the working tree.

The task writes a timestamped directory under
`experimental/vibe/bench/results/`. Each result contains:

- `raw/`: current-checkout native results plus Quent-only main results under
  `raw/main/`;
- `results.json`: the versioned normalized result document;
- `results.csv`: the normalized benchmark rows;
- `latency.pdf`: one linear-scale average-latency panel per language and output
  mode, with grid lines, the call count shown beside every implementation, and
  a measurement legend describing the timed work and captured data.

Full runs execute exactly 1,000,000 measured calls per framework/output-mode
pair. The resolved main commit is recorded in `results.json` and displayed in
the PDF legend. Smoke tests execute one measured call against only the active
checkout so they remain compilation and integration checks rather than
performance runs. Text smoke runs also require a non-empty output file from
every non-clock measurement.

Other tasks are:

```console
pixi run --manifest-path experimental/vibe/bench/pixi.toml build
pixi run --manifest-path experimental/vibe/bench/pixi.toml smoke
pixi run --manifest-path experimental/vibe/bench/pixi.toml test
pixi run --manifest-path experimental/vibe/bench/pixi.toml report
```

`report` regenerates reports for the latest result directory. Pass an explicit
directory with `pixi run --manifest-path experimental/vibe/bench/pixi.toml
python experimental/vibe/bench/report.py report --results PATH`.

## Interpretation

Compare implementations only within the same language panel and on the same
machine. Compiler, CPU frequency scaling, power management, background load,
and framework dispatch semantics materially affect these small timings. The
single-batch result does not describe run-to-run variance; use multiple complete
runs when comparing changes.

CI runs `smoke`, which compiles every adapter, calls every benchmark once, and
validates all report parsers. CI results are not retained as performance
measurements.
