#!/usr/bin/env bash
# The slice's numbers in about 30 lines, with no agent waiting on them
# (docs/process/dev-speed.md). The orchestrator runs this between stages and
# hands the output file to the next stage; agents do not replay the 160 sets.
#
#   tools/measure.sh OUTDIR TARGETS.toml [-o OVERRIDE.toml]...
#
# Builds the release CLI, plays the 40 set, every skirmish40_* run of the
# targets file (with its rules_override) and the three 160 sets through
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
    # One snapshot of the binary and mods for all four runs: later rebuilds
    # and TOML edits do not reach them.
    BATCH_WORLD=$(tools/batch.sh world) || exit 1
    export BATCH_WORLD
    echo "world $BATCH_WORLD"
    tools/batch.sh "$OUT/skirmish40.json" "$@" -- skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary --minutes 45
    # S10b-ai: every skirmish40_* run of the targets file (the controls,
    # e.g. skirmish40_off / skirmish40_loose) with its rules_override files
    # added to this call's overrides.
    python3 -I - "$TARGETS" >"$OUT/controls.lst" <<'PY'
import re, sys
# Python 3.9 has no tomllib: the [runs.*] tables are flat `key = value` lines.
runs, cur = {}, None
for line in open(sys.argv[1]):
    line = line.split("#", 1)[0].strip()
    m = re.match(r"^\[runs\.([A-Za-z0-9_]+)\]$", line)
    if m:
        cur = runs.setdefault(m.group(1), {})
    elif line.startswith("["):
        cur = None
    elif cur is not None and "=" in line:
        k, v = (x.strip() for x in line.split("=", 1))
        cur[k] = re.findall(r'"([^"]*)"', v) if v.startswith("[") else v.strip('"')
for name, r in sorted(runs.items()):
    if name.startswith("skirmish40_") and r.get("kind") == "skirmish":
        ovs = " ".join("-o " + o for o in r.get("rules_override", []))
        both = "--both-bases" if r.get("both_bases") == "true" else ""
        print(f"{name}|{r.get('seeds', 20)}|{r.get('first_seed', 1)}|{r.get('minutes', 30)}|{both}|{ovs}")
PY
    while IFS='|' read -r name seeds first minutes both ovs; do
        [ -n "$name" ] || continue
        # shellcheck disable=SC2086
        tools/batch.sh "$OUT/$name.json" "$@" $ovs -- skirmish --ai-vs-ai --seed "$first" --seeds "$seeds" $both --summary --minutes "$minutes"
    done <"$OUT/controls.lst"
    tools/batch.sh sets "$OUT" "$@"
    python3 -I tools/pool.py ${BASELINE:+"$BASELINE"} "$OUT"
    target/release/frontline-cli targets "$TARGETS" --from-json "$OUT" >"$OUT/targets.txt" 2>&1
    echo "targets (skirmish sets only; armour, fortify and timing runs are in tools/verify.sh): $(tail -1 "$OUT/targets.txt")"
    grep ' MISS' "$OUT/targets.txt" | cut -c1-150
} 2>&1 | tee "$OUT/measure.txt"
