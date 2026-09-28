# Generated instrumentation latency benchmarks

## Run

From the repository root:

```sh
pixi run cargo run --release -p quent-latency-rig -- --frameworks quent --empty-loop --threads 1,2,4 --num-batches 100 --batch-size 20 --num-warmup-batches 10 --batch-pause-us 10
```

The benchmarking rig builds selected Rust implementations in release mode, then runs
each case once in an isolated process and sequentially to prevent the resources
held by various frameworks to contend. It prints a summary table and writes a
JSON report.

| Argument                    | Default                                                      | Meaning                                                                                                              |
| --------------------------- | ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| `--frameworks`              | `quent`                                                      | Emitting frameworks to measure; accepts a comma-separated list.                                                       |
| `--empty-loop`              | Off                                                          | Add an empty-loop control for each language among the selected frameworks.                                            |
| `--event-shape`             | `empty,u8,u64,short-string,long-string,all`                  | Event shape for the selected frameworks; ignored by the loop control. Accepts a comma-separated list.                 |
| `--threads`                 | `1`                                                          | Concurrent instrumentation threads per case; accepts comma-separated positive counts.                                |
| `--num-batches`             | `100`                                                        | Measured batches per thread.                                                                                         |
| `--batch-size`              | `20`                                                         | Iterations per thread in each batch, including warmup batches; one call or control iteration per iteration.          |
| `--num-warmup-batches`      | `10`                                                         | Batches run before measurements; their calls are excluded from timing results.                                       |
| `--batch-pause-us`          | `10`                                                         | Per-thread busy wait after each batch, in microseconds. Actual gaps may be longer.                                    |
| `--no-preflight-call`       | Off                                                          | Skip the one untimed call or control iteration per thread before the first barrier.                                   |
| `--output PATH`             | `target/quent-latency/results-YYYY-MM-DD-HH-MM-SS-pidN.json` | JSON report path; default timestamp uses local time.                                                                 |

### Framework-specific options

#### Quent

| Argument              | Default                 | Meaning                                                                       |
| --------------------- | ----------------------- | ----------------------------------------------------------------------------- |
| `--quent-exporter`    | `noop,ndjson,postcard`  | Quent exporters to measure; accepts a comma-separated list.                  |
| `--quent-binary PATH` | Automatic release build | Run an existing Quent binary; the loop control is still built automatically. |

Available `--quent-exporter` values:

| Value              | Behavior                                                    |
| ------------------ | ----------------------------------------------------------- |
| `noop`             | Discards events.                                            |
| `ndjson`           | Writes NDJSON to temporary files.                           |
| `postcard`         | Writes length-prefixed Postcard records to temporary files. |

### Event shapes

Each iteration emits one repeatable `instr_call` event. In Quent, every event
also carries an implicit timestamp and the entity handle's UUID. The table
lists only the explicit attributes.

Let `i = (batch number * batch size + position in batch) mod 2^64`, with both
positions starting at zero. Warmup batches contribute to the batch number;
each thread uses the same sequence. The optional preflight call uses `i = 0`.
Payload values are prepared before the timed section.

| Shape          | Explicit attributes                       | Values                                               |
| -------------- | ----------------------------------------- | ---------------------------------------------------- |
| `empty`        | None                                      | Only the implicit UUID and timestamp.                |
| `u8`           | `value: u8`                               | `i mod 256`.                                         |
| `u64`          | `value: u64`                              | `i`.                                                 |
| `short-string` | `value: string`                           | `"s"` repeated `8 + (i mod 9)` times (8-16 bytes).  |
| `long-string`  | `value: string`                           | `"l"` repeated `128 + (i mod 129)` times (128-256 bytes). |
| `all`          | `small: u8`, `large: u64`, `short: string`, `long: string` | `small = i mod 256`; `large = i`; `short` and `long` use the string formulas above. |

## Layout

- `rig/`: launches isolated processes and reports results.
- `types/`: shared implementation IDs, event shapes, and JSON case fields.
- `models/`: six shared YAML schemas for the `instr_call` event.
- `implementations/rust/common/`: shared Rust implementation arguments, measurement loop, and result construction.
- `implementations/rust/quent/`: generated Quent API calls.
- `implementations/rust/empty_loop/`: loop control in an isolated process.
- `plots/`: space for report consumers.

## Planned comparisons

- Rust: `tracing`, `log`, `slog`, and [`ticklog`](https://github.com/tensorbinge/ticklog).

## Measurement

- **Setup:** Quent uses one context and observer pipeline per child and one
  entity handle per thread. Every implementation makes one untimed preflight
  call or control iteration per thread by default.
- **Timing:** Threads meet at a barrier before each batch. Each measured batch
  has two clock reads around `--batch-size` calls. Warmup batches, payload
  preparation, and the pause are outside the timed sections. The reported
  value is `sum(thread_batch_elapsed_ns) / (threads * num_batches * batch_size)`:
  ns per instrumentation call for Quent, or ns per loop iteration for the control.
- **Pacing:** Each worker busy-waits after a batch, then prepares the next
  batch and meets the other workers at the barrier. The requested pause is a
  minimum; preparation and synchronization can lengthen the gap between calls.
- **Payloads:** The no-op path may drop prepared strings during timing.
- **Export:** File exporters serialize and write concurrently. Draining and
  event count verification happen after timing; counts include warmup and
  preflight calls.
- **Report:** The table prints shared settings, then mean, sample standard
  deviation, and nearest-rank p50/p95/p99 of batch averages across threads.
  JSON retains those statistics, each thread's batch durations, and system
  properties. Standard deviation is zero for one measured batch.
- **Loop control:** `--empty-loop` adds the Rust control when Quent is selected.
  It uses the same iteration count and thread timing protocol, with
  `black_box(())` once per iteration. It has no exporter or event shape. Its
  ns/iter includes loop and clock-read overhead; compiler
  code generation differs from Quent's loop, so it is not subtracted.

### Interpreting the result

- **Scope:** Each recorded duration covers one batch; dividing by batch size
  gives a batch average, not an individual call latency. Timing calls
  separately could add clock reads that exceed the cost of a fast call.
- **Timer overhead:** Two timestamps bracket each batch, spreading clock-read
  cost across `--batch-size` calls.
- **Loop overhead:** Advancing the loop and handling each payload still cost time
  per call. More calls or more timed batches do not remove that cost. If it is
  comparable to the emission call, Quent's reported ns/call cannot be interpreted
  as the call's latency alone. A representative control and call unrolling can
  test how much the loop affects the result; subtracting a control without
  validating it can be misleading.
- **Variation:** Batch results show variation in the combined call-and-loop
  average. They do not identify individual call variation or its cause.

## Why a custom rig?

The custom rig applies the same measurement protocol to every language
implementation. Keeping that protocol identical would be harder with separate
frameworks such as Criterion, Google Benchmark, and pyperf.
