# Instrumentation latency benchmarks

## Run

From the repository root:

```sh
pixi run cargo run --release -p quent-bench -- --frameworks quent --empty-loop --threads 1,2,4 --num-batches 100 --batch-size 20 --num-warmup-batches 10 --batch-pause-us 10
```

The rig builds implementations when needed and runs each case in a separate
process. Cases run sequentially so their global resources do not overlap. It
prints a summary table and writes a JSON report.

| Argument               | Default                                                      | Meaning                                                                                                     |
| ---------------------- | ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| `--frameworks`         | `quent`                                                      | Select emitting frameworks; accepts a comma-separated list.                                                |
| `--empty-loop`         | Off                                                          | Add an empty-loop control for each language among the selected frameworks.                                  |
| `--event-shape`        | `empty,u8,u64,short-string,long-string,all`                  | Select event payloads; accepts a comma-separated list. Controls do not emit events.                         |
| `--threads`            | `1`                                                          | Concurrent instrumentation threads per case; accepts comma-separated positive counts.                       |
| `--num-batches`        | `100`                                                        | Measured batches per thread.                                                                                |
| `--batch-size`         | `20`                                                         | Calls or control iterations per thread in each batch, including warmup batches.                             |
| `--num-warmup-batches` | `10`                                                         | Batches run before measurement.                                                                             |
| `--batch-pause-us`     | `10`                                                         | Minimum per-thread busy wait between batches, in microseconds.                                              |
| `--no-preflight-call`  | Off                                                          | Skip the one untimed call or control iteration per thread before batching.                                  |
| `--output PATH`        | `target/quent-latency/results-YYYY-MM-DD-HH-MM-SS-pidN.json` | JSON report path; default timestamp uses local time.                                                        |

### Framework-specific options

#### Quent

| Argument           | Default                        | Meaning                                            |
| ------------------ | ------------------------------ | -------------------------------------------------- |
| `--quent-exporter` | `noop,ndjson,msgpack,postcard` | Select exporters; accepts a comma-separated list.  |

Available `--quent-exporter` values:

| Value      | Behavior                                                       |
| ---------- | -------------------------------------------------------------- |
| `noop`     | Discards events.                                               |
| `ndjson`   | Writes NDJSON to temporary files.                              |
| `msgpack`  | Writes length-prefixed MessagePack records to temporary files. |
| `postcard` | Writes length-prefixed Postcard records to temporary files.    |

Quent creates one context and observer pipeline per case, with one entity
handle per thread. File exporters serialize and write during measurement;
draining and event-count verification happen afterward. Counts include warmup
and preflight calls. The no-op path may drop prepared strings during timing.

### Event shapes

Each instrumentation iteration emits one event. The shapes specify its explicit
attributes; an implementation may also supply implicit metadata. Quent emits
`instr_call` and adds a timestamp and the entity handle's UUID, including for
`empty`.

Let `i = (batch number * batch size + position in batch) mod 2^64`, starting at
zero. Warmup batches advance `i`; each thread uses the same sequence. The
preflight call uses `i = 0`. Payload values are prepared outside timing.

| Shape          | Explicit attributes                                        | Values                                                                              |
| -------------- | ---------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `empty`        | None                                                       | No explicit payload.                                                                |
| `u8`           | `value: u8`                                                | `i mod 256`.                                                                        |
| `u64`          | `value: u64`                                               | `i`.                                                                                |
| `short-string` | `value: string`                                            | `"s"` repeated `8 + (i mod 9)` times (8-16 bytes).                                  |
| `long-string`  | `value: string`                                            | `"l"` repeated `128 + (i mod 129)` times (128-256 bytes).                           |
| `all`          | `small: u8`, `large: u64`, `short: string`, `long: string` | `small = i mod 256`; `large = i`; `short` and `long` use the string formulas above. |

## Measurement

Each thread makes one untimed preflight call by default. Threads meet at a
barrier before each batch. Two clock reads bracket the calls in each measured
batch; warmup batches, payload preparation, and pauses are outside timing.
After a batch, each thread busy-waits for at least `--batch-pause-us` before
preparing the next batch. Preparation and synchronization can lengthen the gap.

The reported mean is `sum(thread_batch_elapsed_ns) / (threads * num_batches *
batch_size)`: nanoseconds per instrumentation call, or per control iteration.
The table also shows sample standard deviation and nearest-rank p50/p95/p99 of
batch averages across threads. JSON retains these statistics, per-thread batch
durations, and system properties. Standard deviation is zero for one measured
batch.

### Interpreting the result

A batch average is not an individual-call latency. Two clock reads are spread
across each batch, but loop and payload-handling costs remain in the result.
The empty-loop control uses the same iteration count and thread timing method,
with `black_box(())` per iteration. Its code generation can differ from an
emitter's loop, so its result is not subtracted. Batch variation does not
identify the variation or cause of individual calls.

## Layout

- `rig/`: case selection, process isolation, and reporting.
- `types/`: shared result fields and event shapes.
- `models/`: schemas for the `instr_call` event.
- `implementations/<language>/`: implementation executables and shared language
  code.
- `plots/`: report consumers.

## Other frameworks of interest

Rust: `tracing`, `log`, `slog`, and
[`ticklog`](https://github.com/tensorbinge/ticklog).

## Why a custom rig?

The rig applies the same measurement protocol across languages. Matching that
protocol with separate frameworks such as Criterion, Google Benchmark, and
pyperf would be harder.
