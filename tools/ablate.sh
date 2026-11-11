#!/usr/bin/env bash
# S10a-ai ablations (docs/slices/S10a-ai.md 9): one tools/measure.sh 480 per
# override file, each against the slice head's switches-on measure dir, and
# one line per ablation at the end. The orchestrator runs it (a full run is
# five measure runs, over an hour uncached: start it in the background).
#
#   tools/ablate.sh ONDIR OUTROOT [NAME]...    # NAME: all_off works_off ...
#   tools/ablate.sh ONDIR OUTROOT nest_off dugout_off roof_off sandbags_off
#   tools/ablate.sh --dry-run ONDIR OUTROOT [all]   # checks, prints the commands
#
# ONDIR    the switches-on measure dir (tools/measure.sh OUT TARGETS), the
#          BASELINE of every ablation.
# OUTROOT  each ablation measures into OUTROOT/NAME (measure.txt there).
# NAMEs    default: the five of spec 9 (all_off works_off wire_off breach_off
#          cards_off); `all`: every file in docs/slices/targets/S10a-ai-ablations/
#          (also the per-work four of stage D: nest_off dugout_off roof_off
#          sandbags_off).
#
# The line per ablation: pooled Germany and its change against ONDIR, and
# whether it matters (spec 9: |change| >= 2 points) or decides Tune (>= 4).
# all_off also runs tools/neutral.py against NEUTRAL_BASE (default
# target/s10a-verify): it must say same 160/160 on all three sets. A run
# that measures all_off fails (before measuring) when NEUTRAL_BASE is
# missing, and fails when neutral.py does not say same (S10a-ai fix F11).
#
# --dry-run builds nothing and plays nothing: every override file must hold
# only `[ai.fort]` switch keys set to 0, the CLI (if built) must accept and
# apply each one (a 1-minute match), and the measure commands are printed.
# tools/check.sh quick runs it.
#
# Env: TARGETS (default docs/slices/targets/S10a-ai.toml), NEUTRAL_BASE.
set -u
cd "$(dirname "$0")/.."
DIR=docs/slices/targets/S10a-ai-ablations
TARGETS=${TARGETS:-docs/slices/targets/S10a-ai.toml}
NEUTRAL_BASE=${NEUTRAL_BASE:-target/s10a-verify}
DRY=0
if [ "${1:-}" = "--dry-run" ]; then
    DRY=1
    shift
fi
ON=${1:?usage: tools/ablate.sh [--dry-run] ONDIR OUTROOT [NAME]...}
ROOT=${2:?usage: tools/ablate.sh [--dry-run] ONDIR OUTROOT [NAME]...}
shift 2
if [ $# -eq 1 ] && [ "$1" = all ]; then
    NAMES=()
    for f in "$DIR"/*.toml; do
        n=$(basename "$f" .toml)
        NAMES+=("$n")
    done
elif [ $# -gt 0 ]; then
    NAMES=("$@")
else
    NAMES=(all_off works_off wire_off breach_off cards_off)
fi
[ ${#NAMES[@]} -gt 0 ] || { echo "ablate: no override in $DIR"; exit 1; }

# Each file: only [ai.fort] switches (works, wire, breach, cards, and the
# per-work nest, dugout, roof, sandbags) set to 0.
# (No tomllib on this Mac's python: the files are plain enough to read by
# line.) Prints the number of keys.
check_file() {
    python3 -I - "$1" <<'PY'
import re, sys
path = sys.argv[1]
switches = {"works", "wire", "breach", "cards", "nest", "dugout", "roof", "sandbags"}
table, keys = None, {}
for n, raw in enumerate(open(path), 1):
    line = raw.split("#", 1)[0].strip()
    if not line:
        continue
    m = re.fullmatch(r"\[([A-Za-z0-9_.]+)\]", line)
    if m:
        table = m.group(1)
        if table != "ai.fort":
            sys.exit(f"{path}:{n}: only an [ai.fort] table, not [{table}]")
        continue
    m = re.fullmatch(r"([a-z_]+)\s*=\s*(-?[0-9]+)", line)
    if not m or table != "ai.fort":
        sys.exit(f"{path}:{n}: not an [ai.fort] integer key: {line}")
    k, v = m.group(1), int(m.group(2))
    if k not in switches or v != 0:
        sys.exit(f"{path}:{n}: only the switches {sorted(switches)} set to 0, not {k} = {v}")
    if k in keys:
        sys.exit(f"{path}:{n}: {k} twice")
    keys[k] = v
if not keys:
    sys.exit(f"{path}: no switch set")
print(len(keys))
PY
}

FAIL=0
for n in "${NAMES[@]}"; do
    f="$DIR/$n.toml"
    if [ ! -f "$f" ]; then
        echo "ablate: $f: no such override"
        FAIL=1
        continue
    fi
    check_file "$f" >/dev/null || FAIL=1
done
[ $FAIL -eq 0 ] || exit 1

if [ $DRY -eq 1 ]; then
    CLI=target/release/frontline-cli
    for n in "${NAMES[@]}"; do
        f="$DIR/$n.toml"
        if [ -x "$CLI" ]; then
            keys=$(check_file "$f")
            applied=$("$CLI" skirmish --ai-vs-ai --seed 1 --seeds 1 --minutes 1 --rules-override "$f" --summary 2>&1 |
                grep -c "^override $f: .*ai\.fort\.[a-z]*: 1 -> 0")
            if [ "$applied" != "$keys" ]; then
                echo "ablate: $n: the CLI applied $applied of $keys keys"
                FAIL=1
                continue
            fi
        fi
        echo "dry-run $n: BASELINE=$ON tools/measure.sh $ROOT/$n $TARGETS -o $f"
    done
    [ $FAIL -eq 0 ] && echo "ablate dry-run: ${#NAMES[@]} overrides ok"
    exit $FAIL
fi

[ -d "$ON" ] || { echo "ablate: $ON: no switches-on measure dir"; exit 1; }
for n in "${NAMES[@]}"; do
    if [ "$n" = all_off ] && [ ! -d "$NEUTRAL_BASE" ]; then
        echo "ablate: all_off needs NEUTRAL_BASE $NEUTRAL_BASE (the neutrality verdict)"
        exit 1
    fi
done
gate() { # DIR -> pooled Germany (%), from tools/pool.py's GATE line
    python3 -I tools/pool.py "$1" 2>/dev/null | awk '/GATE pool=/ { sub(/.*pool=/, ""); print $1; exit }'
}
ON_POOL=$(gate "$ON")
[ -n "$ON_POOL" ] || { echo "ablate: $ON: no pooled reading (tools/pool.py)"; exit 1; }
LINES=()
for n in "${NAMES[@]}"; do
    out="$ROOT/$n"
    BASELINE="$ON" tools/measure.sh "$out" "$TARGETS" -o "$DIR/$n.toml" >/dev/null 2>&1
    pool=$(gate "$out")
    if [ -z "$pool" ]; then
        LINES+=("$n: FAIL (no reading; see $out/measure.txt)")
        FAIL=1
        continue
    fi
    line=$(python3 -I -c '
import sys
on, off = float(sys.argv[1]), float(sys.argv[2])
d = off - on
verdict = "decides Tune (>= 4)" if abs(d) >= 4 else "matters (>= 2)" if abs(d) >= 2 else "not decisive (< 2)"
print(f"Germany {off:.2f} % (on {on:.2f}, change {d:+.2f}): {verdict}")
' "$ON_POOL" "$pool")
    if [ "$n" = all_off ]; then
        if neutral=$(python3 -I tools/neutral.py "$NEUTRAL_BASE" "$out"); then
            st=0
        else
            st=1
        fi
        neutral=$(printf '%s' "$neutral" | tr '\n' ' ')
        line="$line; neutral vs $NEUTRAL_BASE: $neutral"
        # neutral.py exits 0 only when every file compared is the same
        # (and at least one was compared).
        if [ $st -ne 0 ]; then
            line="FAIL (not neutral) $line"
            FAIL=1
        fi
    fi
    LINES+=("$n: $line (pool.py change lines: $out/measure.txt)")
done
for l in "${LINES[@]}"; do
    echo "ablate $l"
done
exit $FAIL
