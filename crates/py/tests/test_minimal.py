# SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

import pytest

import quent


def build_demo() -> quent.Model:
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

    return m


def test_imperative_api():
    m = quent.Model("demo")
    m.record("Endpoint", host="string", port="u16")
    conn = m.entity("Connection")
    conn.once("opened", peer="Endpoint", session="uuid")
    conn.multi("data", bytes="u64")
    m.validate()


def test_declarative_generates(tmp_path):
    path = build_demo().generate_rust(str(tmp_path))
    assert path.endswith("demo.rs")
    src = (tmp_path / "demo.rs").read_text()
    assert "enum ConnectionEvent" in src
    assert "struct Endpoint" in src
    assert "struct Meta" in src


def test_invalid_identifier_raises():
    m = quent.Model("demo")
    with pytest.raises(ValueError):
        m.record("1bad", x="u64")


def test_unknown_record_reference_raises():
    m = quent.Model("demo")
    e = m.entity("E")
    e.once("ev", x="Missing")
    with pytest.raises(ValueError):
        m.validate()
