# M0 Godot render spike: Apple M3 (macOS ARM)

**Date:** 2026-10-31, runs between 16:58 and 17:46 CET (from `date`).
**Status:** an M3 **baseline only**. The M0 render criteria are written for
the Steam Deck (Battalion, Deck preset) and a GTX 1660-class PC (Regiment,
1080p). This machine is neither, so nothing here passes or fails a gate.

## Machine and software

| | |
|---|---|
| CPU / GPU | Apple M3, 8 cores (4 performance + 4 efficiency), integrated 10-core GPU (`Apple M3 (Apple9)`) |
| RAM | 8 GB unified |
| Display | built-in, 60 Hz (`screen_get_refresh_rate`), HiDPI scale 2.0 |
| OS | macOS 27.0.1, aarch64 |
| Godot | 4.7.2.stable.official.ed1daf0bf, Metal 4.0 driver (default), standard build |
| Bridge | `godot` crate =0.5.5, feature `api-4-7`; `godot-bridge` built `--release` (rustc 1.99.0) |
| Sim | `frontline-sim` on its own thread, 3 rayon workers, 30 Hz, map 1,024 m, spawn area 200 m, seed 1, projectile cap 5,000 |
| Scene | `godot/main.tscn` via `godot/bench.tscn`: terrain (131k triangles, no shadows), soldiers (3 LODs: 1,060 / 372 / 208 triangles), placeholder tanks (not simulated), tracers, 6 smoke emitters x 48 particles, 1 shadowed sun (2 splits, 150 m, 4096 atlas) |

## Commands

```sh
tools/build_bridge.sh                                # cargo build -p godot-bridge --release, copy to godot/bin/
CARGO_BUILD_JOBS=4 cargo test --workspace --release
$GODOT --headless --path godot --quit-after 150      # clean load check

# Main matrix: Regiment / Battalion x Forward+ / Mobile x 1920x1080 / 1280x800,
# windowed, --disable-vsync at startup, 5 s warm-up + 20 s recorded,
# renderers interleaved, 15-20 s cool-down between runs.
tools/render_bench.sh 20                    # r1 (17:03-17:12); repeated as r2, TAG=r3
VIEW=close RESOLUTIONS=1920x1080 \
  CASES="regiment 2000 300 forward_plus,regiment 2000 300 mobile,battalion 700 100 forward_plus" \
  tools/render_bench.sh 20                  # camera 18 m up, -30 deg: LOD0/1/2 mix
PRESET=deck tools/render_bench.sh 20        # Battalion, 1280x800, FSR1 0.75, 30 fps, LOD2 200 m,
                                            # Godot worker pool 2 (temporary override.cfg)
DRIVER=vulkan tools/render_bench.sh 20      # MoltenVK, to try to get GPU timings (rejected, see below)
```

Each run is one Godot process: `Godot --path godot --rendering-method <r>
--resolution <WxH> --disable-vsync res://bench.tscn -- --soldiers N
--vehicles N --seconds 20 --view <v> [--preset deck] --out <json>`.
JSON output is in `bench/results/render-*.json` (git-ignored scratch).

## Results

Frame time is wall time between `_process` calls. **GPU ms: not available
on this machine** (Metal reports 0; MoltenVK reports one stale value per run,
see Limitations). "Sim step" is the `World::step` wall time on the sim thread
during the recorded window. "Casters" are the shadow-only caster instances
(soldiers + tanks). Draw calls and primitives are Godot's
`RENDER_TOTAL_*_IN_FRAME` monitors (they include the shadow passes).
All runs: vsync disabled (recorded), 60 Hz display, no display lock unless noted.

### Uncapped, RTS camera (85 m up, -50 deg), run r1

The cleanest uncapped set. Host load was not recorded for r1 (the
load column was added afterwards); the 1-minute load average was ~21 by 17:21,
so r1 may already be partly affected. The r1 JSON files were overwritten by
the r2 repeat; the numbers below are from the r1 run log.

| Case | Res | Renderer | fps | Frame mean / p50 / P95 / P99 (ms) | Sim step mean / P99 (ms) | Instances drawn | Soldiers LOD0/1/2 | Casters | Draw calls | Primitives | Presenter CPU |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Regiment 2,000 + 300 | 1920x1080 | Forward+ | 106.4 | 9.40 / 6.96 / 17.02 / 17.53 | 1.80 / 4.11 | 1,785 | 0 / 0 / 1,446 | 1,195 + 179 | 14 | 915k | 0.47 ms |
| Regiment 2,000 + 300 | 1920x1080 | Mobile | 97.4 | 10.27 / 6.95 / 17.37 / 19.98 | 1.97 / 11.83 | 1,791 | 0 / 0 / 1,448 | 1,199 + 179 | 14 | 914k | 0.39 ms |
| Battalion 700 + 100 | 1920x1080 | Forward+ | 143.8 | 6.95 / 6.89 / 7.45 / 7.71 | 1.11 / 7.24 | 614 | 0 / 0 / 503 | 402 + 57 | 14 | 397k | 0.25 ms |
| Battalion 700 + 100 | 1920x1080 | Mobile | 144.2 | 6.93 / 6.90 / 7.43 / 7.68 | 1.20 / 17.20 | 614 | 0 / 0 / 503 | 402 + 57 | 14 | 397k | 0.21 ms |
| Regiment 2,000 + 300 | 1280x800 | Forward+ | 108.8 | 9.19 / 6.93 / 16.97 / 19.15 | 2.04 / 10.91 | 1,705 | 0 / 0 / 1,383 | 1,133 + 171 | 14 | 880k | 0.43 ms |
| Regiment 2,000 + 300 | 1280x800 | Mobile | 119.6 | 8.36 / 6.91 / 16.76 / 21.28 | 2.56 / 18.89 | 1,706 | 0 / 0 / 1,384 | 1,134 + 171 | 14 | 882k | 0.28 ms |
| Battalion 700 + 100 | 1280x800 | Forward+ | 144.5 | 6.92 / 6.90 / 7.48 / 7.75 | 0.78 / 1.63 | 592 | 0 / 0 / 487 | 386 + 53 | 14 | 389k | 0.24 ms |
| Battalion 700 + 100 | 1280x800 | Mobile | 128.6 | 7.77 / 6.91 / 16.49 / 19.35 | 1.10 / 2.85 | 590 | 0 / 0 / 486 | 384 + 53 | 14 | 389k | 0.26 ms |

Tracers drawn: about 45 (Battalion) and 130 (Regiment) of 450 / 1,290 live
projectiles. Upload per frame (all MultiMeshes, capacity-sized): about
106 KB (Battalion) and 300 KB (Regiment).

### Close camera (18 m up, -30 deg), LOD mix, 1920x1080 (host load ~5)

| Case | Renderer | fps | Frame mean / p50 / P95 / P99 (ms) | Sim step mean / P99 (ms) | Instances | Soldiers LOD0/1/2 | Casters | Draw calls | Primitives | Upload |
|---|---|---|---|---|---|---|---|---|---|---|
| Regiment | Forward+ | 143.2 | 6.98 / 6.89 / 7.51 / 7.73 | 1.41 / 6.57 | 792 | 17 / 214 / 411 | 754 + 113 | 13 | 903k | 164 KB |
| Regiment | Mobile | 144.5 | 6.92 / 6.89 / 7.36 / 7.66 | 0.95 / 2.82 | 793 | 17 / 214 / 411 | 754 + 113 | 11 | 902k | 164 KB |
| Battalion | Forward+ | 144.7 | 6.91 / 6.90 / 7.33 / 7.64 | 0.49 / 1.20 | 285 | 8 / 75 / 155 | 274 + 34 | 13 | 401k | 65 KB |

### Deck presenter profile on the M3 (`--preset deck`, 1280x800, host load ~5)

FSR1 at 0.75 render scale, 30 fps lock, LOD2 to 200 m, Godot worker pool 2
(recorded `worker_pool_max_threads: 2`), Battalion caps.

| Renderer | fps | Frame mean / P95 / P99 / max (ms) | Sim step mean / P99 (ms) | Late ticks | Instances | Casters | Draw calls | Primitives | Presenter CPU mean / P99 | GPU busy |
|---|---|---|---|---|---|---|---|---|---|---|
| Forward+ | 30.0 | 33.33 / 33.93 / 34.22 / 34.34 | 0.52 / 1.58 | 0 | 561 | 386 + 53 | 14 | 385k | 0.16 / 0.48 ms | n/a |
| Mobile | 30.0 | 33.33 / 34.11 / 34.30 / 39.10 | 0.58 / 1.58 | 0 | 559 | 386 + 53 | 14 | 389k | 0.19 / 0.54 ms | n/a |

### Excluded runs

- **r2 and r3** (repeats of the main matrix, 17:27-17:46): host 1-minute
  load 15-33 and `kernel_task` at 40-125 % CPU (thermal pressure on the
  fanless M3), with screen capture (`replayd`) and other agents' headless
  Chrome running. Sim step P99 rose to 52-523 ms, and 10 of the 16 runs
  locked to the 60 Hz display with vsync recorded as disabled (flagged
  `valid_for_cost_comparison: false` by the new check). Kept as
  `render-*-r2.json` / `render-*-r3.json`; not used.
- **MoltenVK (`DRIVER=vulkan`)**: the viewport GPU timer returns one value
  for the whole run (1 distinct value in 552 frames; mean GPU ms above the
  frame interval), so it is stale. Rejected by the new bench check. It did
  confirm the timestamp unit: the RenderingDevice timestamp span divided by
  1e6 matched the viewport GPU time exactly (11.44 vs 11.44 ms, 11.98 vs
  11.98 ms).

## Reading the numbers

- **Pacing floor.** Every uncapped run has p50 ≈ 6.9 ms (~144 fps) whatever
  the load: Battalion, Regiment, close view, both renderers and both
  resolutions. Below that floor the frame time does not track the work, so
  Battalion and the close-view cases only show "at or under ~6.9 ms" on this
  machine. Regiment frames are bimodal (most at ~6.9 ms, a tail at ~17 ms),
  which gives the mean of 9.2-10.3 ms. P95 and P99 sit at about 17-21 ms.
- **Forward+ vs Mobile.** On CPU-side frame time the two are within noise
  (Regiment 1080p mean 9.40 vs 10.27 ms, 800p 9.19 vs 8.36 ms; Battalion
  equal at the floor). This is **not** the gate 7 comparison, which asks for
  GPU ms, and GPU ms cannot be measured here.
- **Resolution** barely changes anything (1080p vs 800p), which also says the
  M3 is not pixel-bound in this scene.
- **Presenter CPU** (Rust `_process`: snapshot swap, interpolation, culling,
  packing, uploads) is 0.2-0.5 ms per frame. Sim step stays well inside the
  33 ms tick when the host is quiet (P99 1-11 ms).

## Against the M0 wording (M3 baseline, not the target hardware)

| M0 criterion | Target | On this M3 |
|---|---|---|
| §2.2 gate 7: bone-texture skinning on a skeleton-less MultiMesh in Godot 4.7.2 | works | **Works** on Forward+ and Mobile (Metal); shaders also compile on the headless dummy driver. Checked visually from screenshots. |
| §2.2 gate 7: measure Forward+ vs Mobile GPU ms | GPU ms | **Not measured**: no usable GPU timer on Metal or MoltenVK in Godot 4.7.2. CPU-side frame times are within noise. |
| §15 M0: Battalion caps at 30 fps on Deck (Deck preset, GPU ≤ 60 % busy) | Steam Deck | With the Deck preset on the M3: holds 30 fps (P99 34.2 ms, max 34.3 ms on Forward+), sim P99 1.6 ms, presenter 0.16 ms. GPU busy % unknown. **Says nothing about the Deck.** |
| §15 M0: Regiment caps at 60 fps at 1080p on GTX 1660 class | GTX 1660 | Uncapped mean 9.4 ms (106 fps) on Forward+, but P95 17.0 ms and P99 17.5 ms sit right at the 16.7 ms budget, because of the bimodal pacing. **Not a GTX 1660 result.** |
| §15 M0: units + 1,500 tracers + smoke rendered from sim snapshots while the sim runs | 1,500 tracers | Sim runs live on its own thread with smoke. Up to ~1,300 live projectiles, but only ~130 tracers on screen. The 1,500-tracer load is **not reached**. |

## Limitations

- **Not the target hardware.** Apple M3 with 8 GB of RAM and Metal; not a
  Steam Deck (RDNA2, Vulkan, SteamOS) or a GTX 1660 (Vulkan/D3D12).
- **No GPU timings** (see above). GPU-busy %, which gate 3 and the Deck M0
  criterion test, has to be measured on Vulkan/D3D12.
- **Noisy host.** Shared desktop machine, fanless and thermally limited.
  Other processes (other agents, screen capture, photo analysis) ran during
  the session. Only the r1, close-view and Deck-preset sets are used. Each
  number is a single 20 s run, not a median of repeats.
- **Placeholder content.**
  - The soldier is a procedural mesh with a 2-clip (walk/idle) baked rig.
  - The tanks are bridge-driven boxes on circles. They are not simulated and have no turret or track animation.
  - There are no real assets, textures or materials (vertex colour only).
  - Terrain is one static mesh, not the CDLOD terrain.
- **Shadows are not tuned.**
  - There is one sun with 2 parallel splits, a 150 m shadow distance and a 4096 atlas.
  - Soldier casters use the LOD1 mesh.
  - Terrain casts no shadows.
  - Shadow cost on the Deck is unknown.
- **Missing pieces of §4.1 / §2.2:**
  - The near-camera `Skeleton3D` set (rule 4) is not built: LOD0 also uses the bone texture.
  - There are no impostors beyond the LOD2 distance.
  - There is no auto-degrade ladder.
  - The sim thread is not pinned and does not join its pool (it waits while the pool runs the tick).
  - There is no 40 fps OLED option.
- **Tracer count.** The bench view sees ~10 % of live rounds, so the
  1,500-tracer criterion needs a scenario with denser fire inside the view.
- **Frame time** is CPU wall time between `_process` calls. The macOS
  presentation path caps it at ~6.9 ms here and sometimes locks it to the
  display; runs that lock are flagged.
