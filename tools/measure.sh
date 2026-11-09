#!/usr/bin/env bash
# The slice's numbers in about 30 lines, with no agent waiting on them
# (docs/process/dev-speed.md). The orchestrator runs this between stages and
# hands the output file to the next stage; agents do not replay the 160 sets.
#
#   tools/measure.sh OUTDIR TARGETS.toml [-o OVERRIDE.toml]...
#
# Builds the release CLI, plays the 40 set and the three 160 sets through
# tools/batch.sh (cached per binary + mods + overrides), prints the pooled
# reading (tools/pool.py) and the target lines that miss. Everything also goes
# to OUTDIR/measure.txt. Tick times are NOT measured here (tools/verify.sh).
#
# Env: BASELINE=DIR prints DIR's pooled block first and the change against it.
set -u
cd "$(dirname "$0")/.."
OUT=${1:?usage: tools/measure.sh OUTDIR TARGETS.toml [-o OVERRIDE]...}
TARGETS=${2:?usage: tools/measure.sh OUTDIR TARGETS.toml [-o OVERRIDE]...}
shift 2
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
mkdir -p "$OUT"
{
    echo "measure: $(git rev-parse --short HEAD)$(git diff --quiet || echo ' + uncommitted changes') $(date '+%H:%M') overrides: ${*:-none}"
    if ! cargo build -q --release -p frontline-cli 2>"$OUT/build.log"; then
        echo "build: FAIL"
        grep -E '^error' -A6 "$OUT/build.log" | head -20
        exit 1
    fi
    tools/batch.sh "$OUT/skirmish40.json" "$@" -- skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary --minutes 45
    tools/batch.sh sets "$OUT" "$@"
    python3 -I tools/pool.py ${BASELINE:+"$BASELINE"} "$OUT"
    target/release/frontline-cli targets "$TARGETS" --from-json "$OUT" >"$OUT/targets.txt" 2>&1
    echo "targets (skirmish sets only; armour, fortify and timing runs are in tools/verify.sh): $(tail -1 "$OUT/targets.txt")"
    grep ' MISS' "$OUT/targets.txt" | cut -c1-150
} 2>&1 | tee "$OUT/measure.txt"
