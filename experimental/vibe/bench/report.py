# SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

"""Build, run, and report instrumentation latency benchmarks."""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import csv
from datetime import UTC, datetime
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
from typing import Any, Iterable, Iterator


BENCH_DIR = Path(__file__).resolve().parent
REPO_DIR = BENCH_DIR.parents[2]
TARGET_DIR = BENCH_DIR / "target"
CPP_BUILD_DIR = TARGET_DIR / "cpp-build"
RESULTS_DIR = BENCH_DIR / "results"
CALL_COUNT = 1_000_000
DEFAULT_BASELINE_REF = "upstream/main"
OUTPUT_MODES = ("noop", "text")
RUST_IMPLEMENTATIONS = ("clock", "quent", "tracing", "log", "slog", "opentelemetry")
CPP_IMPLEMENTATIONS = (
    "clock",
    "quent",
    "spdlog",
    "quill-tsc",
    "quill-system",
    "opentelemetry",
)
PYTHON_IMPLEMENTATIONS = (
    "clock",
    "quent",
    "logging",
    "structlog",
    "loguru",
    "opentelemetry",
)
UNIT_TO_NS = {"ns": 1.0, "us": 1_000.0, "ms": 1_000_000.0, "s": 1_000_000_000.0}
BASELINE_WORKSPACE_DEPENDENCIES = (
    'convert_case = "0.11"',
    'proc-macro2 = "1"',
    'quent-constraints = { path = "crates/constraints" }',
    'quent-fsm = { path = "crates/fsm" }',
    'quent-ref-target = { path = "crates/ref-target" }',
    'quent-schema = { path = "crates/schema" }',
    'quent-yaml = { path = "crates/yaml" }',
    'quote = "1"',
)


def run(
    command: list[str], *, capture: bool = False, environment: dict[str, str] | None = None
) -> str:
    """Run a command from the repository root and return captured standard output."""
    result = subprocess.run(
        command,
        cwd=REPO_DIR,
        check=True,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        env=None if environment is None else {**os.environ, **environment},
    )
    return result.stdout if capture else ""


def build() -> None:
    """Build all benchmark executables and install the Python extension."""
    manifest = ["--manifest-path", str(BENCH_DIR / "Cargo.toml")]
    run(
        [
            "cargo",
            "build",
            *manifest,
            "-p",
            "quent-bench-cpp-bridge",
            "--release",
            "--locked",
        ]
    )

    bridge_library = TARGET_DIR / "release" / "libquent_bench_cpp_bridge.a"
    include_candidates = sorted(
        (TARGET_DIR / "release" / "build").glob(
            "quent-bench-cpp-bridge-*/out/cxxbridge/include"
        ),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not bridge_library.is_file() or not include_candidates:
        raise RuntimeError("C++ bridge artifacts were not generated")

    run(
        [
            "cmake",
            "-S",
            str(BENCH_DIR / "cpp"),
            "-B",
            str(CPP_BUILD_DIR),
            "-G",
            "Ninja",
            "-DCMAKE_BUILD_TYPE=Release",
            f"-DQUENT_BRIDGE_LIBRARY={bridge_library}",
            f"-DQUENT_BRIDGE_INCLUDE={include_candidates[0]}",
        ]
    )
    run(["cmake", "--build", str(CPP_BUILD_DIR)])
    run(
        [
            "maturin",
            "develop",
            "--release",
            "--locked",
            "--manifest-path",
            str(BENCH_DIR / "python" / "bridge" / "Cargo.toml"),
        ]
    )
    run(
        [
            "cargo",
            "bench",
            *manifest,
            "-p",
            "quent-bench-rust",
            "--bench",
            "latency",
            "--no-run",
            "--locked",
        ]
    )


def run_isolated_batches(
    raw_dir: Path,
    calls: int,
    *,
    quiet: bool = False,
    rust_implementations: Iterable[str] = RUST_IMPLEMENTATIONS,
    cpp_implementations: Iterable[str] = CPP_IMPLEMENTATIONS,
    python_implementations: Iterable[str] = PYTHON_IMPLEMENTATIONS,
) -> None:
    """Run each implementation in a fresh process and merge native result files."""
    language_dirs = {language: raw_dir / language for language in ("rust", "cpp", "python")}
    for directory in language_dirs.values():
        directory.mkdir()

    rust_paths = []
    for output_mode in OUTPUT_MODES:
        for implementation in rust_implementations:
            output = language_dirs["rust"] / f"{output_mode}-{implementation}.json"
            rust_paths.append(output)
            with tempfile.TemporaryDirectory(prefix="quent-bench-export-") as export_dir:
                run(
                    [
                        "cargo",
                        "bench",
                        "--manifest-path",
                        str(BENCH_DIR / "Cargo.toml"),
                        "-p",
                        "quent-bench-rust",
                        "--bench",
                        "latency",
                        "--locked",
                        "--",
                        "--implementation",
                        implementation,
                        "--output-mode",
                        output_mode,
                        "--calls",
                        str(calls),
                        "--output",
                        str(output),
                    ],
                    environment={"QUENT_BENCH_EXPORT_DIR": export_dir},
                )
                validate_text_output(Path(export_dir), output_mode, implementation)
    merge_benchmark_json(rust_paths, raw_dir / "rust.json")

    cpp_paths = []
    for output_mode in OUTPUT_MODES:
        for implementation in cpp_implementations:
            output = language_dirs["cpp"] / f"{output_mode}-{implementation}.json"
            cpp_paths.append(output)
            command = [
                str(CPP_BUILD_DIR / "quent_bench_cpp"),
                f"--benchmark_filter=^latency/cpp/{output_mode}/{implementation}$",
                f"--benchmark_min_time={calls}x",
                f"--benchmark_out={output}",
                "--benchmark_out_format=json",
            ]
            if quiet:
                command.append("--benchmark_dry_run=true")
            with tempfile.TemporaryDirectory(prefix="quent-bench-export-") as export_dir:
                run(command, environment={"QUENT_BENCH_EXPORT_DIR": export_dir})
                validate_text_output(Path(export_dir), output_mode, implementation)
    merge_benchmark_json(cpp_paths, raw_dir / "cpp.json")

    python_paths = []
    for output_mode in OUTPUT_MODES:
        for implementation in python_implementations:
            output = language_dirs["python"] / f"{output_mode}-{implementation}.json"
            python_paths.append(output)
            command = [
                sys.executable,
                str(BENCH_DIR / "python" / "latency.py"),
                "--processes=1",
                "--values=1",
                "--warmups=0",
                f"--loops={calls}",
                "--inherit-environ=QUENT_BENCH_IMPLEMENTATION,QUENT_BENCH_OUTPUT_MODE,QUENT_BENCH_EXPORT_DIR",
                "--output",
                str(output),
            ]
            if quiet:
                command.append("--quiet")
            with tempfile.TemporaryDirectory(prefix="quent-bench-export-") as export_dir:
                run(
                    command,
                    environment={
                        "QUENT_BENCH_IMPLEMENTATION": implementation,
                        "QUENT_BENCH_OUTPUT_MODE": output_mode,
                        "QUENT_BENCH_EXPORT_DIR": export_dir,
                    },
                )
                validate_text_output(Path(export_dir), output_mode, implementation)
    merge_pyperf(python_paths, raw_dir / "python.json")


def validate_text_output(export_dir: Path, output_mode: str, implementation: str) -> None:
    """Require non-empty output from every non-clock text benchmark."""
    if output_mode != "text" or implementation == "clock":
        return
    candidates = [path for path in export_dir.rglob("*") if path.is_file()]
    if implementation == "quent":
        candidates = [path for path in candidates if path.suffix == ".ndjson"]
    if not any(path.stat().st_size > 0 for path in candidates):
        raise RuntimeError(f"{implementation} text benchmark produced no output")


def merge_benchmark_json(inputs: list[Path], output: Path) -> None:
    """Merge single-implementation native result documents."""
    documents = [json.loads(path.read_text()) for path in inputs]
    merged = documents[0]
    merged["benchmarks"] = [
        benchmark for document in documents for benchmark in document["benchmarks"]
    ]
    output.write_text(json.dumps(merged, indent=2) + "\n")


def merge_pyperf(inputs: list[Path], output: Path) -> None:
    """Merge isolated pyperf suites while preserving benchmark metadata."""
    import pyperf

    merged = pyperf.BenchmarkSuite.load(str(inputs[0]))
    for path in inputs[1:]:
        for benchmark in pyperf.BenchmarkSuite.load(str(path)):
            merged.add_benchmark(benchmark)
    merged.dump(str(output))


def parse_rust(path: Path) -> list[dict[str, Any]]:
    """Parse Rust batch measurements into normalized rows."""
    document = json.loads(path.read_text())
    rows = [
        _row(
            "rust",
            item["name"],
            item["output_mode"],
            item["average_ns"],
            item["calls"],
            "rust-instant",
        )
        for item in document["benchmarks"]
    ]
    if not rows:
        raise ValueError(f"no Rust benchmark rows found in {path}")
    return rows


def parse_cpp(path: Path) -> list[dict[str, Any]]:
    """Parse Google Benchmark JSON into normalized rows."""
    document = json.loads(path.read_text())
    rows = []
    for item in document.get("benchmarks", []):
        if item.get("run_type") == "aggregate":
            continue
        name = item.get("run_name", item["name"])
        prefix = "latency/cpp/"
        if not name.startswith(prefix):
            continue
        scale = UNIT_TO_NS[item["time_unit"]]
        output_mode, implementation = name.removeprefix(prefix).split("/", maxsplit=1)
        rows.append(
            _row(
                "cpp",
                implementation,
                output_mode,
                item["real_time"] * scale,
                item["iterations"],
                "google-benchmark",
            )
        )
    if not rows:
        raise ValueError(f"no C++ benchmark rows found in {path}")
    return rows


def parse_python(path: Path) -> list[dict[str, Any]]:
    """Parse pyperf JSON into normalized rows."""
    import pyperf

    suite = pyperf.BenchmarkSuite.load(str(path))
    rows = []
    for benchmark in suite:
        prefix = "latency/python/"
        if not benchmark.get_name().startswith(prefix):
            continue
        if benchmark.get_nvalue() != 1:
            raise ValueError(f"expected one Python batch in {path}")
        rows.append(
            _row(
                "python",
                benchmark.get_name().removeprefix(prefix).split("/", maxsplit=1)[1],
                benchmark.get_name().removeprefix(prefix).split("/", maxsplit=1)[0],
                benchmark.mean() * UNIT_TO_NS["s"],
                benchmark.get_total_loops(),
                "pyperf",
            )
        )
    if not rows:
        raise ValueError(f"no Python benchmark rows found in {path}")
    return rows


def parse_native_results(raw_dir: Path) -> list[dict[str, Any]]:
    """Parse one complete or Quent-only set of native benchmark results."""
    return [
        *parse_rust(raw_dir / "rust.json"),
        *parse_cpp(raw_dir / "cpp.json"),
        *parse_python(raw_dir / "python.json"),
    ]


def label_quent_revisions(
    current_rows: Iterable[dict[str, Any]], baseline_rows: Iterable[dict[str, Any]]
) -> list[dict[str, Any]]:
    """Place the baseline and current Quent results next to each other."""
    baseline = {
        (row["language"], row["output_mode"]): row
        for row in baseline_rows
        if row["implementation"] == "quent"
    }
    rows = []
    for current in current_rows:
        if current["implementation"] != "quent":
            rows.append(current)
            continue
        key = (current["language"], current["output_mode"])
        if key not in baseline:
            raise ValueError(f"missing baseline Quent result for {key[0]}/{key[1]}")
        rows.append({**baseline[key], "implementation": "quent-main"})
        rows.append({**current, "implementation": "quent-optimized"})
    return rows


def normalize(raw_dir: Path) -> dict[str, Any]:
    """Normalize the three native output formats into one versioned document."""
    rows = parse_native_results(raw_dir)
    baseline_dir = raw_dir / "main"
    baseline_revision = None
    if baseline_dir.is_dir():
        rows = label_quent_revisions(rows, parse_native_results(baseline_dir))
        baseline_revision = (baseline_dir / "revision.txt").read_text().strip()
    return {
        "schema_version": 4,
        "metadata": {
            "generated_at": datetime.now(UTC).isoformat(),
            "git_revision": _git_revision(),
            "baseline_revision": baseline_revision,
            "platform": platform.platform(),
            "machine": platform.machine(),
            "python": platform.python_version(),
        },
        "benchmarks": rows,
    }


def write_report(result_dir: Path, *, expected_call_count: int | None = None) -> None:
    """Write normalized JSON, CSV, and a comparison chart for one result directory."""
    document = normalize(result_dir / "raw")
    if expected_call_count is not None:
        mismatches = [
            f"{row['language']}/{row['implementation']}={row['calls']}"
            for row in document["benchmarks"]
            if row["calls"] != expected_call_count
        ]
        if mismatches:
            raise RuntimeError(
                f"expected {expected_call_count} calls per benchmark; got "
                + ", ".join(mismatches)
            )
    (result_dir / "results.json").write_text(json.dumps(document, indent=2) + "\n")
    with (result_dir / "results.csv").open("w", newline="") as output:
        fields = ("language", "output_mode", "implementation", "average_ns", "calls", "harness")
        writer = csv.DictWriter(output, fieldnames=fields, extrasaction="ignore")
        writer.writeheader()
        writer.writerows(document["benchmarks"])
    _plot(document["benchmarks"], result_dir / "latency.pdf", document["metadata"])
    print(f"Wrote {result_dir}")


def run_benchmarks(
    result_dir: Path | None = None, *, baseline_ref: str = DEFAULT_BASELINE_REF
) -> Path:
    """Build and execute the complete benchmark matrix."""
    result_dir = result_dir or _new_result_dir()
    raw_dir = result_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=False)
    try:
        capture_baseline(raw_dir / "main", CALL_COUNT, baseline_ref)
    finally:
        # The baseline build installs its Python extension into the active Pixi
        # environment. Rebuilding here restores the current checkout even when
        # baseline execution fails.
        build()
    run_isolated_batches(raw_dir, CALL_COUNT)
    write_report(result_dir, expected_call_count=CALL_COUNT)
    return result_dir


def capture_quent(raw_dir: Path, calls: int) -> None:
    """Build this checkout and capture only its Quent measurements."""
    build()
    raw_dir.mkdir(parents=True, exist_ok=False)
    run_isolated_batches(
        raw_dir,
        calls,
        rust_implementations=("quent",),
        cpp_implementations=("quent",),
        python_implementations=("quent",),
    )


def capture_baseline(raw_dir: Path, calls: int, baseline_ref: str) -> None:
    """Capture Quent measurements from an immutable archived revision."""
    revision = _resolve_revision(baseline_ref)
    with archived_baseline(revision) as baseline_repo:
        baseline_report = baseline_repo / "experimental" / "vibe" / "bench" / "report.py"
        run(
            [
                sys.executable,
                str(baseline_report),
                "capture-quent",
                "--results",
                str(raw_dir),
                "--calls",
                str(calls),
            ]
        )
    (raw_dir / "revision.txt").write_text(revision + "\n")


@contextmanager
def archived_baseline(revision: str) -> Iterator[Path]:
    """Yield a temporary source archive with the current benchmark harness."""
    with tempfile.TemporaryDirectory(prefix="quent-bench-main-") as temporary:
        repository = Path(temporary)
        process = subprocess.Popen(
            ["git", "archive", "--format=tar", revision],
            cwd=REPO_DIR,
            stdout=subprocess.PIPE,
        )
        assert process.stdout is not None
        try:
            with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
                archive.extractall(repository, filter="data")
        finally:
            process.stdout.close()
        if process.wait() != 0:
            raise subprocess.CalledProcessError(process.returncode, process.args)

        destination = repository / "experimental" / "vibe" / "bench"
        if destination.exists():
            shutil.rmtree(destination)
        shutil.copytree(
            BENCH_DIR,
            destination,
            ignore=shutil.ignore_patterns(".pixi", "target", "results", "__pycache__", "*.pyc"),
        )
        _add_baseline_workspace_dependencies(repository / "Cargo.toml")
        subprocess.run(
            [
                "cargo",
                "metadata",
                "--manifest-path",
                str(destination / "Cargo.toml"),
                "--offline",
                "--format-version",
                "1",
            ],
            cwd=repository,
            check=True,
            stdout=subprocess.DEVNULL,
        )
        yield repository


def _add_baseline_workspace_dependencies(manifest: Path) -> None:
    """Add workspace declarations needed only to compile the copied harness."""
    contents = manifest.read_text()
    marker = "[workspace.dependencies]\n"
    start = contents.index(marker) + len(marker)
    end = contents.find("\n[", start)
    end = len(contents) if end == -1 else end
    section = contents[start:end]
    missing = [
        dependency
        for dependency in BASELINE_WORKSPACE_DEPENDENCIES
        if f"{dependency.split(' =', maxsplit=1)[0]} =" not in section
    ]
    if missing:
        contents = contents[:start] + "\n".join(missing) + "\n" + contents[start:]
        manifest.write_text(contents)


def smoke() -> None:
    """Build and minimally execute every adapter and report parser."""
    build()
    run(
        [
            "cargo",
            "test",
            "--manifest-path",
            str(BENCH_DIR / "Cargo.toml"),
            "--workspace",
            "--all-targets",
            "--locked",
        ]
    )
    run([sys.executable, "-m", "unittest", "discover", "-s", str(BENCH_DIR / "tests"), "-v"])
    with tempfile.TemporaryDirectory(prefix="quent-bench-") as temporary:
        result_dir = Path(temporary)
        raw_dir = result_dir / "raw"
        raw_dir.mkdir()
        run_isolated_batches(raw_dir, 1, quiet=True)
        write_report(result_dir, expected_call_count=1)


def _row(
    language: str,
    implementation: str,
    output_mode: str,
    average_ns: float,
    calls: int,
    harness: str,
) -> dict[str, Any]:
    return {
        "language": language,
        "output_mode": output_mode,
        "implementation": implementation,
        "average_ns": round(average_ns, 3),
        "calls": calls,
        "harness": harness,
    }


def _git_revision() -> str:
    return run(["git", "rev-parse", "HEAD"], capture=True).strip()


def _resolve_revision(reference: str) -> str:
    return run(
        ["git", "rev-parse", "--verify", f"{reference}^{{commit}}"], capture=True
    ).strip()


def _new_result_dir() -> Path:
    stamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    revision = _git_revision()[:12]
    return RESULTS_DIR / f"{stamp}-{revision}"


def _latest_result_dir() -> Path:
    candidates = [
        path
        for path in RESULTS_DIR.glob("*")
        if (path / "raw" / "rust.json").is_file()
        and (path / "raw" / "cpp.json").is_file()
        and (path / "raw" / "python.json").is_file()
    ]
    if not candidates:
        raise RuntimeError("no benchmark results found; run `pixi run bench` first")
    return max(candidates, key=lambda path: path.stat().st_mtime)


def _plot(
    rows: Iterable[dict[str, Any]], output: Path, metadata: dict[str, Any] | None = None
) -> None:
    matplotlib_cache = TARGET_DIR / "matplotlib"
    matplotlib_cache.mkdir(parents=True, exist_ok=True)
    os.environ.setdefault("MPLCONFIGDIR", str(matplotlib_cache))
    import matplotlib.pyplot as plt
    from matplotlib.patches import Patch
    from matplotlib.ticker import MaxNLocator, StrMethodFormatter

    rows = list(rows)
    grouped = {
        (output_mode, language): [
            row
            for row in rows
            if row["language"] == language and row["output_mode"] == output_mode
        ]
        for output_mode in OUTPUT_MODES
        for language in ("cpp", "rust", "python")
    }
    call_counts = sorted({row["calls"] for values in grouped.values() for row in values})
    call_summary = "/".join(f"{count:,}" for count in call_counts)
    figure, axes = plt.subplots(2, 3, figsize=(18, 10))
    for axis, ((output_mode, language), values) in zip(axes.flat, grouped.items()):
        colors = [
            (
                "#0f766e"
                if row["implementation"] in ("quent", "quent-optimized")
                else "#d97706"
                if row["implementation"] == "quent-main"
                else "#94a3b8"
                if row["implementation"] == "clock"
                else "#2563eb"
            )
            for row in values
        ]
        bars = axis.barh(
            [f"{row['implementation']} (N={row['calls']:,})" for row in values],
            [row["average_ns"] for row in values],
            color=colors,
            alpha=0.8,
            height=0.55,
        )
        axis.set_title(f"{language.upper()} — {output_mode.upper()}")
        axis.set_xlabel("average latency (ns)")
        axis.xaxis.set_major_locator(MaxNLocator(nbins=5))
        axis.xaxis.set_major_formatter(StrMethodFormatter("{x:,.0f}"))
        axis.grid(axis="x", color="#d1d5db", linestyle="--", linewidth=0.8)
        axis.set_axisbelow(True)
        axis.set_xlim(0, max(row["average_ns"] for row in values) * 1.25)
        axis.bar_label(
            bars,
            labels=[_format_ns(row["average_ns"]) for row in values],
            padding=4,
            fontsize=8,
        )
        axis.invert_yaxis()
    figure.suptitle("Instrumentation call latency: no-op and text-output suites")
    figure.legend(
        handles=[
            Patch(facecolor="#94a3b8", alpha=0.8, label="Clock: timestamp-call baseline"),
            Patch(facecolor="#d97706", alpha=0.8, label="Quent main entity.tick()"),
            Patch(facecolor="#0f766e", alpha=0.8, label="Quent optimized entity.tick()"),
            Patch(facecolor="#2563eb", alpha=0.8, label="Comparison framework"),
        ],
        loc="lower center",
        bbox_to_anchor=(0.5, 0.205),
        ncol=4,
        frameon=False,
        title="Series",
    )
    baseline_revision = (metadata or {}).get("baseline_revision")
    revision_summary = (
        f" Quent main is built from {baseline_revision[:12]}; Quent optimized is built from "
        f"{(metadata or {}).get('git_revision', 'the active checkout')[:12]}."
        if baseline_revision
        else ""
    )
    explanation = (
        "MEASUREMENT LEGEND\n"
        f"Timed operation: one batch of {call_summary} emissions through a pre-created entity or "
        "logger. The bar is total batch time divided by the call count. Every record captures a "
        "timestamp and carries a pre-created entity UUID; setup, UUID creation/string conversion, "
        "preflight calls, final draining, and shutdown are outside timing. Each framework/mode pair "
        "runs in a fresh process with a fresh temporary output directory."
        f"{revision_summary}\n"
        "Timestamp capture: Quent main calls its system clock inside tick(); Quent optimized captures "
        "a raw counter there and converts it on the forwarder. Rust tracing/log/slog capture "
        "SystemTime on their dispatch paths. spdlog, Python logging, and Loguru use native "
        "record timestamps; structlog calls time_ns(). Quill-TSC captures a counter on the caller and "
        "converts it on the backend; Quill-system calls system_clock. The OpenTelemetry SDK captures an "
        "observed timestamp during emit. Clock measures only the corresponding timestamp call.\n"
        "No-op suite: every framework dispatches to a no-op/discard sink. Text suite: Quent writes "
        "NDJSON; Quill, structlog, and Loguru write JSON; OpenTelemetry uses ostream text in C++, "
        "console JSON in Python, and text in Rust; spdlog, tracing, log, slog, and Python "
        "logging write native or equivalent text. Quent and Quill output asynchronously; their "
        "serialization and I/O run concurrently and are drained after timing.\n"
        "Captured data: Quent records its native 128-bit entity ID. Comparison records use the "
        "preformatted UUID as structured context, a record field, or a deferred formatting argument. "
        "There are no user event attributes. Clock is a timestamp-only baseline in both suites.\n"
        "Average data: each implementation runs once. Rust uses one Instant interval, C++ uses one "
        "Google Benchmark iteration batch, and Python uses one pyperf value with a fixed loop count. "
        "The chart shows batch averages, not individual-call distributions or error bars."
    )
    figure.text(0.03, 0.155, explanation, ha="left", va="top", fontsize=8, wrap=True)
    figure.subplots_adjust(
        left=0.13, right=0.98, top=0.92, bottom=0.34, wspace=0.85, hspace=0.7
    )
    figure.savefig(output, bbox_inches="tight", pad_inches=0.15)
    plt.close(figure)


def _format_ns(value: float) -> str:
    if value >= 1_000_000:
        return f"{value / 1_000_000:.2f} ms"
    if value >= 1_000:
        return f"{value / 1_000:.2f} µs"
    return f"{value:.1f} ns"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command", choices=("build", "run", "report", "smoke", "capture-quent")
    )
    parser.add_argument("--results", type=Path, help="result directory to create or report")
    parser.add_argument(
        "--baseline-ref",
        default=DEFAULT_BASELINE_REF,
        help=f"Quent baseline revision for a full run (default: {DEFAULT_BASELINE_REF})",
    )
    parser.add_argument("--calls", type=int, help=argparse.SUPPRESS)
    arguments = parser.parse_args()
    if arguments.command == "build":
        build()
    elif arguments.command == "run":
        run_benchmarks(arguments.results, baseline_ref=arguments.baseline_ref)
    elif arguments.command == "report":
        write_report(arguments.results or _latest_result_dir())
    elif arguments.command == "smoke":
        smoke()
    else:
        if arguments.results is None or arguments.calls is None:
            parser.error("capture-quent requires --results and --calls")
        capture_quent(arguments.results, arguments.calls)


if __name__ == "__main__":
    main()
