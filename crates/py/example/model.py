# SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

"""Author the demo event model in Python and generate the instrumentation.

Run with the `quent` module installed (``maturin develop -m crates/py/Cargo.toml``):

    python crates/py/example/model.py
"""

from pathlib import Path

import quent

m = quent.Model("demo")


@quent.record(m)
class Endpoint:
    host: "string"
    port: "u16"


@quent.record(m)
class Meta:
    tags: "list<string>"
    extra: "dynamic"


@quent.entity(m)
class Connection:
    opened = quent.once(peer="Endpoint", session="uuid")
    data = quent.multi(bytes="u64", meta="option<Meta>")
    closed = quent.once()


def main() -> None:
    out = Path(__file__).parent / "generated"
    out.mkdir(exist_ok=True)

    # Validates through the full Rust constraint stack, then runs
    # quent-instrumentation-build.
    path = m.generate_rust(str(out))
    print(f"instrumentation library written to {path}\n")
    print(Path(path).read_text())


if __name__ == "__main__":
    main()
