#!/usr/bin/env bash
# Captures the S2 presenter screenshots on the mock net (MockNet; no
# network, no sockets) into docs/slices/screens/s2_mock_*.png, at 1600x900
# and 1280x800 (windowed Godot; a window opens briefly for each shot).
#
#   godot/tools/capture_net_screens.sh [path/to/Godot] [out_dir] [name_regex]
#
# GODOT defaults to $GODOT, then `godot` on PATH. name_regex limits the
# run to matching shots (e.g. "desync").
set -euo pipefail
cd "$(dirname "$0")/../.."
GODOT_BIN="${1:-${GODOT:-godot}}"
OUT="${2:-$PWD/docs/slices/screens}"
ONLY="${3:-}"
mkdir -p "$OUT"

shot() {
    local name="$1" scene="$2" setup="$3" size="$4"
    if [[ -n "$ONLY" && ! "$name" =~ $ONLY ]]; then
        return 0
    fi
    local args=(--path godot --resolution "$size")
    [[ -n "$scene" ]] && args+=("$scene")
    args+=(-- --capture "$OUT/$name.png" --capture_after 2.5 --capture_size "$size" --capture_setup "$setup")
    "$GODOT_BIN" "${args[@]}" >/dev/null 2>&1 || echo "capture failed: $name" >&2
}

for size in 1600x900 1280x800; do
    tag="${size%%x*}"
    shot "s2_mock_menu_$tag" "" mp "$size"
    shot "s2_mock_menu_errors_$tag" "" mp_errors "$size"
    shot "s2_mock_lobby_$tag" "" lobby "$size"
    shot "s2_mock_lobby_guest_$tag" "" lobby_guest "$size"
    shot "s2_mock_lobby_same_faction_$tag" "" lobby_same_faction "$size"
    shot "s2_mock_countdown_$tag" "" countdown "$size"
    shot "s2_mock_lobby_disconnected_$tag" "" mp_disconnected "$size"
    shot "s2_mock_readout_$tag" res://match/match_mock.tscn net_ok "$size"
    shot "s2_mock_pending_$tag" res://match/match_mock.tscn net_pending "$size"
    shot "s2_mock_unstable_$tag" res://match/match_mock.tscn net_unstable "$size"
    shot "s2_mock_desync_$tag" res://match/match_mock.tscn net_desync "$size"
    shot "s2_mock_waiting_$tag" res://match/match_mock.tscn net_waiting "$size"
    shot "s2_mock_paused_$tag" res://match/match_mock.tscn net_paused_by_them "$size"
    shot "s2_mock_disconnect_$tag" res://match/match_mock.tscn net_disconnect "$size"
done
echo "capture_net_screens: $OUT"
