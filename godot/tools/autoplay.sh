#!/usr/bin/env bash
# Plays a scripted skirmish against the AI (S1 "Done when" 4, S1b "Done
# when" 3) and saves the s1_autoplay_*.png and s1b_*.png screenshots into
# docs/slices/screens/ (windowed Godot). The S1b frag-into-an-enemy-trench
# step needs the AI to have dug in where our men can reach, which can take
# a while: hence the longer default run.
#
#   godot/tools/autoplay.sh [path/to/Godot] [game minutes] [sim speed]
#
# GODOT defaults to $GODOT, then `godot` on PATH. Extra arguments after the
# third are passed to the game (e.g. --seed 3 --faction usa).
set -euo pipefail
cd "$(dirname "$0")/../.."
GODOT_BIN="${1:-${GODOT:-godot}}"
MINUTES="${2:-18}"
SPEED="${3:-4}"
shift $(( $# > 3 ? 3 : $# ))
"$GODOT_BIN" --path godot --resolution 1600x900 -- \
    --autoplay "$MINUTES" --autoplay_speed "$SPEED" \
    --autoplay_shots "$PWD/docs/slices/screens" "$@"
