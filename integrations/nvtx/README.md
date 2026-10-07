<!--
SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
SPDX-License-Identifier: Apache-2.0
-->

# NVTX integration

Captures NVTX events (ranges, marks, domains, registered strings, categories,
thread names, resources, and the core payload union) from an application and
turns them into Quent events.

The application owns its Quent context and exporter, emits NVTX annotations,
and links a small shim so NVTX initializes capture **in-process** — no cdylib or
`NVTX_INJECTION64_PATH` is needed.

**In-process capture requires 64-bit Linux.** `nvtx-injection` enforces this at
compile time. The event types, analyzer, and default bridge build without the
injection crate.

## Contents

- [Crates](#crates)
- [How capture works](#how-capture-works)
- [Using it](#using-it)
- [Captured surface](#captured-surface)
- [NVTX injection bindings](#nvtx-injection-bindings)

## Crates

| Crate                 | Path             | Role                                                                                                                                                                                                                                         |
| --------------------- | ---------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `nvtx-injection`      | `injection/`     | The **NVTX C ABI layer**, intended for upstreaming. Fills NVTX's callback tables, copies caller-owned values into `Record`, and hands them to a sink-agnostic `Fn(Record)` hook. The example links it in-process through `static-injection`. |
| `quent-nvtx-events`   | `events/`        | The NVTX event types used by the Quent bridge and analyzer.                                                                                                                                                                                  |
| `quent-nvtx-bridge`   | `bridge/`        | The **bridge**: `NvtxEventEntity` implements Quent's `EventPayload`. Its optional `capture` feature converts owned `Record` values and pulls in the Linux-only injection crate.                                                              |
| `quent-nvtx-example`  | `example/`       | A runnable example that forwards NVTX events to an observer.                                                                                                                                                                                 |
| `quent-nvtx-analyzer` | `analyzer/`      | Reconstructs ranges, names, and resources from recorded NVTX events.                                                                                                                                                                         |
| `quent-nvtx-ui`       | `ui/`            | Defines the NVTX viewport data and presentation contracts.                                                                                                                                                                                   |
| `quent-nvtx-server`   | `server/`        | Loads NVTX streams and serves the analyzer and UI data through HTTP routes.                                                                                                                                                                  |
| `nvtx-server`         | `server-compat/` | Re-exports the server API for `quent-open` viewers pinned to the former package name.                                                                                                                                                        |

## How capture works

1. The application links `nvtx-injection` with its **`static-injection`**
   feature, publishing a _strong_ `InitializeInjectionNvtx2_fnptr` that
   overrides the _weak_ one in NVTX's header-only client. NVTX then initializes
   injection **in-process** at the first NVTX call.
2. Injection claims NVTX's callback tables — our `extern "C"` functions become
   NVTX's implementation of the subscribed calls.
3. Each callback copies caller-owned NVTX data into an owned `Record` and
   calls the installed hook on the emitting thread.
4. The example's hook uses the bridge's `capture` feature to convert each record
   into `NvtxEventEntity`, adding the caller's OS thread ID to push/pop events.
   It then forwards the event to its `Observer`. Injection supplies handles,
   ids, and nesting levels even before the hook is installed, so handles an app
   caches early stay valid.
5. The hook and callback pointers stay installed for the process lifetime.
   A hook can hold a weak observer reference so the observer and exporter are
   released on drop. Late callbacks tolerate destroyed TLS.

## Using it

Create an observer, then install a hook before the NVTX events to capture. The
bridge's `capture` feature converts the records. See the
[runnable example](example/src/main.rs) for the complete setup.

Installation is one-shot. The hook remains installed after the observer is
dropped, but weak upgrades then fail and events are discarded. The hook must not
call NVTX APIs.

Stop and join NVTX-producing threads before dropping the observer if all events
must be flushed. An in-flight callback that already upgraded the weak reference
can delay observer release and exporter flush.

`static-injection` is requested in the manifest:

```toml
nvtx-injection = { path = "…/injection", features = ["static-injection"] }
quent-nvtx-bridge = { path = "…/bridge", features = ["capture"] }
nvtx = { version = "2", default-features = false, features = ["std"] }
```

The bundled example uses a **callback exporter** to debug-print each captured
event. Run it without a GPU:

```sh
pixi run cargo run -p quent-nvtx-example
```

It prints one line per event — entity `id`, capture `timestamp` (ns), and the
`NvtxEvent`:

```text
[019f… @ 1784726504601037528] Mark { domain: 0, attributes: { message: Some(String("startup")), .. } }
```

### Test

The bridge tests check record conversion, capture, thread IDs, and dropping an
observer from inside the hook:

```sh
pixi run cargo test -p quent-nvtx-bridge --features capture
```

Injection tests cover late hook installation, duplicate registration, and NVTX
calls after Rust TLS destruction:

```sh
pixi run cargo test -p nvtx-injection --features static-injection
```

These tests require no GPU.

## Captured surface

Both NVTX ASCII surfaces: **domain-scoped (CORE2)** — mark, range
start/end/push/pop, domain/register-string/name-category/resource — and the
**classic default domain (CORE)** on domain `0`, plus OS thread naming.

Default-domain wide-char (`*W`) variants are copied as owned code units, then
decoded by the bridge's `capture` feature while preserving nesting and
synthesized IDs.
Domain-scoped wide-name calls (`DomainCreateW`, `DomainRegisterStringW`, and
`DomainNameCategoryW`) are not yet subscribed.

## NVTX injection bindings

`nvtx-injection` consumes the upstream NVTX injection ABI through
[`nvtx-sys`](https://crates.io/crates/nvtx-sys) with its `tools` feature. That
feature exposes the callback tables, callback ids, injection result codes, and
function signatures needed by a tool such as Quent. `nvtx-sys` generates the
target-specific Rust declarations from the NVTX headers bundled in the crate,
so Quent no longer carries its own wrapper, bindgen allowlist, or generated
bindings file. No external NVTX installation or `CONDA_PREFIX` is required.

Because `nvtx-sys` runs bindgen and compiles its bundled C implementation, a
fresh downstream build does require a discoverable `libclang` and a native C
compiler. Quent's Pixi environment provides those on Linux through `libclang`
and `cxx-compiler`; the aarch64 environment also installs `clang` for its
compiler resource headers. Outside Pixi, install equivalent system packages
and set `LIBCLANG_PATH` only when `clang-sys` cannot discover the library.
