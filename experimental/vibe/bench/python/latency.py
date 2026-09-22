# SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

"""Measure Python instrumentation-call latency with native pyperf timing."""

from __future__ import annotations

import logging
import os
from time import time_ns
from typing import Any

import pyperf
import quent_bench as quent
import structlog
from loguru import logger as loguru_logger
from opentelemetry.sdk._logs import LoggerProvider
from opentelemetry.sdk._logs.export import (
    ConsoleLogRecordExporter,
    LogRecordExporter,
    LogRecordExportResult,
    SimpleLogRecordProcessor,
)


class DiscardStructlogLogger:
    """Accept structured log calls without producing output."""

    def info(self, event: str, **kw: Any) -> None:
        del event, kw


class DiscardLogRecordExporter(LogRecordExporter):
    """Accept OpenTelemetry records synchronously without storing them."""

    def export(self, batch: Any) -> LogRecordExportResult:
        _ = len(batch)
        return LogRecordExportResult.SUCCESS

    def shutdown(self) -> None:
        pass

    def force_flush(self, timeout_millis: int = 10_000) -> bool:
        del timeout_millis
        return True


def capture_timestamp(
    logger: Any, method_name: str, event_dict: dict[str, Any]
) -> dict[str, Any]:
    """Capture an event timestamp without adding it as an attribute."""
    del logger, method_name
    _ = time_ns()
    return event_dict


def capture_timestamp_field(
    logger: Any, method_name: str, event_dict: dict[str, Any]
) -> dict[str, Any]:
    """Capture an event timestamp as structured output data."""
    del logger, method_name
    event_dict["timestamp_ns"] = time_ns()
    return event_dict


def export_path(filename: str) -> str:
    """Return a file path in the isolated output directory."""
    return os.path.join(os.environ["QUENT_BENCH_EXPORT_DIR"], filename)


def main() -> None:
    implementation = os.environ["QUENT_BENCH_IMPLEMENTATION"]
    output_mode = os.environ["QUENT_BENCH_OUTPUT_MODE"]
    if output_mode not in {"noop", "text"}:
        raise ValueError(f"unknown output mode: {output_mode}")
    prefix = f"latency/python/{output_mode}"
    runner = pyperf.Runner(processes=1, values=1, loops=1_000_000, warmups=0)
    runner.metadata["description"] = "Instrumentation call latency"
    if implementation == "clock":
        runner.timeit(f"{prefix}/clock", "time_ns()", globals={"time_ns": time_ns})
    elif implementation == "quent":
        context = (
            quent.Context()
            if output_mode == "noop"
            else quent.Context(quent.ExporterOptions.ndjson(export_path("quent")))
        )
        entity = context.entity_observer().handle()
        entity.tick()
        entity.tick()
        runner.timeit(f"{prefix}/quent", "entity.tick()", globals={"entity": entity})
        del entity
        context.close()
    elif implementation == "logging":
        entity_id = new_entity_id()
        logger = logging.getLogger("quent-bench")
        logger.handlers.clear()
        handler: logging.Handler
        if output_mode == "noop":
            handler = logging.NullHandler()
        else:
            handler = logging.FileHandler(export_path("logging.log"), mode="w")
            handler.setFormatter(
                logging.Formatter("%(asctime)s %(levelname)s entity_id=%(entity_id)s %(message)s")
            )
        logger.addHandler(handler)
        logger.propagate = False
        logger.setLevel(logging.INFO)
        bound_logger = logging.LoggerAdapter(logger, {"entity_id": entity_id})
        bound_logger.info("")
        runner.timeit(
            f"{prefix}/logging",
            'logger.info("")',
            globals={"logger": bound_logger},
        )
        handler.flush()
        handler.close()
        logger.handlers.clear()
    elif implementation == "structlog":
        output = None
        wrapped_logger: Any = DiscardStructlogLogger()
        processors = [capture_timestamp]
        if output_mode == "text":
            output = open(export_path("structlog.json"), "w", encoding="utf-8")
            wrapped_logger = structlog.PrintLogger(output)
            processors = [capture_timestamp_field, structlog.processors.JSONRenderer()]
        logger = structlog.wrap_logger(
            wrapped_logger,
            processors=processors,
            wrapper_class=structlog.stdlib.BoundLogger,
        ).bind(entity_id=new_entity_id())
        logger.info("")
        runner.timeit(
            f"{prefix}/structlog",
            'logger.info("")',
            globals={"logger": logger},
        )
        if output is not None:
            output.close()
    elif implementation == "loguru":
        loguru_logger.remove()
        sink = loguru_logger.add(
            (lambda message: None)
            if output_mode == "noop"
            else export_path("loguru.json"),
            level="INFO",
            format="{message}",
            serialize=output_mode == "text",
            enqueue=False,
            backtrace=False,
            diagnose=False,
        )
        logger = loguru_logger.bind(entity_id=new_entity_id())
        logger.info("")
        runner.timeit(
            f"{prefix}/loguru",
            'logger.info("")',
            globals={"logger": logger},
        )
        loguru_logger.remove(sink)
    elif implementation == "opentelemetry":
        output = None
        exporter: LogRecordExporter
        if output_mode == "noop":
            exporter = DiscardLogRecordExporter()
        else:
            output = open(export_path("opentelemetry.json"), "w", encoding="utf-8")
            exporter = ConsoleLogRecordExporter(out=output)
        provider = LoggerProvider(shutdown_on_exit=False)
        provider.add_log_record_processor(SimpleLogRecordProcessor(exporter))
        logger = provider.get_logger("quent-bench")
        attributes = {"entity.id": new_entity_id()}
        logger.emit(attributes=attributes)
        runner.timeit(
            f"{prefix}/opentelemetry",
            "logger.emit(attributes=attributes)",
            globals={"logger": logger, "attributes": attributes},
        )
        provider.shutdown()
        if output is not None:
            output.close()
    else:
        raise ValueError(f"unknown implementation: {implementation}")


def new_entity_id() -> str:
    """Create the entity UUID used by one isolated comparison process."""
    context = quent.Context()
    entity_id = str(context.entity_observer().handle().uuid)
    context.close()
    return entity_id


if __name__ == "__main__":
    main()
