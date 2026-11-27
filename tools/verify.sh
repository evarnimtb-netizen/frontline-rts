#!/usr/bin/env bash
# The orchestrator's full verification of a slice branch before a push
# (docs/process/dev-speed.md). Same checks as the per-slice verify scripts of
# S6-S9, minus the repeats: the release tests and the Godot tests run once
# (inside check.sh full), the deterministic runs go through tools/batch.sh
# (cached per binary + mods), and check.sh full, the armour and fortify runs play beside the
# 160 sets instead of after them.
#
#   tools/verify.sh OUTDIR TARGETS.toml [--map ID]
#
# S16 review: `--map ID` plays the timing 40 and the 160 sets on that map
# (S16 plan 7 reads tick p99 on Linden Brook); default the CLI's (Kessel
# Ridge, same cache keys). The armour, fortify and mortar runs keep their
# own test scenarios.
#
# Writes OUTDIR/summary.txt (one line per step with its seconds) and prints it.
# Run it alone on a quiet machine: step 1 measures tick time, before anything else starts.
# Env: SETTINGS_BAK (see check.sh), VERIFY_SERIAL=1 (armour after the sets).
set -u
cd "$(dirname "$0")/.."
O=${1:?usage: tools/verify.sh OUTDIR TARGETS.toml}
TARGETS=${2:?usage: tools/verify.sh OUTDIR TARGETS.toml [--map ID]}
MAP=()
if [ "${3:-}" = --map ]; then
    MAP=(--map "${4:?--map needs an id}")
fi
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
C=target/release/frontline-cli
mkdir -p "$O"
SUM=$O/summary.txt
: >"$SUM"
T0=$SECONDS
step() { echo "== $1: exit $2 ($3 s)" | tee -a "$SUM"; }
timed() { # NAME CMD... (stdout+stderr to OUTDIR/NAME.txt)
    local n=$1 t=$SECONDS
    shift
    "$@" >"$O/$n.txt" 2>&1
    step "$n" $? $((SECONDS - t))
}
{
    git rev-parse --abbrev-ref HEAD
    git log --oneline -1
    git status --porcelain | head -5
} | tee -a "$SUM"

# 1. Tick time first, alone on a quiet machine: 40 matches at 3 jobs, never
#    from the cache. (check.sh full builds the CLI later; build it here.)
cargo build -q --release -p frontline-cli >"$O/build_cli.txt" 2>&1 || { cat "$O/build_cli.txt"; exit 2; }
timed run_timing40 env BATCH_NO_CACHE=1 BATCH_JOBS=3 tools/batch.sh "$O/timing40.json" -- \
    skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary --minutes 45 ${MAP[@]+"${MAP[@]}"}
cp "$O/timing40.json" "$O/skirmish40.json"
python3 -I -c "
import json,sys
t=json.load(open('$O/timing40.json'))['tick_p99_ms']
print('tick p99 ms:', json.dumps(t))" | tee -a "$SUM"

# 2. Beside each other: check.sh full (fmt, clippy, release tests with the
#    ignored ones, golden, validate, bridge, Godot tests, settings.cfg), the
#    160 sets, and the fortify/mortar run and the two armour runs.
side() {
    if [ -n "${VERIFY_SERIAL:-}" ]; then
        timed fortify tools/batch.sh "$O/fortify10.json" -- fortify-test --seeds 10
        timed run_armour5 tools/batch.sh "$O/armour5.json" -- armour-test --seeds 5
        timed armour_stress tools/batch.sh "$O/armour_stress10.json" -- armour-test --stress --seeds 10
        timed mortar_bench "$C" mortar-bench --seeds 10 --jobs 2
        return
    fi
    ( timed fortify tools/batch.sh "$O/fortify10.json" -- fortify-test --seeds 10
      timed mortar_bench "$C" mortar-bench --seeds 10 --jobs 2 ) &
    timed run_armour5 tools/batch.sh "$O/armour5.json" -- armour-test --seeds 5 &
    timed armour_stress tools/batch.sh "$O/armour_stress10.json" -- armour-test --stress --seeds 10 &
    wait
}
if [ -n "${VERIFY_SERIAL:-}" ]; then
    timed check_full tools/check.sh full
    timed sets tools/batch.sh sets "$O" ${MAP[@]+"${MAP[@]}"}
    side
else
    timed check_full tools/check.sh full &
    CHK=$!
    side &
    SIDE=$!
    timed sets tools/batch.sh sets "$O" ${MAP[@]+"${MAP[@]}"}
    wait $SIDE $CHK
fi
cat "$O/check_full.txt" >>"$SUM"
cat "$O/sets.txt" >>"$SUM"
python3 -I tools/pool.py "$O" | tee -a "$SUM"

# 4. Targets, tuner self-test, determinism across job counts, doc sizes.
"$C" targets "$TARGETS" --from-json "$O" >"$O/targets.txt" 2>&1
echo "targets exit $?: $(tail -1 "$O/targets.txt")" | tee -a "$SUM"
grep ' MISS' "$O/targets.txt" | cut -c1-160 | tee -a "$SUM"
timed autotune_tests python3 -I tools/autotune/test_autotune.py
grep -E "0x[0-9a-f]{16}$" "$O/timing40.txt" | sort >"$O/h40.txt"
grep -E "0x[0-9a-f]{16}$" "$O/skirmish160.txt" | sort >"$O/h160.txt"
echo "hash lines of the jobs-3 run: $(wc -l <"$O/h40.txt" | tr -d ' '); found in the jobs-6 160 run: $(grep -Fxc -f "$O/h40.txt" "$O/h160.txt")" | tee -a "$SUM"
echo "STATE.md lines: $(wc -l <docs/STATE.md | tr -d ' ')" | tee -a "$SUM"
echo "verify: $(( (SECONDS - T0) / 60 )) min $(( (SECONDS - T0) % 60 )) s; failed steps: $(grep -c 'exit [1-9]' "$SUM")" | tee -a "$SUM"
