#!/usr/bin/env bash
set -euo pipefail

ROOT=/tmp/quent-perf-isolation
RESULTS="$ROOT/results"
MASTER="$RESULTS/run.log"
mkdir -p "$RESULTS"
exec > >(tee -a "$MASTER") 2>&1

OPS=50000
REPS=3
ATTRS=8,64
THREADS=1,16

postcard4='quent-static-log/postcard,quent-dynamic-log/postcard,quent-static-span/postcard,quent-dynamic-span/postcard'
noop4='quent-static-log/noop,quent-dynamic-log/noop,quent-static-span/noop,quent-dynamic-span/noop'
grpc4='quent-static-log/grpc,quent-dynamic-log/grpc,quent-static-span/grpc,quent-dynamic-span/grpc'
filesystem12='quent-static-log/ndjson,quent-dynamic-log/ndjson,quent-static-span/ndjson,quent-dynamic-span/ndjson,quent-static-log/msgpack,quent-dynamic-log/msgpack,quent-static-span/msgpack,quent-dynamic-span/msgpack,quent-static-log/postcard,quent-dynamic-log/postcard,quent-static-span/postcard,quent-dynamic-span/postcard'
baseline_union="$filesystem12,$grpc4,$noop4"

package_temp() {
    local zone
    for zone in /sys/class/thermal/thermal_zone*; do
        if test -r "$zone/type" && test "$(cat "$zone/type")" = x86_pkg_temp; then
            cat "$zone/temp"
            return
        fi
    done
    printf '0\n'
}

dirty_kib() {
    awk '/^(Dirty|Writeback):/ { total += $2 } END { print total + 0 }' /proc/meminfo
}

wait_idle() {
    local temp dirty
    while :; do
        temp=$(package_temp)
        dirty=$(dirty_kib)
        if ! pgrep -x cargo >/dev/null \
            && ! pgrep -x rustc >/dev/null \
            && ! pgrep -x quent-bench >/dev/null \
            && test "$temp" -le 50000 \
            && test "$dirty" -le 65536; then
            break
        fi
        printf 'waiting for idle: package_temp_mC=%s dirty_kib=%s\n' "$temp" "$dirty"
        sleep 5
    done
}

snapshot() {
    printf 'time=%s package_temp_mC=%s dirty_kib=%s load=' \
        "$(date --iso-8601=seconds)" "$(package_temp)" "$(dirty_kib)"
    awk '{ print $1, $2, $3 }' /proc/loadavg
}

run_case() {
    local label=$1
    local worktree=$2
    local variants=$3
    local binary="$worktree/target/release/quent-bench"
    local csv="$RESULTS/$label.csv"
    local json="$RESULTS/$label.json"

    wait_idle
    printf '\nRUN %s\n' "$label"
    printf 'commit=%s variants=%s\n' "$(git -C "$worktree" rev-parse HEAD)" "$variants"
    snapshot
    /run/current-system/sw/bin/time -v env \
        BENCH_ATTRS="$ATTRS" \
        BENCH_THREADS="$THREADS" \
        BENCH_OPS="$OPS" \
        BENCH_REPS="$REPS" \
        BENCH_VARIANTS="$variants" \
        BENCH_CSV="$csv" \
        BENCH_JSON="$json" \
        RAYON_NUM_THREADS=24 \
        pixi run "$binary"
    sync
    snapshot
    printf 'DONE %s\n' "$label"
}

run_case baseline-start "$ROOT/base" "$baseline_union"

run_case candidate-01 "$ROOT/candidate-01" \
    "$postcard4,quent-static-log/postcard-producer,quent-dynamic-log/postcard-producer,quent-static-span/postcard-producer,quent-dynamic-span/postcard-producer"
run_case candidate-02 "$ROOT/candidate-02" "$postcard4"
run_case candidate-03 "$ROOT/candidate-03" \
    "$postcard4,quent-static-log/raw,quent-dynamic-log/raw,quent-static-span/raw,quent-dynamic-span/raw"
run_case candidate-04 "$ROOT/candidate-04" "$postcard4"
run_case candidate-05 "$ROOT/candidate-05" "$grpc4"
run_case candidate-06 "$ROOT/candidate-06" "$postcard4"
run_case candidate-07 "$ROOT/candidate-07" "$noop4"
run_case candidate-08 "$ROOT/candidate-08" \
    'quent-dynamic-log/noop,quent-dynamic-span/noop,quent-dynamic-log/postcard,quent-dynamic-span/postcard'
run_case candidate-09 "$ROOT/candidate-09" "$filesystem12"
run_case candidate-10 "$ROOT/candidate-10" \
    'quent-static-log/postcard,quent-static-span/postcard,quent-static-log/bincode-serde,quent-static-span/bincode-serde,quent-static-log/bincode-native,quent-static-span/bincode-native'
run_case candidate-11 "$ROOT/candidate-11" "$filesystem12"
run_case candidate-12 "$ROOT/candidate-12" \
    "$postcard4,quent-static-log/bitcode,quent-dynamic-log/bitcode,quent-static-span/bitcode,quent-dynamic-span/bitcode"
run_case candidate-13 "$ROOT/candidate-13" "$postcard4"
run_case candidate-14 "$ROOT/candidate-14" "$postcard4"

run_case baseline-end "$ROOT/base" "$baseline_union"

printf '\nALL RUNS COMPLETE\n'
snapshot
