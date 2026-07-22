#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
"""Plot quent-bench throughput and latency results.

    BENCH_CSV=bench.csv pixi run cargo run -p quent-bench --release
    pixi run uv run --with matplotlib bench/plot.py bench.csv

For each (attributes, threads) cell there are two aligned panels sharing the same
row order (ranked by achieved throughput): the LEFT panel is real throughput
(confirmed flushed/delivered), the RIGHT is the offered API-call rate. Rates are
operations/second (1 op = one log or one span).

Latency output is written beside the throughput figure with `-latency` appended
to the filename.
"""
import csv
import os
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.ticker as mticker

# Colored by library family, so a variant reads the same in both panels.
FAMILY = {
    "quent": "#0072B2",  # blue
    "otel": "#D55E00",  # vermillion
    "tracing": "#009E73",  # green
}


LOSS = "#BBBBBB"  # dropped fraction, overlaid on the offered bar


def family(v):
    return v.split("-", 1)[0]


def fnum(s):
    return float(s) if s else 0.0


def optional_num(s):
    return float(s) if s else None


def loss_frac(r):
    # Fraction of offered ops that never reached the sink. noop (no counted
    # sink, no delivery) counts as fully discarded.
    if r["loss_pct"]:
        return float(r["loss_pct"]) / 100
    return 0.0 if r["tput_ops_s"] else 1.0


def si(v):
    if v >= 999_999_950:
        return f"{v / 1e9:.2f}G"
    if v >= 999_950:
        return f"{v / 1e6:.2f}M"
    if v >= 999.95:
        return f"{v / 1e3:.1f}k"
    return f"{v:.0f}"


def duration_ns(v):
    if v >= 1e9:
        return f"{v / 1e9:.2f}s"
    if v >= 1e6:
        return f"{v / 1e6:.2f}ms"
    if v >= 1e3:
        return f"{v / 1e3:.1f}us"
    return f"{v:.0f}ns"


def byte_rate(v):
    if v >= 999_950_000:
        return f"{v / 1e9:.2f}GB/s"
    if v >= 999_950:
        return f"{v / 1e6:.1f}MB/s"
    if v >= 999.95:
        return f"{v / 1e3:.1f}kB/s"
    return f"{v:.0f}B/s"


def panel(ax, order, values, labels, colors, title, losses=None, formatter=si):
    ys = list(range(len(order)))
    bars = ax.barh(ys, values, height=0.92, color=colors)
    if losses is not None:
        for y, (v, lf) in enumerate(zip(values, losses)):
            if lf > 0:
                ax.barh(y, v * lf, left=v * (1 - lf), height=0.92, color=LOSS)
    ax.bar_label(bars, labels=[formatter(v) for v in values], padding=2, fontsize=7)
    ax.set_yticks(ys)
    ax.set_yticklabels(labels, fontsize=8)
    ax.set_title(title, fontsize=9)
    ax.xaxis.set_major_formatter(mticker.FuncFormatter(lambda x, _: formatter(x)))
    ax.tick_params(axis="x", labelsize=7)
    ax.margins(x=0.20)  # headroom for the data labels


def latency_panel(ax, order, values, colors, title, show_labels=True):
    ys = list(range(len(order)))
    points = [(y, max(value, 1.0), colors[y])
              for y, value in enumerate(values) if value is not None]
    if points:
        point_y, point_x, point_colors = zip(*points)
        ax.scatter(point_x, point_y, c=point_colors, s=18, zorder=3)
        for y, value, _ in points:
            ax.annotate(duration_ns(value), (value, y), xytext=(4, 0),
                        textcoords="offset points", va="center", fontsize=6)
    ax.set_xscale("log")
    ax.set_yticks(ys)
    ax.set_yticklabels(order if show_labels else [], fontsize=8)
    ax.set_title(title, fontsize=9)
    ax.xaxis.set_major_formatter(mticker.FuncFormatter(lambda x, _: duration_ns(x)))
    ax.tick_params(axis="x", labelsize=7)
    ax.grid(axis="x", which="both", alpha=0.2)
    ax.margins(x=0.25)


def plot_latency(rows, attrs, threads, n_variants, out):
    nrows, ncols = len(attrs), 2 * len(threads)
    fig, axes = plt.subplots(
        nrows, ncols,
        figsize=(3.0 * ncols, 0.17 * n_variants * nrows + 1.2 * nrows),
        squeeze=False, layout="constrained",
    )
    for i, a in enumerate(attrs):
        for j, t in enumerate(threads):
            cell = {
                r["variant"]: (
                    optional_num(r["call_p99_ns"]),
                    None if not r["drain_ms"] else float(r["drain_ms"]) * 1e6,
                )
                for r in rows
                if int(r["attrs"]) == a and int(r["threads"]) == t
            }
            order = sorted(cell, key=lambda v: cell[v][0] or 0.0)
            colors = [FAMILY[family(v)] for v in order]
            latency_panel(
                axes[i][2 * j], order, [cell[v][0] for v in order], colors,
                f"{a} attrs · {t} thr — caller p99",
            )
            latency_panel(
                axes[i][2 * j + 1], order, [cell[v][1] for v in order], colors,
                "pipeline drain", show_labels=False,
            )

    fig.legend(
        handles=[plt.Rectangle((0, 0), 1, 1, color=c) for c in FAMILY.values()],
        labels=["quent", "OpenTelemetry", "tracing"],
        loc="outside lower center", ncol=3,
    )
    fig.suptitle(
        "Latency under saturated caller load\n"
        "caller p99 = sampled API operation    ·    "
        "drain = emission end to drained/stalled sink    ·    noop drain omitted",
        fontsize=9,
    )
    fig.savefig(out, dpi=130)
    print("wrote", out)


def plot_write_throughput(rows, attrs, threads, out):
    file_variants = {r["variant"] for r in rows if r.get("write_bytes_s")}
    if not file_variants:
        return
    fig, axes = plt.subplots(
        len(attrs), len(threads),
        figsize=(4.0 * len(threads), 0.20 * len(file_variants) * len(attrs) + 1.3 * len(attrs)),
        squeeze=False, layout="constrained",
    )
    for i, a in enumerate(attrs):
        for j, t in enumerate(threads):
            cell = {
                r["variant"]: float(r["write_bytes_s"])
                for r in rows
                if int(r["attrs"]) == a and int(r["threads"]) == t
                and r.get("write_bytes_s")
            }
            order = sorted(cell, key=cell.get)
            colors = [FAMILY[family(v)] for v in order]
            panel(
                axes[i][j], order, [cell[v] for v in order], order, colors,
                f"{a} attrs · {t} thr — logical write throughput",
                formatter=byte_rate,
            )
    fig.legend(
        handles=[plt.Rectangle((0, 0), 1, 1, color=c) for c in FAMILY.values()],
        labels=["quent", "OpenTelemetry", "tracing"],
        loc="outside lower center", ncol=3,
    )
    fig.suptitle(
        "Logical filesystem write throughput\n"
        "bytes in completed files ÷ full drained delivery time · flushed, not fsynced · page cache may hide device limits",
        fontsize=9,
    )
    fig.savefig(out, dpi=130)
    print("wrote", out)


def main():
    csv_path = sys.argv[1] if len(sys.argv) > 1 else "bench.csv"
    out = sys.argv[2] if len(sys.argv) > 2 else "plots/bench.png"
    os.makedirs(os.path.dirname(out) or ".", exist_ok=True)

    rows = list(csv.DictReader(open(csv_path)))
    if not rows:
        sys.exit(f"no rows in {csv_path}")

    attrs = sorted({int(r["attrs"]) for r in rows})
    threads = sorted({int(r["threads"]) for r in rows})
    n_variants = len({r["variant"] for r in rows})

    nrows, ncols = len(attrs), 2 * len(threads)
    fig, axes = plt.subplots(
        nrows, ncols,
        figsize=(3.0 * ncols, 0.17 * n_variants * nrows + 1.2 * nrows),
        squeeze=False, layout="constrained",
    )
    for i, a in enumerate(attrs):
        for j, t in enumerate(threads):
            cell = {
                r["variant"]: (
                    fnum(r["offered_ops_s"]),
                    fnum(r["tput_ops_s"]),
                    loss_frac(r),
                )
                for r in rows
                if int(r["attrs"]) == a and int(r["threads"]) == t
            }
            order = sorted(cell, key=lambda v: cell[v][1])  # by real throughput
            colors = [FAMILY[family(v)] for v in order]
            panel(axes[i][2 * j], order, [cell[v][1] for v in order], order,
                  colors, f"{a} attrs · {t} thr — throughput")
            axo = axes[i][2 * j + 1]
            panel(axo, order, [cell[v][0] for v in order], order, colors, "offered",
                  losses=[cell[v][2] for v in order])
            axo.set_yticklabels([])  # share the left panel's labels

    fig.legend(
        handles=[plt.Rectangle((0, 0), 1, 1, color=c)
                 for c in list(FAMILY.values()) + [LOSS]],
        labels=["quent", "OpenTelemetry", "tracing", "dropped (loss)"],
        loc="outside lower center", ncol=4,
    )
    fig.suptitle(
        "quent vs OpenTelemetry vs tracing — each cell spams a fixed number of ops as fast as possible\n"
        "throughput = ops ÷ full time to deliver everything    ·    offered = ops ÷ emit time\n"
        "offered > throughput = caller outruns the pipeline: quent/tracing remain lossless; gray = dropped (OTel)",
        fontsize=9,
    )
    fig.savefig(out, dpi=130)
    print("wrote", out)

    if "call_p99_ns" in rows[0]:
        stem, extension = os.path.splitext(out)
        latency_out = f"{stem}-latency{extension or '.png'}"
        plot_latency(rows, attrs, threads, n_variants, latency_out)
    if "write_bytes_s" in rows[0]:
        stem, extension = os.path.splitext(out)
        write_out = f"{stem}-write{extension or '.png'}"
        plot_write_throughput(rows, attrs, threads, write_out)


if __name__ == "__main__":
    main()
