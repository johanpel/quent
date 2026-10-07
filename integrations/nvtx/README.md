<!--
SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
SPDX-License-Identifier: Apache-2.0
-->

# NVTX integration

Captures NVTX events (ranges, marks, domains, registered strings, categories,
thread names, resources, and the core payload union) from an application and
turns them into Quent events.

The application owns its Quent `Context` and exporter, annotates its code with
the NVTX Rust API, and links a small shim so NVTX initializes capture
**in-process** — no cdylib, no `NVTX_INJECTION64_PATH`.

**Linux 64-bit only** — capture relies on the ELF weak-symbol mechanism
(`nvtx-injection` enforces this with a `compile_error!`).

## Contents

- [Crates](#crates)
- [How capture works](#how-capture-works)
- [Using it](#using-it)
- [Captured surface](#captured-surface)
- [NVTX injection bindings](#nvtx-injection-bindings)
- [Upstreaming](#upstreaming)

## Crates

| Crate | Path | Role |
|-------|------|------|
| `nvtx-events` | `events/` | The application-agnostic NVTX event **vocabulary** (`NvtxEvent` + attribute/payload types). Pure Rust, upstreamable to the NVTX Rust crates. |
| `nvtx-injection` | `injection/` | The **NVTX C ABI layer**. Fills NVTX's callback tables, copies caller-owned values into `Record`, and hands them to a sink-agnostic `Fn(Record)` hook. Attach in-process via the `static-injection` feature, or at runtime as a cdylib via `NVTX_INJECTION64_PATH`. |
| `nvtx-bridge` | `bridge/` | The **bridge**: `NvtxEventEntity` implements Quent's `EventPayload`. Its optional `capture` feature converts owned `Record` values and pulls in the Linux-only injection crate. |
| `nvtx-example` | `example/` | A runnable example that forwards NVTX events to an observer. |

## How capture works

1. The application links `nvtx-injection` with its **`static-injection`**
   feature, publishing a *strong* `InitializeInjectionNvtx2_fnptr` that
   overrides the *weak* one in NVTX's header-only client. NVTX then initializes
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

The app owns the pipeline; wiring capture is two steps — build an observer, then
install the hook. The bridge's `capture` feature supplies the conversion:

```rust
use std::sync::Arc;

// 1. The app owns its Quent context and picks the exporter.
let ctx = Context::try_new(session)?;
let options = FileSystemExporterOptions::new(FileSystemFormat::Ndjson, out_dir);
let observer = Arc::new(ctx.block_on(async {
    ctx.observer::<NvtxEventEntity>(options).await
})?);

// 2. Forward captured NVTX events into it, before the first NVTX call.
let weak_observer = Arc::downgrade(&observer);
nvtx_injection::install_hook(move |event| {
    if let Some(observer) = weak_observer.upgrade() {
        observer.emit(session, NvtxEventEntity::from(event));
    }
})?;

// 3. Ordinary app code, annotated with NVIDIA's NVTX Rust API.
nvtx::mark(c"startup");
let range = nvtx::Range::new(c"phase-1");
drop(range); // end the range before flushing

// 4. Release and flush the observer.
drop(observer);
```

Installation is one-shot. The hook remains installed after the observer is
dropped, but weak upgrades then fail and events are discarded. The hook must not
call NVTX APIs.

Stop and join NVTX-producing threads before dropping the observer if all events
must be flushed. A callback that already upgraded the weak reference may finish
before the observer and exporter are released.

`static-injection` is requested in the manifest:

```toml
nvtx-injection = { path = "…/injection", features = ["static-injection"] }
nvtx-bridge = { path = "…/bridge", features = ["capture"] }
nvtx = { version = "2", default-features = false, features = ["std"] }
```

The bundled example uses a **callback exporter** to debug-print each captured
event (a real app would swap in ndjson, the collector, …). Run it — no GPU:

```sh
pixi run cargo run -p nvtx-example
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
pixi run cargo test -p nvtx-bridge --features capture
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

## Upstreaming

`nvtx-injection` already sources its ABI definitions from upstream
`nvtx-sys/tools`. Its capture logic and the application-agnostic `nvtx-events`
vocabulary remain candidates for contribution to NVIDIA/NVTX. Parallel effort,
not a blocker.
