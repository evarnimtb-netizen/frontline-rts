#!/usr/bin/env bash
# Captures the S1 presenter screenshots into docs/slices/screens/ (windowed
# Godot; a window opens briefly for each shot). The match shots use the mock
# match source.
#
#   godot/tools/capture_screens.sh [path/to/Godot]
#
# GODOT defaults to $GODOT, then `godot` on PATH.
set -euo pipefail
cd "$(dirname "$0")/../.."
GODOT_BIN="${1:-${GODOT:-godot}}"
OUT="$PWD/docs/slices/screens"
mkdir -p "$OUT"

shot() {
    local name="$1" scene="$2" setup="$3" after="$4"
    local args=(--path godot --resolution 1600x900)
    [[ -n "$scene" ]] && args+=("$scene")
    args+=(-- --capture "$OUT/$name.png" --capture_after "$after")
    [[ -n "$setup" ]] && args+=(--capture_setup "$setup")
    "$GODOT_BIN" "${args[@]}"
}

shot s1_menu "" "" 2.0
shot s1_menu_settings "" settings 2.0
shot s1_hud_mock res://match/match.tscn hud 3.0
shot s1_dig_tool_mock res://match/match.tscn dig 3.0
shot s1_pause_mock res://match/match.tscn pause 3.0
shot s1_end_mock res://match/match.tscn end 3.0
echo "capture_screens: $OUT"
