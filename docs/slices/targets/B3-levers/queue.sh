#!/usr/bin/env bash
# B3 run queue (docs/slices/B3-run-1.md; B2's queue with B3's paths). Plays
# one seed block for the base and for each named lever through tools/batch.sh
# (cached: a restart resumes at the first set not yet played), and after each
# lever appends its paired line to docs/slices/targets/B3-results/paired.txt,
# writes the per-match CSVs and the lever's counters beside it (commit them:
# target/ may be lost).
#
#   queue.sh check                 # every lever file loads; prints what it changes
#   queue.sh BLOCK NAME...         # BLOCK: first seed (3001, 3101, 3201); 80 seeds, both bases
#   queue.sh --dry BLOCK NAME...   # prints the commands only
#   queue.sh run1                  # armour own_class_twins, then base + 11 single levers (~2.5 h)
#   queue.sh armour NAME           # armour-test (5 seeds, all cases) base and NAME; prints what moved
#   queue.sh code                  # B3 code screen: base + code_direct_budget, code_direct_budget_cap6 (block 3001)
#   queue.sh run2                  # B3 run 2 confirm: base, c_he, c_all on F1 (3101) then F2 (3201): six sets
#
# NAME is a file of this folder without .toml; "a+b" applies a.toml then
# b.toml (a combination). The base is always played first. Start it in the
# background:
#   tools/run.sh target/b3/q1.log docs/slices/targets/B3-levers/queue.sh 3001 he_shell_cap6 ...
# Env: BATCH_JOBS (default here 4), SEEDS (default 80; a tiny test: SEEDS=2 B3_OUT=target/b3t B3_RESULTS=target/b3t/res queue.sh 3001 NAME).
set -u
cd "$(dirname "$0")/../../../.."
L=docs/slices/targets/B3-levers
R=${B3_RESULTS:-docs/slices/targets/B3-results}
export BATCH_JOBS="${BATCH_JOBS:-4}"
DRY=
[ "${1:-}" = "--dry" ] && { DRY=1; shift; }

ovs() { # NAME -> "-o file -o file"
    local out=() part
    for part in ${1//+/ }; do
        [ -f "$L/$part.toml" ] || { echo "no lever file $L/$part.toml" >&2; return 2; }
        out+=(-o "$L/$part.toml")
    done
    echo "${out[@]}"
}

play() { # OUT.json SEED SEEDS [-o FILE]...
    local out=$1 seed=$2 seeds=$3
    shift 3
    if [ -n "$DRY" ]; then
        echo tools/batch.sh "$out" "$@" -- skirmish --ai-vs-ai --seed "$seed" --seeds "$seeds" --both-bases --summary --minutes 45
        return 0
    fi
    tools/batch.sh "$out" "$@" -- skirmish --ai-vs-ai --seed "$seed" --seeds "$seeds" --both-bases --summary --minutes 45
}

block() { # DIR SEED SEEDS NAME...
    local dir=$1 seed=$2 seeds=$3 name o tag
    shift 3
    play "$dir/base.json" "$seed" "$seeds" || { echo "base failed"; return 1; }
    [ -n "$DRY" ] || { mkdir -p "$R"; python3 -I "$L/paired.py" --counters "$dir/base.json" >"$R/counters_$(basename "$dir")_base.txt"; }
    for name in "$@"; do
        o=$(ovs "$name") || return 2
        # shellcheck disable=SC2086
        play "$dir/$name.json" "$seed" "$seeds" $o || { echo "$name failed"; continue; }
        [ -n "$DRY" ] && continue
        tag="$name@$(basename "$dir")"
        # A resumed queue reads a lever again: its old line is replaced, not repeated.
        if [ -f "$R/paired.txt" ]; then
            grep -v -F -- "$tag " "$R/paired.txt" >"$R/paired.txt.tmp" || true
            mv "$R/paired.txt.tmp" "$R/paired.txt"
        fi
        python3 -I "$L/paired.py" --csv "$R" "$tag" "$dir/base.json" "$dir/$name.json" | tee -a "$R/paired.txt"
        python3 -I "$L/paired.py" --counters "$dir/$name.json" >"$R/counters_$(basename "$dir")_$name.txt"
    done
}

case "${1:-}" in
    check)
        rc=0
        for f in "$L"/*.toml; do
            n=$(basename "$f" .toml)
            tools/batch.sh "target/b3/check/$n.json" -o "$f" -- skirmish --ai-vs-ai --seed 1 --minutes 1 >/dev/null
            c=$(grep -c -- ' -> ' "target/b3/check/$n.txt" 2>/dev/null)
            echo "$n: ${c:-0} value(s) changed"
            grep -- ' -> ' "target/b3/check/$n.txt" 2>/dev/null | sed 's/^/    /'
            [ "${c:-0}" -gt 0 ] || rc=1
        done
        exit $rc
        ;;
    armour)
        name=${2:?usage: queue.sh armour NAME}
        o=$(ovs "$name") || exit 2
        mkdir -p "$R" target/b3/armour
        # A failing hard check makes armour-test exit 1, so batch.sh does not cache it:
        # a finished JSON in target/b3/armour is kept and not played again (about 14 min each).
        # Base and lever run side by side (one thread each).
        if [ ! -s target/b3/armour/base.json ]; then
            tools/batch.sh target/b3/armour/base.json -- armour-test --seeds 5 || echo "base armour-test exit $? (hard checks failing is normal)" &
        fi
        if [ ! -s "target/b3/armour/$name.json" ]; then
            # shellcheck disable=SC2086
            tools/batch.sh "target/b3/armour/$name.json" $o -- armour-test --seeds 5 || echo "$name armour-test exit $?" &
        fi
        wait
        python3 -I "$L/paired.py" --armour target/b3/armour/base.json "target/b3/armour/$name.json" | tee "$R/armour_$name.txt"
        ;;
    run1)
        # Everything of run 1 that needs no decision: armour test, base, the eleven single levers.
        "$0" armour own_class_twins
        "$0" 3001 he_shell_cap6 own_m17_160_64 own_rg_gap25 hv_save360 own_m36_h1 hv_ger_reach mix_vehicle30 \
            he_mission3 own_m36_w150 diag_head300 own_s12_bleed120_300
        echo "run 1 done. Summary: $R/paired.txt (one line per lever), $R/counters_s3001_*.txt, $R/armour_own_class_twins.txt, CSVs in $R; JSON in target/b3/s3001/. Commit $R."
        ;;
    code)
        # B3 code (SAVE 32, `[ai.arty] direct_in_budget`): the new binary replays the base (switch 0
        # is neutral: same numbers as run 1's base), then the switch alone and with the 6 % cap.
        "$0" 3001 code_direct_budget code_direct_budget_cap6
        echo "code screen done. Summary: $R/paired.txt (code_direct_budget*@s3001), $R/counters_s3001_code_*.txt. Commit $R."
        ;;
    run2)
        # B3 run 2 (docs/slices/B3-run-2.md): the two candidates on both fresh blocks.
        "$0" 3101 c_he c_all
        "$0" 3201 c_he c_all
        echo "run 2 done. Summary: $R/paired.txt (c_he@s3101 ... c_all@s3201), $R/counters_s3[12]01_*.txt. Commit $R."
        ;;
    "" | -h | --help)
        sed -n 2,20p "$0"
        exit 2
        ;;
    *)
        b=$1
        shift
        block "${B3_OUT:-target/b3}/s$b" "$b" "${SEEDS:-80}" "$@"
        ;;
esac
