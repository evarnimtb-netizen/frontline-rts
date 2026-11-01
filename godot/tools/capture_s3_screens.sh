#!/usr/bin/env bash
# Captures the S3 presenter screenshots on the mock source into
# docs/slices/screens/s3_mock_*.png (windowed Godot; a window opens briefly
# for each shot), at 1600x900 and 1280x800. The staging is in
# godot/match/match.gd (_stage_s3); the shots fire their tanks a moment
# before the capture so flashes, tracers and hit markers are in the picture.
#
#   godot/tools/capture_s3_screens.sh [path/to/Godot]
#
# GODOT defaults to $GODOT, then `godot` on PATH.
set -euo pipefail
cd "$(dirname "$0")/../.."
GODOT_BIN="${1:-${GODOT:-godot}}"
OUT="$PWD/docs/slices/screens"
mkdir -p "$OUT"

shot() {
    local name="$1" setup="$2" size="$3"
    "$GODOT_BIN" --path godot --resolution "$size" res://match/match_mock.tscn \
        -- --capture "$OUT/$name.png" --capture_after 3.0 --capture_size "$size" --capture_setup "$setup"
}

shot s3_mock_vehicles_1600 s3_vehicles 1600x900
shot s3_mock_vehicles_1280 s3_vehicles 1280x800
shot s3_mock_card_1600 s3_card 1600x900
shot s3_mock_card_1280 s3_card 1280x800
shot s3_mock_fit_1280 s3_fit 1280x800
shot s3_mock_callin_locked_1600 s3_locked 1600x900
shot s3_mock_callin_locked_1280 s3_locked 1280x800
shot s3_mock_escort_1600 s3_escort 1600x900
echo "capture_s3_screens: $OUT"
