#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
"""Plot quent-bench results as one figure.

    BENCH_CSV=bench.csv cargo run -p quent-bench --release
    uv run --with matplotlib bench/plot.py bench.csv        # -> plots/bench.png

For each (attributes, threads) cell there are two aligned panels sharing the same
row order (ranked by achieved throughput): the LEFT panel is real throughput
(confirmed flushed/delivered), the RIGHT is the offered API-call rate. Rates are
operations/second (1 op = one log or one span).
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


def panel(ax, order, values, labels, colors, title, losses=None):
    ys = list(range(len(order)))
    bars = ax.barh(ys, values, height=0.92, color=colors)
    if losses is not None:
        for y, (v, lf) in enumerate(zip(values, losses)):
            if lf > 0:
                ax.barh(y, v * lf, left=v * (1 - lf), height=0.92, color=LOSS)
    ax.bar_label(bars, labels=[si(v) for v in values], padding=2, fontsize=7)
    ax.set_yticks(ys)
    ax.set_yticklabels(labels, fontsize=8)
    ax.set_title(title, fontsize=9)
    ax.xaxis.set_major_formatter(mticker.FuncFormatter(lambda x, _: si(x)))
    ax.tick_params(axis="x", labelsize=7)
    ax.margins(x=0.20)  # headroom for the data labels


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
        "offered > throughput = caller outruns the pipeline: quent's unbounded queue just drains (0% loss); gray = dropped (OTel/tracing)",
        fontsize=9,
    )
    fig.savefig(out, dpi=130)
    print("wrote", out)


if __name__ == "__main__":
    main()
