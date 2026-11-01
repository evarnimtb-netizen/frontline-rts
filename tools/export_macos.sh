#!/usr/bin/env bash
# Builds the macOS zip a friend can run (S2 "Export"):
#
#   tools/export_macos.sh [path/to/Godot] [--allow-dirty] [--arm64-only] [--verify]
#
# 1. Builds the bridge in release for arm64 and x86_64 and merges them with
#    lipo (arm64 only if the x86_64 build fails or --arm64-only is given;
#    the script says so). The build id the relay checks (commit, -dirty,
#    protocol and save version; crates/net/build.rs) is stamped through
#    FRONTLINE_BUILD_COMMIT.
# 2. Exports godot/ with the "macOS" preset (godot/export_presets.cfg) to
#    build/export/Frontline.app. Needs the Godot 4.7.2 export templates
#    (Editor > Manage Export Templates); without them it stops here.
# 3. Copies mods/ww2 into Frontline.app/Contents/Resources/mods/ww2 (the
#    bridge looks there), signs the app ad hoc again (no Apple identity),
#    and zips build/Frontline-<commit>-macos.zip with README.txt.
# 4. --verify unzips into a fresh folder and starts the app once, which saves
#    a screenshot of the main menu (build/export/verify_menu.png) and quits.
#
# Both players need the same zip: the relay refuses a different build id.
# GODOT defaults to $GODOT, then ~/Applications/Godot.app, then `godot`.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"

GODOT_BIN="${GODOT:-}"
ALLOW_DIRTY=0
ARM64_ONLY=0
VERIFY=0
for a in "$@"; do
    case "$a" in
        --allow-dirty) ALLOW_DIRTY=1 ;;
        --arm64-only) ARM64_ONLY=1 ;;
        --verify) VERIFY=1 ;;
        -h | --help) sed -n '2,24p' "$0"; exit 0 ;;
        -*) echo "export_macos: unknown flag $a" >&2; exit 64 ;;
        *) GODOT_BIN="$a" ;;
    esac
done
if [[ -z "$GODOT_BIN" ]]; then
    if [[ -x "$HOME/Applications/Godot.app/Contents/MacOS/Godot" ]]; then
        GODOT_BIN="$HOME/Applications/Godot.app/Contents/MacOS/Godot"
    else
        GODOT_BIN="godot"
    fi
fi
[[ "$(uname -s)" == Darwin ]] || { echo "export_macos: macOS only" >&2; exit 1; }

# ---- build id ---------------------------------------------------------------
COMMIT="$(git rev-parse --short=12 HEAD)"
if ! git diff --quiet HEAD --; then
    if [[ $ALLOW_DIRTY -eq 0 ]]; then
        echo "export_macos: the work tree has uncommitted changes; commit them or pass --allow-dirty" >&2
        exit 1
    fi
    COMMIT="$COMMIT-dirty"
fi
export FRONTLINE_BUILD_COMMIT="$COMMIT"
echo "export_macos: build $COMMIT"

# ---- bridge: arm64 + x86_64 -----------------------------------------------------
# rustup's toolchain (rust-toolchain.toml) has the x86_64-apple-darwin std; a
# Homebrew cargo on PATH may not.
CARGO=cargo
if command -v rustup >/dev/null 2>&1 && rustup which cargo >/dev/null 2>&1; then
    CARGO="$(rustup which cargo)"
    RUSTC="$(rustup which rustc)"
    export RUSTC
fi
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
LIB=libfrontline_godot.dylib
"$CARGO" build -p godot-bridge --release --target aarch64-apple-darwin
ARCHS="arm64"
LIBS=("target/aarch64-apple-darwin/release/$LIB")
if [[ $ARM64_ONLY -eq 0 ]]; then
    if "$CARGO" build -p godot-bridge --release --target x86_64-apple-darwin; then
        ARCHS="arm64 x86_64"
        LIBS+=("target/x86_64-apple-darwin/release/$LIB")
    else
        echo "export_macos: WARNING: the x86_64 bridge did not build; the zip runs on Apple silicon only" >&2
    fi
fi
mkdir -p godot/bin build/export
lipo -create "${LIBS[@]}" -output "godot/bin/.$LIB.tmp"
mv -f "godot/bin/.$LIB.tmp" "godot/bin/$LIB"
echo "export_macos: bridge for $ARCHS: $(lipo -archs "godot/bin/$LIB")"

# ---- export -------------------------------------------------------------------
GV="$("$GODOT_BIN" --version 2>/dev/null | head -1)"   # e.g. 4.7.2.stable.official.ed1daf0bf
TPL_VER="$(echo "$GV" | cut -d. -f1-4)"                 # 4.7.2.stable
TPL_DIR="$HOME/Library/Application Support/Godot/export_templates/$TPL_VER"
if [[ ! -f "$TPL_DIR/macos.zip" ]]; then
    echo "export_macos: no export templates for Godot $TPL_VER in $TPL_DIR" >&2
    echo "  Install them from the editor (Editor > Manage Export Templates) or" >&2
    echo "  unpack Godot_v${TPL_VER%.stable}-stable_export_templates.tpz there. The bridge is built;" >&2
    echo "  rerun this script afterwards." >&2
    exit 2
fi
APP="$ROOT/build/export/Frontline.app"
rm -rf "$APP"
# The first headless run imports the project if it never was.
"$GODOT_BIN" --headless --path godot --import >/dev/null 2>&1 || true
"$GODOT_BIN" --headless --path godot --export-release "macOS" "$APP"
[[ -d "$APP" ]] || { echo "export_macos: Godot did not write $APP" >&2; exit 1; }

# ---- content, signature -------------------------------------------------------
mkdir -p "$APP/Contents/Resources/mods"
rm -rf "$APP/Contents/Resources/mods/ww2"
cp -R mods/ww2 "$APP/Contents/Resources/mods/ww2"
# Ad hoc: no Apple identity, so the friend needs the xattr step below.
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

# ---- zip --------------------------------------------------------------------------
STAGE="$ROOT/build/export/stage"
rm -rf "$STAGE"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/Frontline.app"
cp THIRD_PARTY_NOTICES "$STAGE/THIRD_PARTY_NOTICES.txt"
cp docs/net/playing-online.md "$STAGE/Playing online.txt"
cat > "$STAGE/README.txt" <<EOF
Frontline (build $COMMIT, for: $ARCHS)

1. Unzip this file (double-click it).
2. In Terminal, in the folder with Frontline.app, run:
       xattr -dr com.apple.quarantine Frontline.app
   The game is not signed by Apple; without this step macOS says it is
   damaged or cannot be checked.
3. Open Frontline.app.

Multiplayer: both players need this same zip (the game checks the build).
One player hosts (Multiplayer > Host, UDP port 47015 by default; macOS
asks once whether to accept incoming connections: allow it). The lobby
shows "Friends join: <address>:47015"; the other player picks
Multiplayer > Join and types that. Over the internet use Tailscale (both
install it; join the host's 100.x address) or a frontline-relay on a
server. "Playing online.txt" in this zip explains every option.

Desync reports are written to
~/Library/Application Support/Godot/app_userdata/Frontline/desync/
EOF
ZIP="$ROOT/build/Frontline-$COMMIT-macos.zip"
rm -f "$ZIP"
ditto -c -k --sequesterRsrc "$STAGE" "$ZIP"
echo "export_macos: $ZIP ($(du -h "$ZIP" | cut -f1))"

# ---- verify -------------------------------------------------------------------
if [[ $VERIFY -eq 1 ]]; then
    V="$(mktemp -d "${TMPDIR:-/tmp}/frontline-verify.XXXXXX")"
    ditto -x -k "$ZIP" "$V"
    xattr -dr com.apple.quarantine "$V/Frontline.app" 2>/dev/null || true
    SHOT="$ROOT/build/export/verify_menu.png"
    rm -f "$SHOT"
    "$V/Frontline.app/Contents/MacOS/Frontline" --resolution 1280x720 -- \
        --capture "$SHOT" --capture_after 3 || true
    if [[ -f "$SHOT" ]]; then
        echo "export_macos: verify OK, menu screenshot $SHOT (app in $V)"
    else
        echo "export_macos: verify FAILED: no menu screenshot (app in $V)" >&2
        exit 1
    fi
fi
