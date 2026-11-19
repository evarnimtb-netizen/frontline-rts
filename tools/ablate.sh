#!/usr/bin/env bash
# Ablations (S10a-ai.md 9, S10b-ai.md 2): one tools/measure.sh 480 per
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
# NAMEs    default with the S10a dir: the five of S10a-ai spec 9 (all_off
#          works_off wire_off breach_off cards_off); otherwise, and with `all`:
#          every *_off.toml in ABLATIONS (a control such as S10b-ai's
#          loose.toml is not an ablation and is never run here).
#
# The line per ablation: pooled Germany and its change against ONDIR, and
# whether it matters (spec 9: |change| >= 2 points) or decides Tune (>= 4).
# all_off also runs tools/neutral.py against NEUTRAL_BASE (default
# target/s10b-verify): it must say same 160/160 on all three sets. A run
# that measures all_off fails (before measuring) when NEUTRAL_BASE is
# missing, and fails when neutral.py does not say same (S10a-ai fix F11).
#
# --dry-run builds nothing and plays nothing: every override file must hold
# only switch keys set to their off value ([ai.fort] of S10a-ai; [ai.obstacles],
# [fortify] mine_paths and [ai.truck] refusal_memory of S10b-ai), the CLI
# (if built) must accept and
# apply each one (a 1-minute match), and the measure commands are printed.
# tools/check.sh quick runs it.
#
# S10c-ai (docs/slices/S10c-ai.md 2): with ABLATIONS=docs/slices/targets/
# S10c-ai-ablations every *.toml there is an override (c_off, front_off,
# mine_assign_off, belt_off, jobs1, builders2, builders0, site80), and a file may hold
# only the S10c keys at their listed values ([ai.obstacles] front,
# mine_assign, belt 0; jobs_max 1; builder_cards_min 0 or 2;
# front_site_threat_m 80; front_wire_len_m 20; front_slots 1; front_home 0
# or 1, the orchestrator's front_home_on of 2026-11-16; mine_assign_reach_m
# 450, 600 or 800, fix pass 3's reach450/600/800). c_off is the
# neutrality control there (as all_off: neutral.py against NEUTRAL_BASE,
# default target/s10c-base40, the 40 set of main 54009f6).
#
# S11a1 (docs/slices/S11a1.md 2): with ABLATIONS=docs/slices/targets/
# S11a1-ablations every *.toml there is an override (a_off, arty_off,
# fo_off, prep_off, radio_off, base_rearm_off), and a file may hold only the
# S11a1 switches: [fire_mission] gun_missions, radio_link, mask 0; [supply]
# arty_base_rearm 0 or 1; [ai.arty] buy, fo, prep 0. a_off is the
# neutrality control there (NEUTRAL_BASE default target/s11a1-base40, the
# 40 set of base 730e67f).
#
# S11a2 (docs/slices/S11a2.md 2): with ABLATIONS=docs/slices/targets/
# S11a2-ablations every *.toml there is an override (b_off, ff_off,
# ff_safe_off, cb_off, counter_fire_off), and a file may hold only the
# S11a2 switches at 0: [fire_mission] friendly_fire, counter_battery;
# [ai.arty] ff_safe, counter_fire. b_off is the neutrality control there
# (NEUTRAL_BASE default target/s11a2-base40, the 40 set of base b74ed58).
#
# Env: ABLATIONS=DIR (default docs/slices/targets/S10b-ai-ablations; the
# S10a set is docs/slices/targets/S10a-ai-ablations), TARGETS (default the
# slice file beside DIR: S10b-ai.toml / S10a-ai.toml), NEUTRAL_BASE (default
# target/s10b-verify; target/s10c-base40 with the S10c dir).
set -u
cd "$(dirname "$0")/.."
DIR=${ABLATIONS:-docs/slices/targets/S10b-ai-ablations}
DIR=${DIR%/}
TARGETS=${TARGETS:-${DIR%-ablations}.toml}
S10C=0
NEUTRAL_NAME=all_off
MODE=default
if [ "$(basename "$DIR")" = S10c-ai-ablations ]; then
    S10C=1
    MODE=s10c
    NEUTRAL_NAME=c_off
    NEUTRAL_BASE=${NEUTRAL_BASE:-target/s10c-base40}
fi
if [ "$(basename "$DIR")" = S11a1-ablations ]; then
    # The S10c rules for listing and applying (every *.toml; a key may
    # already hold its value), the S11a1 key list.
    S10C=1
    MODE=s11a1
    NEUTRAL_NAME=a_off
    NEUTRAL_BASE=${NEUTRAL_BASE:-target/s11a1-base40}
fi
if [ "$(basename "$DIR")" = S11a2-ablations ]; then
    # The S10c rules for listing and applying, the S11a2 key list.
    S10C=1
    MODE=s11a2
    NEUTRAL_NAME=b_off
    NEUTRAL_BASE=${NEUTRAL_BASE:-target/s11a2-base40}
fi
NEUTRAL_BASE=${NEUTRAL_BASE:-target/s10b-verify}
DRY=0
if [ "${1:-}" = "--dry-run" ]; then
    DRY=1
    shift
fi
ON=${1:?usage: tools/ablate.sh [--dry-run] ONDIR OUTROOT [NAME]...}
ROOT=${2:?usage: tools/ablate.sh [--dry-run] ONDIR OUTROOT [NAME]...}
shift 2
if { [ $# -eq 1 ] && [ "$1" = all ]; } || { [ $# -eq 0 ] && [ "$(basename "$DIR")" != S10a-ai-ablations ]; }; then
    NAMES=()
    GLOB="$DIR/*_off.toml"
    [ $S10C -eq 1 ] && GLOB="$DIR/*.toml"
    for f in $GLOB; do
        [ -f "$f" ] || continue
        n=$(basename "$f" .toml)
        NAMES+=("$n")
    done
elif [ $# -gt 0 ]; then
    NAMES=("$@")
else
    NAMES=(all_off works_off wire_off breach_off cards_off)
fi
[ ${#NAMES[@]} -gt 0 ] || { echo "ablate: no override in $DIR"; exit 1; }

# Each file: only switches set to their off value: [ai.fort] works, wire,
# breach, cards and the per-work nest, dugout, roof, sandbags (S10a-ai);
# [ai.obstacles] mines, at_mines, obstacles, bunker, sweep, demolish,
# [fortify] mine_paths and [ai.truck] refusal_memory (S10b-ai) at 0; the
# S10b-ai Build E keys lane_file, bunker_assign, bunker_truck_first at 0,
# at_rows at 1 and barrier_job_max_s at 900 (the job_max_s of every job);
# B1-0 demolish_handover and bunker_yields at 0.
# (No tomllib on this Mac's python: the files are plain enough to read by
# line.) Prints the number of keys.
check_file() {
    python3 -I - "$1" "$MODE" <<'PY'
import re, sys
path = sys.argv[1]
mode = sys.argv[2]
zero = lambda *ks: {k: 0 for k in ks}
# S10c-ai: each key with the values its override files may set.
allowed = {
    "ai.obstacles": {
        "front": {0}, "mine_assign": {0}, "belt": {0}, "jobs_max": {1},
        "builder_cards_min": {0, 2}, "front_site_threat_m": {80},
        "front_wire_len_m": {20}, "front_slots": {1}, "front_home": {0, 1},
        "mine_assign_reach_m": {450, 600, 800},
    },
}
switches = {
    "ai.fort": zero("works", "wire", "breach", "cards", "nest", "dugout", "roof", "sandbags"),
    "ai.obstacles": {
        **zero("mines", "at_mines", "obstacles", "bunker", "sweep", "demolish",
               "lane_file", "bunker_assign", "bunker_truck_first",
               "demolish_handover", "bunker_yields"),
        "at_rows": 1,
        "barrier_job_max_s": 900,
    },
    "fortify": zero("mine_paths"),
    "ai.truck": zero("refusal_memory"),
}
# S11a1: its switches (arty_base_rearm may also state its default 1).
allowed_s11a1 = {
    "fire_mission": {"gun_missions": {0}, "radio_link": {0}, "mask": {0}},
    "supply": {"arty_base_rearm": {0, 1}},
    "ai.arty": {"buy": {0}, "fo": {0}, "prep": {0}},
}
if mode == "s10c":
    switches = allowed
elif mode == "s11a1":
    switches = allowed_s11a1
elif mode == "s11a2":
    # S11a2: its four switches, at 0 only.
    switches = {
        "fire_mission": {"friendly_fire": {0}, "counter_battery": {0}},
        "ai.arty": {"ff_safe": {0}, "counter_fire": {0}},
    }
else:
    switches = {t: {k: {v} for k, v in ks.items()} for t, ks in switches.items()}
table, keys = None, {}
for n, raw in enumerate(open(path), 1):
    line = raw.split("#", 1)[0].strip()
    if not line:
        continue
    m = re.fullmatch(r"\[([A-Za-z0-9_.]+)\]", line)
    if m:
        table = m.group(1)
        if table not in switches:
            sys.exit(f"{path}:{n}: only the tables {sorted(switches)}, not [{table}]")
        continue
    m = re.fullmatch(r"([a-z_]+)\s*=\s*(-?[0-9]+)", line)
    if not m or table not in switches:
        sys.exit(f"{path}:{n}: not a switch table's integer key: {line}")
    k, v = m.group(1), int(m.group(2))
    if k not in switches[table] or v not in switches[table][k]:
        offs = ", ".join(f"{a} = {'/'.join(map(str, sorted(b)))}" for a, b in sorted(switches[table].items()))
        sys.exit(f"{path}:{n}: only the switches at their off value ({offs}), not {k} = {v}")
    if (table, k) in keys:
        sys.exit(f"{path}:{n}: {k} twice")
    keys[(table, k)] = v
if not keys:
    sys.exit(f"{path}: no switch set")
print(len(keys))
PY
}

# S10c-ai (fix pass 1, A-3): the keys of override FILE that the CLI output
# on stdin echoed as changed to their file value, or that
# mods/ww2/defs/rules/skirmish.toml already holds at that value. Prints the
# count.
s10c_applied() {
    python3 -I - "$1" mods/ww2/defs/rules/skirmish.toml 3<&0 <<'PY'
import os, re, sys
out = os.fdopen(3).read()
def table_keys(path):
    table, keys = None, {}
    for raw in open(path):
        line = raw.split("#", 1)[0].strip()
        m = re.fullmatch(r"\[([A-Za-z0-9_.]+)\]", line)
        if m:
            table = m.group(1)
            continue
        m = re.fullmatch(r"([a-z_]+)\s*=\s*(-?[0-9]+)", line)
        if m and table:
            keys[(table, m.group(1))] = int(m.group(2))
    return keys
want, mod = table_keys(sys.argv[1]), table_keys(sys.argv[2])
n = 0
for (t, k), v in want.items():
    echoed = re.search(rf": {re.escape(t)}\.{k}: -?[0-9]+ -> {v}$", out, re.M)
    if echoed or mod.get((t, k)) == v:
        n += 1
print(n)
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
            out=$("$CLI" skirmish --ai-vs-ai --seed 1 --seeds 1 --minutes 1 --rules-override "$f" --summary 2>&1)
            st=$?
            applied=$(printf '%s\n' "$out" |
                grep -cE "^override $f: .*: (ai\.fort|ai\.obstacles|fortify|ai\.truck|ai\.arty|fire_mission|supply)\.[a-z_]*: [0-9]+ -> [0-9]+$")
            # S10c-ai: a key may already hold its listed value (c_off sets
            # `front_slots` 1, the default). Fix pass 1 (finding A-3):
            # each key of the file counts once, when the CLI echoed its
            # change or the mod's skirmish.toml already holds that value;
            # a key neither echoed nor at its default is not applied.
            if [ $S10C -eq 1 ] && [ $st -eq 0 ]; then
                applied=$(printf '%s\n' "$out" | s10c_applied "$f")
            fi
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
    if [ "$n" = "$NEUTRAL_NAME" ] && [ ! -d "$NEUTRAL_BASE" ]; then
        echo "ablate: $n needs NEUTRAL_BASE $NEUTRAL_BASE (the neutrality verdict)"
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
    if [ "$n" = "$NEUTRAL_NAME" ]; then
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
