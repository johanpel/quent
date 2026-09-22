# SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

"""Tests for benchmark result normalization."""

import json
from pathlib import Path
import sys
import tempfile
import unittest


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import report  # noqa: E402


class ReportParserTests(unittest.TestCase):
    def test_parse_rust(self) -> None:
        document = {
            "benchmarks": [
                {
                    "name": "quent",
                    "output_mode": "noop",
                    "average_ns": 17,
                    "calls": 1_000_000,
                }
            ]
        }
        with self.temporary_file(json.dumps(document)) as path:
            self.assertEqual(
                report.parse_rust(path),
                [report._row("rust", "quent", "noop", 17, 1_000_000, "rust-instant")],
            )

    def test_parse_cpp_uses_iteration_average(self) -> None:
        document = {
            "benchmarks": [
                {
                    "name": "latency/cpp/text/quent",
                    "run_name": "latency/cpp/text/quent",
                    "run_type": "iteration",
                    "real_time": 0.2,
                    "time_unit": "us",
                    "iterations": 1_000_000,
                },
            ]
        }
        with self.temporary_file(json.dumps(document)) as path:
            self.assertEqual(
                report.parse_cpp(path),
                [
                    report._row("cpp", "quent", "text", 200, 1_000_000, "google-benchmark")
                ],
            )

    def test_parse_python_uses_seconds(self) -> None:
        document = {
            "version": "1.0",
            "metadata": {"unit": "second"},
            "benchmarks": [
                {
                    "metadata": {"name": "latency/python/noop/quent"},
                    "runs": [
                        {
                            "metadata": {"loops": 1_000_000},
                            "values": [2e-6],
                        }
                    ],
                }
            ],
        }
        with self.temporary_file(json.dumps(document)) as path:
            self.assertEqual(
                report.parse_python(path),
                [
                    report._row("python", "quent", "noop", 2_000, 1_000_000, "pyperf")
                ],
            )

    def temporary_file(self, contents: str):
        return TemporaryTextFile(contents)


class TemporaryTextFile:
    def __init__(self, contents: str) -> None:
        self.contents = contents
        self.directory: tempfile.TemporaryDirectory[str] | None = None

    def __enter__(self) -> Path:
        self.directory = tempfile.TemporaryDirectory()
        path = Path(self.directory.name) / "input"
        path.write_text(self.contents)
        return path

    def __exit__(self, *_: object) -> None:
        assert self.directory is not None
        self.directory.cleanup()


if __name__ == "__main__":
    unittest.main()
