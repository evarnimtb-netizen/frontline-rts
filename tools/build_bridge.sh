#!/usr/bin/env bash
# Builds the Godot bridge (crates/godot-bridge) in release mode and copies
# the library into godot/bin/, where godot/frontline.gdextension loads it.
#
#   tools/build_bridge.sh
#
# Close the Godot editor first on Windows (it locks the DLL it loaded).
# Linux / Steam Deck builds belong in the Steam Runtime (sniper) SDK
# container; a .so built on a newer-glibc host may not load on the Deck.
set -euo pipefail
cd "$(dirname "$0")/.."

CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}" cargo build -p godot-bridge --release
mkdir -p godot/bin
case "$(uname -s)" in
    Darwin) lib=libfrontline_godot.dylib ;;
    Linux) lib=libfrontline_godot.so ;;
    MINGW* | MSYS* | CYGWIN*) lib=frontline_godot.dll ;;
    *) echo "build_bridge: unsupported OS $(uname -s)" >&2; exit 1 ;;
esac
# Copy to a temp name and rename, so a running Godot never sees a half file.
cp "target/release/$lib" "godot/bin/.$lib.tmp"
mv -f "godot/bin/.$lib.tmp" "godot/bin/$lib"
echo "build_bridge: godot/bin/$lib"
