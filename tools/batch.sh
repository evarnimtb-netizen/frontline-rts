#!/usr/bin/env bash
# Cached deterministic runs of frontline-cli (docs/process/dev-speed.md).
#
#   tools/batch.sh OUT.json [-o OVERRIDE.toml]... -- SUBCOMMAND ARGS...
#   tools/batch.sh sets OUTDIR [-o OVERRIDE.toml]...   # the three 160 sets
#
# Runs `target/release/frontline-cli SUBCOMMAND ARGS... --json OUT.json`, text
# output to OUT.txt, and keeps both under target/batch-cache/KEY. KEY is the
# hash of the CLI binary, every file under mods/, the override files' contents
# and the arguments (without --jobs: results do not depend on it). The sim is
# deterministic, so a second call with the same key copies the files back
# instead of playing the matches again.
#
# Tick times in a cached file are from whichever run filled the cache: read
# tick times only from a fresh `--jobs 3` run made with BATCH_NO_CACHE=1 on a
# quiet machine.
#
# Env: BATCH_JOBS (default 6), BATCH_NO_CACHE=1 (always run, still stores).
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
    case " ${args[*]} " in *" --seeds "* | *" mortar-bench "*) jobs=(--jobs "$JOBS") ;; esac
    local t0=$SECONDS
    "$CLI" "${args[@]}" ${ovargs[@]+"${ovargs[@]}"} ${jobs[@]+"${jobs[@]}"} --json "$out" >"$txt" 2>&1
    local rc=$?
    if [ $rc -eq 0 ] && [ -s "$out" ]; then
        mkdir -p "$dir"
        cp "$out" "$dir/out.json"
        cp "$txt" "$dir/out.txt"
    fi
    echo "$(basename "$out"): ran in $((SECONDS - t0)) s, exit $rc ($key)"
    return $rc
}

[ -x "$CLI" ] || { echo "no $CLI: cargo build --release -p frontline-cli" >&2; exit 2; }
WORLD=$(world_key)

case "${1:-}" in
    sets)
        dir=${2:?usage: tools/batch.sh sets OUTDIR [-o OVERRIDE]...}
        shift 2
        rc=0
        for s in "skirmish160 1" "skirmish160_oos 101" "skirmish160_s201 201"; do
            set -- "$@"
            name=${s% *}
            seed=${s#* }
            run_one "$dir/$name.json" "$@" -- skirmish --ai-vs-ai --seed "$seed" --seeds 80 --both-bases --summary --minutes 45 || rc=1
        done
        exit $rc
        ;;
    "" | -h | --help)
        sed -n 2,18p "$0"
        exit 2
        ;;
    *)
        run_one "$@"
        ;;
esac
