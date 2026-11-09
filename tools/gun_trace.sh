#!/usr/bin/env bash
# Traces AI-vs-AI matches and prints the gun funnel (tools/gun_funnel.py):
# why guns do or do not put armour out of action. About 1.5 minutes for the
# default 40 matches.
#
#   tools/gun_trace.sh OUTDIR [SEEDS] [FIRST_SEED] [-o OVERRIDE.toml]...
#
# Plays seeds FIRST_SEED .. FIRST_SEED+SEEDS-1 (default 20 from 1) with both
# base orders, five matches at a time, one trace file per match in OUTDIR.
set -u
cd "$(dirname "$0")/.."
OUT=${1:?usage: tools/gun_trace.sh OUTDIR [SEEDS] [FIRST_SEED] [-o OVERRIDE]...}
N=${2:-20}
FIRST=${3:-1}
shift $(($# < 3 ? $# : 3))
OV=""
while [ $# -gt 0 ]; do
    [ "$1" = "-o" ] && shift
    OV="$OV --rules-override $1"
    shift
done
mkdir -p "$OUT"
rm -f "$OUT"/*.jsonl
for s in $(seq "$FIRST" $((FIRST + N - 1))); do
    echo "$s n"
    echo "$s s"
done | xargs -P 5 -L 1 sh -c 'target/release/frontline-cli skirmish --ai-vs-ai --seed "$1" --minutes 45 --threads 1 $( [ "$2" = s ] && echo --swap-bases ) '"$OV"' --gun-trace "'"$OUT"'/m$1$2.jsonl" >/dev/null 2>&1' sh
python3 -I tools/gun_funnel.py "$OUT"/*.jsonl
