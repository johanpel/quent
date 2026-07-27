<!-- SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved. -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Schema Explorer

Experimental application for exploring Quent entity graphs, resource
timelines, state machines, records, and YAML models.

From the repository root:

```sh
pixi run pnpm --dir ui install
pixi run pnpm --dir ui --filter @quent/schema-viewer build
pixi run pnpm --dir ui schema:explorer
```

Open the local URL printed by Vite, normally `http://localhost:5173`.

## YAML WebAssembly

The editor parses YAML through a browser build of `quent-yaml`. Regenerate the
checked-in bindings after changing that crate:

```sh
rustup target add wasm32-unknown-unknown --toolchain 1.97.0
cargo install wasm-bindgen-cli --version 0.2.126 --locked
pixi run pnpm --dir ui --filter @quent-experimental/schema-explorer wasm:build
pixi run pnpm --dir ui --filter @quent-experimental/schema-explorer wasm:test
```
