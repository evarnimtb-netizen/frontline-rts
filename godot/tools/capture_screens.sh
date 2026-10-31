#!/usr/bin/env bash
# Captures the S1/S1b presenter screenshots into docs/slices/screens/
# (windowed Godot; a window opens briefly for each shot). The match shots use
# the mock match source. The S1b HUD is shot at 1600x900 and 1280x800.
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
    local name="$1" scene="$2" setup="$3" after="$4" size="${5:-1600x900}"
    local args=(--path godot --resolution "$size")
    [[ -n "$scene" ]] && args+=("$scene")
    args+=(-- --capture "$OUT/$name.png" --capture_after "$after" --capture_size "$size")
    [[ -n "$setup" ]] && args+=(--capture_setup "$setup")
    "$GODOT_BIN" "${args[@]}"
}

shot s1_menu "" "" 2.0
shot s1_menu_settings "" settings 2.0
shot s1_hud_mock res://match/match_mock.tscn hud 3.0
shot s1b_hud_mock_1600 res://match/match_mock.tscn hud 3.0 1600x900
shot s1b_hud_mock_1280 res://match/match_mock.tscn hud 3.0 1280x800
shot s1b_throw_tool_mock res://match/match_mock.tscn throw 3.0
shot s1b_minimap_rotated_mock res://match/match_mock.tscn rotated 3.0
shot s1_dig_tool_mock res://match/match_mock.tscn dig 3.0
shot s1_pause_mock res://match/match_mock.tscn pause 3.0
shot s1_end_mock res://match/match_mock.tscn end 3.0
echo "capture_screens: $OUT"
