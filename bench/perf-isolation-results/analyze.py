import csv
import json
import math
from pathlib import Path

ROOT = Path("/tmp/quent-perf-isolation/results")


def load(name):
    with (ROOT / f"{name}.csv").open(newline="") as f:
        rows = list(csv.DictReader(f))
    result = {}
    for row in rows:
        key = (row["variant"], int(row["attrs"]), int(row["threads"]))
        result[key] = {
            k: (float(v) if v else None)
            for k, v in row.items()
            if k not in {"variant", "attrs", "threads"}
        }
    return result


start = load("baseline-start")
end = load("baseline-end")


def mean(a, b):
    if a is None or b is None:
        return a if b is None else b
    return (a + b) / 2


baseline = {
    key: {metric: mean(values[metric], end[key][metric]) for metric in values}
    for key, values in start.items()
}


def gmean(values):
    values = [v for v in values if v is not None and v > 0]
    return math.exp(sum(math.log(v) for v in values) / len(values)) if values else None


def compare_external(name, variants, metrics=("tput_ops_s", "offered_ops_s", "call_p99_ns", "drain_ms")):
    candidate = load(name)
    pairs = []
    for key, values in candidate.items():
        if key[0] not in variants or key not in baseline:
            continue
        ratios = {}
        for metric in metrics:
            a = values.get(metric)
            b = baseline[key].get(metric)
            ratios[metric] = a / b if a is not None and b not in (None, 0) else None
        pairs.append((key, ratios))
    summary = {metric: gmean([ratios[metric] for _, ratios in pairs]) for metric in metrics}
    return pairs, summary


def compare_internal(name, optimized_backend, control_backend, modes=("static", "dynamic")):
    rows = load(name)
    pairs = []
    for mode in modes:
        for signal in ("log", "span"):
            optimized_variant = f"quent-{mode}-{signal}/{optimized_backend}"
            control_variant = f"quent-{mode}-{signal}/{control_backend}"
            for attrs in (8, 64):
                for threads in (1, 16):
                    ok = (optimized_variant, attrs, threads)
                    ck = (control_variant, attrs, threads)
                    if ok not in rows or ck not in rows:
                        continue
                    ratios = {}
                    for metric in ("tput_ops_s", "offered_ops_s", "call_p99_ns", "drain_ms", "write_bytes_s", "bytes_written"):
                        a = rows[ok].get(metric)
                        b = rows[ck].get(metric)
                        ratios[metric] = a / b if a is not None and b not in (None, 0) else None
                    pairs.append((ok, ratios))
    summary = {
        metric: gmean([ratios[metric] for _, ratios in pairs])
        for metric in ("tput_ops_s", "offered_ops_s", "call_p99_ns", "drain_ms", "write_bytes_s", "bytes_written")
    }
    return pairs, summary


postcard4 = {
    f"quent-{mode}-{signal}/postcard"
    for mode in ("static", "dynamic")
    for signal in ("log", "span")
}
noop4 = {
    f"quent-{mode}-{signal}/noop"
    for mode in ("static", "dynamic")
    for signal in ("log", "span")
}
grpc4 = {
    f"quent-{mode}-{signal}/grpc"
    for mode in ("static", "dynamic")
    for signal in ("log", "span")
}
filesystem12 = {
    f"quent-{mode}-{signal}/{backend}"
    for mode in ("static", "dynamic")
    for signal in ("log", "span")
    for backend in ("ndjson", "msgpack", "postcard")
}

external_specs = {
    "candidate-02": postcard4,
    "candidate-04": postcard4,
    "candidate-05": grpc4,
    "candidate-06": postcard4,
    "candidate-07": noop4,
    "candidate-08-postcard": {"quent-dynamic-log/postcard", "quent-dynamic-span/postcard"},
    "candidate-08-noop": {"quent-dynamic-log/noop", "quent-dynamic-span/noop"},
    "candidate-09": filesystem12,
    "candidate-11": filesystem12,
    "candidate-13": postcard4,
    "candidate-14": postcard4,
}

results = {"baseline_drift": {}, "external": {}, "internal": {}}
for metric in ("tput_ops_s", "offered_ops_s", "call_p99_ns", "drain_ms"):
    ratios = []
    for key in start:
        a = end[key].get(metric)
        b = start[key].get(metric)
        if a is not None and b not in (None, 0):
            ratios.append(a / b)
    results["baseline_drift"][metric] = gmean(ratios)

for label, variants in external_specs.items():
    source = label.split("-postcard")[0].split("-noop")[0]
    pairs, summary = compare_external(source, variants)
    results["external"][label] = {"summary": summary, "pairs": pairs}

for label, optimized, control, modes in (
    ("candidate-01", "postcard-producer", "postcard", ("static", "dynamic")),
    ("candidate-03", "raw", "postcard", ("static", "dynamic")),
    ("candidate-10", "bincode-native", "bincode-serde", ("static",)),
    ("candidate-12", "bitcode", "postcard", ("static", "dynamic")),
):
    pairs, summary = compare_internal(label, optimized, control, modes)
    results["internal"][label] = {"summary": summary, "pairs": pairs}

# Validate all schemas, row counts, and lossless file/collector results.
validation = {}
for csv_path in sorted(ROOT.glob("*.csv")):
    rows = load(csv_path.stem)
    json_path = csv_path.with_suffix(".json")
    payload = json.loads(json_path.read_text())
    bad_loss = [key for key, row in rows.items() if row["loss_pct"] not in (None, 0.0)]
    validation[csv_path.stem] = {
        "csv_rows": len(rows),
        "json_rows": len(payload["results"]),
        "bad_loss": bad_loss,
    }
results["validation"] = validation

(ROOT / "analysis.json").write_text(json.dumps(results, indent=2))

print("baseline drift (end/start geometric mean):")
for metric, ratio in results["baseline_drift"].items():
    print(f"  {metric:16s} {ratio:8.3f}x")

print("\nexternal candidate/baseline ratios (geometric mean):")
for label, data in results["external"].items():
    s = data["summary"]
    print(
        f"  {label:24s} tput={s['tput_ops_s'] or float('nan'):6.3f}x "
        f"offered={s['offered_ops_s'] or float('nan'):6.3f}x "
        f"p99={s['call_p99_ns'] or float('nan'):6.3f}x "
        f"drain={s['drain_ms'] or float('nan'):6.3f}x"
    )

print("\ninternal optimized/control ratios (geometric mean):")
for label, data in results["internal"].items():
    s = data["summary"]
    print(
        f"  {label:24s} tput={s['tput_ops_s'] or float('nan'):6.3f}x "
        f"offered={s['offered_ops_s'] or float('nan'):6.3f}x "
        f"p99={s['call_p99_ns'] or float('nan'):6.3f}x "
        f"drain={s['drain_ms'] or float('nan'):6.3f}x "
        f"size={s['bytes_written'] or float('nan'):6.3f}x"
    )

assert all(v["csv_rows"] == v["json_rows"] for v in validation.values())
assert all(not v["bad_loss"] for v in validation.values())
print(f"\nvalidated {len(validation)} datasets: CSV/JSON row counts match; every counted row lossless")
