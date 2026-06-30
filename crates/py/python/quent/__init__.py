# SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

"""Pythonic event-model authoring on top of the Rust builder + validation stack.

Two styles are available:

- Imperative (the low-level bindings)::

    m = quent.Model("demo")
    m.record("Endpoint", host="string", port="u16")
    conn = m.entity("Connection")
    conn.once("opened", peer="Endpoint", session="uuid")

- Declarative (the sugar in this module)::

    @quent.record(m)
    class Endpoint:
        host: "string"
        port: "u16"

    @quent.entity(m)
    class Connection:
        opened = quent.once(peer="Endpoint", session="uuid")
        data = quent.multi(bytes="u64", meta="option<Meta>")
"""

from quent._quent import Entity, Model

__all__ = ["Model", "Entity", "once", "multi", "record", "entity"]


class _Event:
    """An event descriptor produced by :func:`once` / :func:`multi`."""

    def __init__(self, cardinality: str, payload: dict[str, str]):
        self.cardinality = cardinality
        self.payload = payload


def once(**payload: str) -> _Event:
    """A once (zero-or-one) event whose payload is `name=type` kwargs."""
    return _Event("once", payload)


def multi(**payload: str) -> _Event:
    """A multi (zero-or-more) event whose payload is `name=type` kwargs."""
    return _Event("multi", payload)


def record(model: Model):
    """Class decorator: declare a record from annotated `name: "type"` fields."""

    def deco(cls):
        fields = {}
        for name, ann in getattr(cls, "__annotations__", {}).items():
            if not isinstance(ann, str):
                raise TypeError(f"field {name!r} type must be a string type expression")
            fields[name] = ann
        model.record(cls.__name__, **fields)
        return cls

    return deco


def entity(model: Model):
    """Class decorator: declare an entity from `once()` / `multi()` members."""

    def deco(cls):
        handle = model.entity(cls.__name__)
        for name, value in vars(cls).items():
            if isinstance(value, _Event):
                emit = handle.once if value.cardinality == "once" else handle.multi
                emit(name, **value.payload)
        return cls

    return deco
