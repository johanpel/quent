# Quent isolated optimization experiment

## Provenance

- Upstream base: `f884b337b9108f6fb06af28d0f9c7c9d852df4a3`
- Baseline branch: `perf-isolation-benchmark`
- Baseline worktree: `/tmp/quent-perf-isolation/base`
- Candidate branches: `perf-isolation-01-*` through `perf-isolation-14-*`
- Candidate worktrees: `/tmp/quent-perf-isolation/candidate-01` through
  `/tmp/quent-perf-isolation/candidate-14`

All candidates start from the completed benchmark-only baseline commit. No
candidate may incorporate another candidate branch. If a candidate needs
enabling code, that code must be minimal, documented, and included in the
candidate's measured delta.

## Candidate list

1. Producer-side serialization into reusable producer-local byte batches.
2. Bounded lossless instrumentation queues with backpressure.
3. Simple length-framed, fixed-width Bincode raw exporter/importer.
4. Batched channel handoffs and exporter operations.
5. Remove intermediate per-event collector forwarding.
6. Batch existing filesystem formats.
7. Make generated static schemas the primary supported path.
8. Reduce dynamic-attribute allocations.
9. Reuse codec state and output buffers.
10. Native codec derives for generated schemas.
11. Thresholded Rayon encoding for sufficiently large batches.
12. Optional compact Bitcode exporter/importer.
13. Explicit flush and shutdown lifecycle support.
14. Identifier/entity-lifecycle cost audit and any justified mainline fix.

## Agent rules

- Read applicable `AGENTS.md` files before editing.
- Work only in the assigned candidate worktree and branch.
- Do not merge, rebase, cherry-pick another candidate, push, or modify remotes.
- Do not run performance benchmarks, release sweeps, profilers, or throughput
  measurements while implementation agents operate concurrently.
- Compile, format, lint, unit tests, round-trip tests, and tiny functional smoke
  runs are allowed only through `pixi run`.
- Preserve losslessness, event ordering, and complete shutdown semantics.
- Commit the candidate implementation and report exact checks and limitations.

## Sequential measurement rules

Only the root agent performs performance measurements, after all candidate
agents finish and no build/test/benchmark process remains. Baseline and
candidates run one at a time with identical workload and environment. Each run
records commit, configuration, duration, temperature, memory, disk space,
logical bytes, and any available physical I/O counters.

