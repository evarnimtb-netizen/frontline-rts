#!/usr/bin/env bash
# M0 render spike benchmark (SYSTEM_DESIGN.md §2.2 gate 7, §15 M0).
#
# Runs the Godot presenter + live sim windowed with vsync off (set at
# startup with --disable-vsync) and writes one JSON per run to
# bench/results/render-<case>-<renderer>-<WxH>[-<view>][-deck].json
# (git-ignored). Runs are interleaved (renderers and tiers alternate) with a
# cool-down between them, so thermal drift on a fanless laptop does not
# line up with one renderer.
#
#   tools/render_bench.sh                 # default matrix, 20 s per run
#   tools/render_bench.sh 30              # 30 s per run
#   RESOLUTIONS="1920x1080" tools/render_bench.sh
#   VIEW=close tools/render_bench.sh      # camera near the battle: LOD0/1 mix
#   PRESET=deck tools/render_bench.sh     # §2.2 Deck presenter profile:
#       Battalion, 1280x800, 0.75 scale + FSR1, 30 fps lock, LOD2 to 200 m,
#       Godot worker pool capped at 2 (temporary godot/override.cfg)
#   DRIVER=vulkan tools/render_bench.sh   # macOS: MoltenVK instead of Metal.
#       Godot 4.7.2's Metal driver reports 0 GPU time; under MoltenVK the
#       viewport GPU timer works, so this gives a GPU-ms proxy for the
#       Forward+ vs Mobile comparison (the frames are display-locked there).
#   TAG=r2 tools/render_bench.sh          # suffix the output names (repeats)
#   COOLDOWN=30 GODOT=/path/to/godot tools/render_bench.sh
#
# Default matrix, per resolution (1920x1080 and 1280x800):
#   Regiment  (2,000 + 300) Forward+, Regiment Mobile, Battalion (700 + 100)
#   Forward+, Battalion Mobile. CASES="regiment 2000 300 forward_plus,..."
#   overrides it.
#
# Keep the window visible and in front: macOS throttles occluded windows.
# Numbers from a non-target machine (e.g. an M3) are a baseline only; the
# gates are judged on the Deck and a GTX 1660-class PC.
set -euo pipefail
cd "$(dirname "$0")/.."

GODOT="${GODOT:-$HOME/Applications/Godot.app/Contents/MacOS/Godot}"
REC_SECONDS="${1:-20}"
COOLDOWN="${COOLDOWN:-15}"
VIEW="${VIEW:-rts}"
PRESET="${PRESET:-}"
DRIVER="${DRIVER:-}"

tools/build_bridge.sh
if [ ! -f godot/.godot/extension_list.cfg ]; then
    # First open registers the GDExtension (writes .godot/extension_list.cfg).
    "$GODOT" --headless --path godot --import || true
fi

extra=()
driver_args=()
cases=()
if [ -n "$DRIVER" ]; then
    driver_args=(--rendering-driver "$DRIVER")
fi
if [ "$PRESET" = "deck" ]; then
    RESOLUTIONS="${RESOLUTIONS:-1280x800}"
    cases=("battalion 700 100 forward_plus" "battalion 700 100 mobile")
    extra=(--preset deck)
    # Godot reads the worker-pool size at startup; project.godot only sets it
    # for exports tagged "deck".
    if [ -e godot/override.cfg ]; then
        echo "render_bench: godot/override.cfg already exists; refusing to replace it" >&2
        exit 1
    fi
    printf '[threading]\n\nworker_pool/max_threads=2\n' > godot/override.cfg
    trap 'rm -f godot/override.cfg' EXIT
else
    RESOLUTIONS="${RESOLUTIONS:-1920x1080 1280x800}"
    if [ -n "${CASES:-}" ]; then
        IFS=',' read -r -a cases <<<"$CASES"
    else
        cases=(
        "regiment 2000 300 forward_plus"
        "regiment 2000 300 mobile"
        "battalion 700 100 forward_plus"
        "battalion 700 100 mobile"
    )
    fi
fi

mkdir -p bench/results
first=1
for res in $RESOLUTIONS; do
    for c in "${cases[@]}"; do
        read -r name soldiers vehicles renderer <<<"$c"
        suffix="$res"
        [ "$VIEW" != "rts" ] && suffix="$suffix-$VIEW"
        [ -n "$PRESET" ] && suffix="$suffix-$PRESET"
        [ -n "$DRIVER" ] && suffix="$suffix-$DRIVER"
        [ -n "${TAG:-}" ] && suffix="$suffix-$TAG"
        out="$PWD/bench/results/render-$name-$renderer-$suffix.json"
        if [ $first -eq 0 ] && [ "$COOLDOWN" -gt 0 ]; then
            sleep "$COOLDOWN"
        fi
        first=0
        load_before="$(python3 -c 'import os; print(*os.getloadavg())')"
        "$GODOT" --path godot ${driver_args[@]+"${driver_args[@]}"} \
            --rendering-method "$renderer" --resolution "$res" \
            --disable-vsync res://bench.tscn -- \
            --soldiers "$soldiers" --vehicles "$vehicles" --seconds "$REC_SECONDS" \
            --renderer "$renderer" --view "$VIEW" ${extra[@]+"${extra[@]}"} --out "$out"
        # The window may come up smaller than asked (HiDPI, screen size):
        # check what was actually rendered.
        # Also record the host load: other work on the machine shows up in
        # both frame and sim step times.
        python3 - "$out" "$res" "$load_before" <<'PY'
import json, os, sys
d = json.load(open(sys.argv[1]))
d["host_loadavg_before"] = [float(x) for x in sys.argv[3].split()]
d["host_loadavg_after"] = list(os.getloadavg())
json.dump(d, open(sys.argv[1], "w"), indent=2)
want = [float(x) for x in sys.argv[2].split("x")]
got = d.get("resolution", [0, 0])
if [round(v) for v in got] != [round(v) for v in want]:
    print(f"render_bench: WARNING {sys.argv[1]}: rendered {got}, asked for {want}")
if not d.get("valid_for_cost_comparison", False):
    print(f"render_bench: NOTE {sys.argv[1]}: not valid for cost comparison: {d.get('notes')}")
PY
    done
done
