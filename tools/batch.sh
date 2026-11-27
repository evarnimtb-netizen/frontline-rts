#!/usr/bin/env bash
# Cached deterministic runs of frontline-cli (docs/process/dev-speed.md).
#
#   tools/batch.sh OUT.json [-o OVERRIDE.toml]... -- SUBCOMMAND ARGS...
#   tools/batch.sh sets OUTDIR [--map ID] [-o OVERRIDE.toml]...   # the three 160 sets
#   tools/batch.sh teams OUTDIR [-o OVERRIDE.toml]...   # S17b: the three 2v2 40 sets
#   tools/batch.sh pair OUTDIR F0 F1 [--map ID] [-o OVERRIDE.toml]...
#       one 40 set of faction F0 (player 0) against F1: seeds 1-20, both
#       bases, 45 min, into OUTDIR/pair_<f0>_<f1>40.json (ids with or
#       without the `ww2:` prefix; S15 plan 4, M tools; `--map` as in sets,
#       the file then pair_<f0>_<f1>_<map>40.json)
#
# Runs `target/release/frontline-cli SUBCOMMAND ARGS... --json OUT.json`, text
# output to OUT.txt, and keeps both under target/batch-cache/KEY. KEY is the
# hash of the CLI binary, every file under mods/, the override files' contents
# and the arguments (without --jobs: results do not depend on it). The sim is
# deterministic, so a second call with the same key copies the files back
# instead of playing the matches again. S16a: `--map ID` plays the sets on
# that map; without it the CLI's default (Kessel Ridge) and the same cache
# keys as before. S17b: `teams` plays the 2v2 breakage sets on Harrow Cross
# (plan 9: seeds 1-20, both sides, 45 min), mixed teams (Germany and USA on
# each side) in skirmish2v2_mixed and same-faction teams (Germany against
# USA) in skirmish2v2_same; their JSON has an `s17_derived` block. Merge
# with S15: skirmish2v2_ussr puts the USSR on a team (USA and USSR against
# two Germans; every hostile pair a `[matchups]` pair).
#
# Each call runs a snapshot of the binary and of mods/ kept under
# target/batch-cache/world/KEY, so a rebuild or a TOML edit while a long run
# plays cannot change it. BATCH_WORLD=KEY pins a call to a snapshot taken
# earlier (tools/measure.sh does that for its four runs).
#
# Tick times in a cached file are from whichever run filled the cache: read
# tick times only from a fresh `--jobs 3` run made with BATCH_NO_CACHE=1 on a
# quiet machine.
#
# Env: BATCH_JOBS (default 6), BATCH_NO_CACHE=1 (always run, still stores),
# BATCH_WORLD (above). `tools/batch.sh world` prints the current key after
# taking the snapshot. The cache is safe to delete: rm -rf target/batch-cache.
set -u
cd "$(dirname "$0")/.."
CLI=target/release/frontline-cli
CACHE=target/batch-cache
JOBS="${BATCH_JOBS:-6}"

sha() { shasum -a 256 | cut -c1-24; }

world_key() { # binary + mods
    {
        shasum -a 256 "$CLI"
        find mods -type f -print0 | sort -z | xargs -0 shasum -a 256
    } | sha
}

run_one() { # OUT.json OVERRIDES... -- SUBCOMMAND ARGS...
    local out=$1
    shift
    local ovs=() args=() ovargs=()
    while [ $# -gt 0 ] && [ "$1" != "--" ]; do
        [ "$1" = "-o" ] && shift
        ovs+=("$1")
        shift
    done
    shift
    args=("$@")
    local key
    key=$({
        echo "$WORLD"
        for f in ${ovs[@]+"${ovs[@]}"}; do
            shasum -a 256 <"$f"
            ovargs+=(--rules-override "$f")
        done
        printf '%s\n' "${args[@]}"
    } | sha)
    # The subshell above cannot fill ovargs: build it again here.
    ovargs=()
    for f in ${ovs[@]+"${ovs[@]}"}; do ovargs+=(--rules-override "$f"); done
    local dir=$CACHE/$key txt=${out%.json}.txt
    mkdir -p "$(dirname "$out")"
    if [ -z "${BATCH_NO_CACHE:-}" ] && [ -f "$dir/out.json" ]; then
        cp "$dir/out.json" "$out"
        cp "$dir/out.txt" "$txt"
        echo "$(basename "$out"): cached ($key)"
        return 0
    fi
    local jobs=()
    # Only these two subcommands take --jobs (armour-test and fortify-test do not).
    case "${args[0]}" in skirmish | mortar-bench) jobs=(--jobs "$JOBS") ;; esac
    # A single match has no --jobs either; a caller's own --jobs wins.
    case " ${args[*]} " in *" --jobs "*) jobs=() ;; esac
    if [ "${args[0]}" = skirmish ]; then
        case " ${args[*]} " in *" --seeds "*) ;; *) jobs=() ;; esac
    fi
    local t0=$SECONDS
    "$SNAP/frontline-cli" "${args[@]}" --mods "$SNAP/mods/ww2" ${ovargs[@]+"${ovargs[@]}"} ${jobs[@]+"${jobs[@]}"} --json "$out" >"$txt" 2>&1
    local rc=$?
    if [ $rc -eq 0 ] && [ -s "$out" ]; then
        mkdir -p "$dir"
        cp "$out" "$dir/out.json"
        cp "$txt" "$dir/out.txt"
    fi
    echo "$(basename "$out"): ran in $((SECONDS - t0)) s, exit $rc ($key)"
    return $rc
}

if [ -n "${BATCH_WORLD:-}" ]; then
    WORLD=$BATCH_WORLD
    SNAP=$CACHE/world/$WORLD
    [ -x "$SNAP/frontline-cli" ] || { echo "no snapshot $SNAP" >&2; exit 2; }
else
    [ -x "$CLI" ] || { echo "no $CLI: cargo build --release -p frontline-cli" >&2; exit 2; }
    WORLD=$(world_key)
    SNAP=$CACHE/world/$WORLD
    if [ ! -x "$SNAP/frontline-cli" ]; then
        mkdir -p "$SNAP.tmp$$"
        cp "$CLI" "$SNAP.tmp$$/frontline-cli"
        cp -R mods "$SNAP.tmp$$/mods"
        # Taken while something wrote the binary or mods: not a snapshot.
        if [ "$(world_key)" != "$WORLD" ]; then
            rm -rf "$SNAP.tmp$$"
            echo "the binary or mods/ changed while the snapshot was taken: run again" >&2
            exit 2
        fi
        mkdir -p "$CACHE/world"
        mv "$SNAP.tmp$$" "$SNAP" 2>/dev/null || rm -rf "$SNAP.tmp$$"
    fi
fi

case "${1:-}" in
    world)
        echo "$WORLD"
        ;;
    sets)
        dir=${2:?usage: tools/batch.sh sets OUTDIR [--map ID] [-o OVERRIDE]...}
        shift 2
        map=()
        if [ "${1:-}" = --map ]; then
            map=(--map "${2:?--map needs an id}")
            shift 2
        fi
        rc=0
        for s in "skirmish160 1" "skirmish160_oos 101" "skirmish160_s201 201"; do
            set -- "$@"
            name=${s% *}
            seed=${s#* }
            run_one "$dir/$name.json" "$@" -- skirmish --ai-vs-ai --seed "$seed" --seeds 80 --both-bases --summary --minutes 45 ${map[@]+"${map[@]}"} || rc=1
        done
        exit $rc
        ;;
    teams)
        dir=${2:?usage: tools/batch.sh teams OUTDIR [-o OVERRIDE]...}
        shift 2
        rc=0
        for s in "skirmish2v2_mixed ww2:germany ww2:usa ww2:germany ww2:usa" \
            "skirmish2v2_same ww2:germany ww2:germany ww2:usa ww2:usa" \
            "skirmish2v2_ussr ww2:usa ww2:ussr ww2:germany ww2:germany"; do
            # shellcheck disable=SC2086
            set -- $s "$@"
            name=$1 f0=$2 f1=$3 f2=$4 f3=$5
            shift 5
            run_one "$dir/$name.json" "$@" -- skirmish --ai-vs-ai --teams 2v2 --map ww2:harrow_cross \
                --faction0 "$f0" --faction1 "$f1" --faction2 "$f2" --faction3 "$f3" \
                --seed 1 --seeds 20 --both-bases --summary --minutes 45 || rc=1
        done
        exit $rc
        ;;
    pair)
        dir=${2:?usage: tools/batch.sh pair OUTDIR F0 F1 [-o OVERRIDE]...}
        f0=${3:?usage: tools/batch.sh pair OUTDIR F0 F1 [-o OVERRIDE]...}
        f1=${4:?usage: tools/batch.sh pair OUTDIR F0 F1 [-o OVERRIDE]...}
        shift 4
        map=() tag=""
        if [ "${1:-}" = --map ]; then
            map=(--map "${2:?--map needs an id}")
            tag="_${2#*:}"
            shift 2
        fi
        run_one "$dir/pair_${f0#*:}_${f1#*:}${tag}40.json" "$@" -- skirmish --ai-vs-ai --faction0 "ww2:${f0#*:}" --faction1 "ww2:${f1#*:}" --seed 1 --seeds 20 --both-bases --summary --minutes 45 ${map[@]+"${map[@]}"}
        ;;
    "" | -h | --help)
        sed -n 2,38p "$0"
        exit 2
        ;;
    *)
        run_one "$@"
        ;;
esac
