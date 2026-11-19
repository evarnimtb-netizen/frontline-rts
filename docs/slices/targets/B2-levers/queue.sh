#!/usr/bin/env bash
# B2 run queue (docs/slices/B2-run-1.md, B2-run-2.md). Plays one seed block
# for the base and for each named lever through tools/batch.sh (cached: a
# restart resumes at the first set not yet played), and after each lever
# appends its paired line to docs/slices/targets/B2-results/paired.txt and
# writes the per-match CSVs beside it (commit them: target/ may be lost).
#
#   queue.sh check                 # every lever file loads; prints what it changes
#   queue.sh BLOCK NAME...         # BLOCK: first seed (2001, 2101, 2201); 80 seeds, both bases
#   queue.sh --dry BLOCK NAME...   # prints the commands only
#   queue.sh diag40 NAME           # seeds 2001-2020 both bases (40 matches), base and NAME
#
# NAME is a file of this folder without .toml; "a+b" applies a.toml then
# b.toml (a combination). The base is always played first, with
# --side-detail (hashes equal with it, B1-3). Start it in the background:
#   tools/run.sh target/b2/q1.log docs/slices/targets/B2-levers/queue.sh 2001 mix_line35 ...
# Env: BATCH_JOBS (default here 4: the cloud machine has 4 cores).
set -u
cd "$(dirname "$0")/../../../.."
L=docs/slices/targets/B2-levers
R=docs/slices/targets/B2-results
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
        echo tools/batch.sh "$out" "$@" -- skirmish --ai-vs-ai --seed "$seed" --seeds "$seeds" --both-bases --summary --minutes 45 --side-detail
        return 0
    fi
    tools/batch.sh "$out" "$@" -- skirmish --ai-vs-ai --seed "$seed" --seeds "$seeds" --both-bases --summary --minutes 45 --side-detail
}

block() { # DIR SEED SEEDS NAME...
    local dir=$1 seed=$2 seeds=$3 name o
    shift 3
    play "$dir/base.json" "$seed" "$seeds" || { echo "base failed"; return 1; }
    for name in "$@"; do
        o=$(ovs "$name") || return 2
        # shellcheck disable=SC2086
        play "$dir/$name.json" "$seed" "$seeds" $o || { echo "$name failed"; continue; }
        [ -n "$DRY" ] && continue
        mkdir -p "$R"
        python3 -I "$L/paired.py" --csv "$R" "$name@$(basename "$dir")" "$dir/base.json" "$dir/$name.json" | tee -a "$R/paired.txt"
    done
}

case "${1:-}" in
    check)
        rc=0
        for f in "$L"/*.toml; do
            n=$(basename "$f" .toml)
            tools/batch.sh "target/b2/check/$n.json" -o "$f" -- skirmish --ai-vs-ai --seed 1 --minutes 1 >/dev/null
            c=$(grep -c -- ' -> ' "target/b2/check/$n.txt" 2>/dev/null)
            echo "$n: ${c:-0} value(s) changed"
            grep -- ' -> ' "target/b2/check/$n.txt" 2>/dev/null | sed 's/^/    /'
            [ "${c:-0}" -gt 0 ] || rc=1
        done
        exit $rc
        ;;
    diag40)
        block target/b2/d2001 2001 20 "${2:?usage: queue.sh diag40 NAME}"
        ;;
    "" | -h | --help)
        sed -n 2,18p "$0"
        exit 2
        ;;
    *)
        b=$1
        shift
        block "target/b2/s$b" "$b" 80 "$@"
        ;;
esac
