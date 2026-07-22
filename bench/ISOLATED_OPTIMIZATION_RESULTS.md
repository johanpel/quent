# Isolated Quent optimization results

## Executive summary

Fourteen proposed changes were independently implemented or audited from one
benchmark-only baseline over upstream `main`. The experiment found two credible
throughput improvements:

1. **Producer-side serialization with producer-local byte batching** delivered
   **4.57x** the throughput of ordinary consumer-side Postcard across the tested
   cells. At 64 attributes and 16 threads it delivered **2.09x to 3.75x** more
   operations per second. This is the largest result by a wide margin.
2. **Direct serialization into reusable filesystem exporter buffers** delivered
   **1.16x** geometric-mean throughput across NDJSON, MessagePack, and Postcard.
   The benefit was strongest at one producer thread and for dynamic payloads.

The first result is not free. Its bounded byte pipeline applies backpressure at
high concurrency: 64-attribute/16-thread p99 caller latency rose from roughly
0.07–0.34 ms to 5.1–17.1 ms, while final drain fell from 276–507 ms to 23–48 ms.
It should be developed as an explicit bounded fast path with documented latency
semantics, not silently substituted under an API promising cheap submission.

Several attractive ideas did not improve the stress workload in isolation:

- a simple consumer-side raw Bincode format was throughput-neutral;
- native Bincode derives helped small/low-contention cells but regressed or were
  neutral at 64 attributes and 16 threads;
- thresholded Rayon encoding reduced overall throughput and increased drain;
- compact Bitcode was slower at high concurrency and only 2.3% smaller than
  Postcard for this string/array-heavy payload;
- direct collector forwarding was strongly payload-size dependent and regressed
  the 64-attribute cells;
- a bounded typed-event queue greatly reduced memory/backlog but was not a
  throughput win.

Generic exporter batching, existing filesystem batching, flush-on-last-drop,
and persistent entity identity were already present or already correct in the
latest upstream base. They should not be claimed as new isolated wins.

## Repository layout and provenance

- Latest fetched upstream base: `f884b337b9108f6fb06af28d0f9c7c9d852df4a3`
- Benchmark-only baseline: `e8f09eb1914b0ab7a5a8df25ab311c6b2a20a19c`
- Baseline branch: `perf-isolation-benchmark`
- Baseline worktree: `/tmp/quent-perf-isolation/base`
- Candidate worktrees: `/tmp/quent-perf-isolation/candidate-01` through
  `/tmp/quent-perf-isolation/candidate-14`

Every candidate commit has `e8f09eb1` as its direct parent. No candidate merged,
rebased, or cherry-picked another candidate. All worktrees were clean before
measurement.

| Candidate | Commit | Change |
|---:|---|---|
| 01 | `6843c0f7` | Producer-side Postcard serialization and local byte batches |
| 02 | `8d9f393b` | Bounded lossless typed-event queue |
| 03 | `c4dae850` | Consumer-side raw Bincode exporter/importer |
| 04 | `c0bdd8ce` | Generic batched handoff audit—already upstream |
| 05 | `77a1f6ec` | Remove intermediate collector event forwarding |
| 06 | `3bc852ba` | Existing filesystem batching audit—already upstream |
| 07 | `73f84df8` | Generated static schemas as primary documented path |
| 08 | `4ea1675f` | Dynamic-attribute allocation reductions |
| 09 | `9937621c` | Reusable codec/output buffers |
| 10 | `0c3e7a09` | Native Bincode versus Serde Bincode |
| 11 | `6dda934b` | Thresholded Rayon batch encoding |
| 12 | `866703fe` | Compact Bitcode exporter/importer |
| 13 | `bc203702` | Flush/shutdown lifecycle audit and contract tests |
| 14 | `b1d36752` | Identifier/entity lifecycle audit |

## Method

Implementation agents were allowed to compile, lint, test, and run tiny
functionality smokes. They were explicitly prohibited from running performance
benchmarks. After every implementation was finished, the root agent compiled
all release binaries sequentially, waited for the workstation to become idle,
and measured candidates one at a time outside the restrictive sandbox.

The fixed measurement screen used:

```text
attributes: 8, 64
threads:    1, 16
operations: 50,000 per cell
repetitions: 3
latency sampling: 1 / 1,024 operations
Rayon threads: 24
```

Each run used only the affected variants plus an unchanged control. The baseline
was run at both the beginning and end. Before each candidate the runner required:

- no Cargo, rustc, or quent-bench process;
- package temperature at or below 50°C;
- dirty plus writeback pages below 64 MiB.

It called `sync` after each candidate and did not overlap runs. The complete
successful sequence took 1,274.6 seconds (21 minutes 14.6 seconds). Boundary
package temperatures stayed between 35°C and 38°C. No thermal contention was
observed. The machine used the Intel P-state `powersave` governor throughout.

The start-to-end baseline geometric-mean drift was:

| Metric | End / start |
|---|---:|
| Delivered throughput | 0.988x |
| Offered throughput | 1.053x |
| Caller p99 | 0.954x |
| Drain | 1.029x |

Documentation/no-runtime controls measured about 0.89–0.91x the bracketing
baseline for the filtered Postcard run. This reveals a variant-order/process
warm-up confound beyond the start/end drift. Effects near 10% must therefore be
treated as inconclusive unless supported by same-run controls or consistent
per-cell behavior.

## Result summary

Ratios are geometric means across matching measured cells. Lower p99 and drain
ratios are better. Candidates 01, 03, 10, and 12 use a same-run format control;
the others use the mean of baseline-start and baseline-end.

| # | Isolated result | Delivered | Offered | p99 | Drain | Decision |
|---:|---|---:|---:|---:|---:|---|
| 01 | Producer Postcard / ordinary Postcard | **4.568x** | 2.060x | 2.413x | **0.023x** | Continue; highest priority, explicit backpressure semantics |
| 02 | Bounded typed queue / baseline Postcard | 0.844x | 0.302x | 7.629x | **0.122x** | Hold as safety/opt-in control, not throughput default |
| 03 | Raw Bincode / Postcard | 1.002x | 1.118x | 0.586x | 1.110x | Drop as standalone speed optimization |
| 04 | Generic handoff batching | No runtime delta | | | | Already upstream; retain |
| 05 | Direct collector forwarding / baseline gRPC | 0.870x | 0.921x | 1.282x | 1.043x | Rework/hold; size-dependent regression |
| 06 | Existing filesystem batching | No runtime delta | | | | Already upstream; retain |
| 07 | Static-first documentation | No runtime delta | | | | Merge as guidance, not speed claim |
| 08 | Dynamic allocation reduction, Postcard | 1.027x | 0.839x | 2.021x | 0.864x | Do not merge as-is; salvage non-breaking pieces |
| 08 | Dynamic allocation reduction, noop offered rate | — | 1.246x | 0.751x | — | Construction improves, pipeline mostly hides it |
| 09 | Reusable encoding buffers / baseline files | **1.164x** | 1.019x | 0.795x | 1.008x | Merge candidate after focused confirmation |
| 10 | Native Bincode / Serde Bincode | 1.161x | 1.183x | 0.594x | 0.997x | Hold; benefit disappears at high-contention target |
| 11 | Rayon encoding / baseline files | 0.933x | 1.056x | 0.946x | 1.381x | Drop default threshold/design |
| 12 | Bitcode / Postcard | 0.933x | 1.145x | 1.061x | 1.514x | Do not merge as a speed feature |
| 13 | Flush lifecycle | No runtime delta | | | | Runtime already correct; merge contract tests |
| 14 | Entity/UUID lifecycle | No runtime delta | | | | Already correct; no production change |

## Candidate details

### 01 — Producer-side serialization and local byte batching

Candidate 01 retains standard Postcard framing but encodes on producer threads
into reusable thread-local batches. Typed payloads are destroyed on their
producer. A batch moves to one ordered writer at 64 events or 1 MiB; the byte
queue is bounded at 256 batches and recycles buffers. Explicit handle-drop hooks
flush partial batches.

At the original difficult cell:

| Variant | Ordinary tput | Producer tput | Ratio | Ordinary p99 | Producer p99 | Ordinary drain | Producer drain |
|---|---:|---:|---:|---:|---:|---:|---:|
| Static log, 64 attrs/16 threads | 121.1k/s | 314.3k/s | **2.60x** | 0.344 ms | 17.15 ms | 348 ms | 48 ms |
| Static span, 64 attrs/16 threads | 153.0k/s | 319.1k/s | **2.09x** | 0.215 ms | 5.13 ms | 276 ms | 23 ms |
| Dynamic log, 64 attrs/16 threads | 87.4k/s | 327.5k/s | **3.75x** | 0.091 ms | 12.36 ms | 507 ms | 42 ms |
| Dynamic span, 64 attrs/16 threads | 92.1k/s | 305.8k/s | **3.32x** | 0.067 ms | 7.18 ms | 467 ms | 23 ms |

At one thread, delivered throughput improved 4.02x–6.09x and caller p99 also
improved. At 16 threads, the writer becomes the limiter and the bounded byte
queue deliberately exposes that limit to callers. The dramatic drain reduction
shows that the old offered rate mostly represented deferred work.

This candidate packages several necessary mechanisms: producer encoding,
thread-local aggregation, a bounded byte queue, buffer recycling, one writer,
and explicit partial-batch flush. It isolates the complete fast-path design from
mainline, but it does not assign the gain among those internal mechanisms.

Recommendation: continue this design first. Before merging, choose and document
the backpressure contract, make batch/byte capacity configurable, propagate
writer failures, audit thread-local sink identity reuse, and benchmark steady
state rather than only finite-run drain.

### 02 — Bounded typed-event queue

The typed queue capacity defaults to 4,096 per observer. It reduced final drain
to 12.2% of baseline and reduced process peak RSS to about 297 MiB, versus
roughly 1.4–2.2 GiB in filtered Postcard no-op-control runs. It also reduced
delivered throughput by about 16%, reduced offered throughput by 70%, and raised
caller p99 substantially.

At 64 attributes/16 threads, delivered throughput was 75.6k–136.9k/s with p99
of 0.84–3.24 ms. This is useful bounded-memory behavior, but not a throughput
optimization. Keep the implementation as an opt-in/safety candidate or revisit
capacity/byte-based bounds. Do not merge the measured default claiming speed.

### 03 — Simple raw Bincode

The Serde Bincode exporter uses consumer-side encoding on the unchanged event
pipeline. Overall delivered throughput was effectively identical to Postcard
(1.002x) and file size was 1.164x. At 64 attributes/16 threads, static results
regressed while dynamic results were roughly neutral.

This is strong evidence that changing the codec/framing alone does not solve the
pipeline problem. Do not merge raw as a standalone speed feature. It could still
serve a product need for a deliberately simple format, but producer-side
Postcard proves that raw framing is not required for the main optimization.

### 04 and 06 — Batching already exists upstream

Upstream commit `24d832ad` already provides reusable observer event buffers,
`recv_many`, `batch_size_hint`, and `drain_events`. Upstream filesystem exporters
already batch NDJSON, MessagePack, and Postcard writes while preserving format
and order. These candidates are documentation controls, not changes to merge.

### 05 — Collector forwarding

Removing the intermediate event channel and 128 ms serializer/coalescer helped
several 8-attribute cells (up to 2.13x) but regressed most 64-attribute cells.
At 64 attributes/16 threads the delivered ratios ranged from 0.43x to 0.95x.
Peak RSS remained around 8 GiB.

Do not merge this form. A follow-up should retain direct observer-batch
serialization while adding size-aware coalescing, or serialize into a reusable
byte/request buffer without discarding the old request aggregation behavior.

### 07 — Static schemas as the primary path

This is documentation and example work, not a runtime change. It correctly
guides users toward generated `instrumentation-build` schemas and labels dynamic
attributes as the runtime-defined fallback. Merge on clarity merits only.

### 08 — Dynamic allocation reductions

Candidate 08 borrows literal keys through `Cow<'static, str>`, stores up to four
attributes inline with `SmallVec`, reserves exact larger capacities, constructs
dynamic values directly, and avoids an intermediate boolean-vector conversion.

The dynamic noop offered rate improved 1.246x overall and 1.50x–1.73x at
64 attributes/16 threads. Actual Postcard delivered throughput improved only
1.027x overall and was mixed at high concurrency. More importantly, the public
`String`→`Cow` and `Vec`→`SmallVec` field changes are source-breaking.

Do not merge the patch as-is. Retain the equivalence tests and separately test
non-breaking constructors/builders that reserve capacity, accept static keys,
and construct typed lists directly.

### 09 — Reusable codec/output buffers

NDJSON now serializes directly into its persistent byte buffer. MessagePack and
Postcard reserve frame prefixes, serialize directly into reusable framed
storage, patch lengths, and roll back partial output after errors. Exact legacy
bytes and ordered round trips are tested.

Delivered throughput improved 1.164x overall. One-thread cells improved
1.23x–1.44x at 8 attributes and mostly 1.11x–1.35x at 64 attributes. At
64 attributes/16 threads, dynamic variants improved 1.05x–1.24x, while static
variants ranged from 0.86x to 1.11x. The mixed high-concurrency static results
and the roughly 10% control noise warrant a focused confirmation, but the
one-thread consistency, exact-byte compatibility, and modest code scope make
this the best conventional merge candidate.

### 10 — Native Bincode derives

Native and Serde variants use identical fixed-width Bincode configuration,
framing, allocation, locking, and file writes. Native encoding improved the
geometric mean by 1.161x, driven by 8-attribute/one-thread gains of 1.47x–1.55x.
At 64 attributes/16 threads it was 0.87x for logs and 0.98x for spans.

Do not maintain dual paths solely from this result. Revisit native derives only
inside the producer-side fast path, where codec work occurs before queueing and
the contention model is different.

### 11 — Thresholded Rayon

Batches of at least 128 events move to Rayon; smaller batches encode inline.
Overall throughput fell to 0.933x and drain rose to 1.381x. Most
64-attribute/16-thread variants delivered 0.79x–0.95x baseline throughput.
The normal observer batch is 256, so nearly all steady-state work paid task
scheduling/transfer cost.

Drop this default. A byte-based threshold or dedicated encoding pool might have
a crossover for much larger batches, but it must win a new isolated test.

### 12 — Compact Bitcode

Bitcode improved several one-thread cells but regressed high concurrency. At
64 attributes/16 threads it delivered 0.41x–0.94x Postcard throughput and
increased drain. Its output was only 2.3% smaller overall because the payload is
dominated by variable strings and arrays.

Do not merge it as a throughput optimization. Keep only if compact format or
decode properties are independently required and measured.

### 13 — Flush and shutdown lifecycle

Latest upstream already uses shared observer ownership, last-drop cancellation,
ordered queue drain, a partial final batch, exactly one exporter shutdown, and a
synchronous join. Candidate 13 adds tests covering 1,025 queued events, clone
lifetime, push failure continuation, and shutdown failure. Merge the tests if
desired; there is no runtime optimization.

### 14 — Entity/UUID lifecycle

Generated handles create one UUID in `Handle::new` and reuse it for every event.
Contexts, observers, exporters, runtimes, and sidecars are also constructed
outside the event hot path. The benchmark already creates one handle per
producer. No mainline change is justified.

## Recommended upstream sequence

1. Develop candidate 01 as an explicit producer-encoded, bounded fast path.
   Preserve Postcard compatibility initially so codec and pipeline work remain
   separable. Resolve caller-latency/backpressure semantics before merging.
2. Confirm and merge candidate 09's direct reusable encoding buffers.
3. Merge candidate 07 documentation and candidate 13 contract tests as
   non-performance improvements.
4. Salvage non-breaking pieces of candidate 08 and benchmark them separately.
5. Keep candidate 02 available only if bounded memory/backpressure is a desired
   operational policy; do not advertise it as throughput work.
6. Rework collector aggregation before reconsidering candidate 05.
7. Do not merge candidates 03, 10, 11, or 12 for performance based on this
   experiment.

## Validation and limitations

- All 16 datasets were produced successfully: two baselines and 14 candidates.
- 504 CSV rows match 504 self-describing JSON rows.
- Every counted Quent row has `loss_pct == 0`.
- All branch worktrees compiled in debug and release modes before measurement.
- Candidate-specific formatting, strict Clippy, unit, roundtrip, order, shutdown,
  and tiny functionality checks passed before performance runs.
- No candidate benchmark overlapped another candidate or an implementation
  agent.
- No before/after claim uses another machine.

Limitations:

- Three repetitions are adequate for screening, not a final statistical study.
- Candidate filtering changes within-process variant warm-up/order; no-op controls
  show that effects around 10% can be noise.
- Candidate 01 measures the complete producer-fast-path package, not every
  internal mechanism separately.
- Logical file B/s is flushed but not fsynced physical-device telemetry.
- `/usr/bin/time`-style filesystem-output counters and logical byte rates do not
  establish durable NVMe bandwidth.
- The Intel P-state governor was `powersave`; it was held constant but CPU
  frequency was not pinned.
- Results apply to this deterministic mixed string/primitive-array workload and
  this workstation.

## Artifacts

- Repository-local experiment artifacts: `bench/perf-isolation-results/`
- Raw CSV/JSON:
  `bench/perf-isolation-results/{baseline-*,candidate-*}.{csv,json}`
- Complete log: `bench/perf-isolation-results/run.log`
- Machine-readable analysis: `bench/perf-isolation-results/analysis.json`
- Experiment manifest: `bench/perf-isolation-results/EXPERIMENT.md`
- Sequential runner: `bench/perf-isolation-results/run-sequential.sh`
- Analysis script: `bench/perf-isolation-results/analyze.py`
- Experiment manifest: `/tmp/quent-perf-isolation/EXPERIMENT.md`
- Runner: `/tmp/quent-perf-isolation/run-sequential.sh`
- Complete log: `/tmp/quent-perf-isolation/results/run.log`
- Machine-readable analysis: `/tmp/quent-perf-isolation/results/analysis.json`
- Raw CSV/JSON: `/tmp/quent-perf-isolation/results/{baseline-*,candidate-*}.{csv,json}`
