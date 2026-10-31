#!/usr/bin/env bash
# Plays a scripted skirmish against the AI (S1 "Done when" 4) and saves the
# s1_autoplay_*.png screenshots into docs/slices/screens/ (windowed Godot).
#
#   godot/tools/autoplay.sh [path/to/Godot] [game minutes] [sim speed]
#
# GODOT defaults to $GODOT, then `godot` on PATH. Extra arguments after the
# third are passed to the game (e.g. --seed 3 --faction usa).
set -euo pipefail
cd "$(dirname "$0")/../.."
GODOT_BIN="${1:-${GODOT:-godot}}"
MINUTES="${2:-6}"
SPEED="${3:-3}"
shift $(( $# > 3 ? 3 : $# ))
"$GODOT_BIN" --path godot --resolution 1600x900 -- \
    --autoplay "$MINUTES" --autoplay_speed "$SPEED" \
    --autoplay_shots "$PWD/docs/slices/screens" "$@"
