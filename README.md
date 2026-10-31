# Frontline RTS

A WW2-era tactical RTS in the "direct control + simulated ballistics" genre:
squad-scale battles, per-soldier inventories, crewed vehicles with real armour
values, destructible terrain, and the option to take direct control of any
single unit. Two goals drive the design:

1. **Performance**: thousands of simulated entities and projectiles at a stable
   frame rate (Battalion tier on a Steam Deck, Regiment tier on a 6-core desktop).
2. **Moddability**: humans *and AI agents* can add or change units, weapons,
   maps, missions and AI without recompiling the game.

The game is a deterministic Rust simulation core driven in lockstep, with a
Godot 4.7 presenter (an M0 render spike so far, `godot/`) and Luau scripting
for mods.

**Design doc:** [`SYSTEM_DESIGN.md`](SYSTEM_DESIGN.md). Start with §2.2 (battle
tiers and budgets), §5 (simulation core), §5.11 (determinism), §8.1 (CLI),
§13 (repo layout) and §15 (milestones). Current milestone: **M0**, the sim
skeleton and benchmark spike.

## Layout

```
Cargo.toml              workspace root (resolver 3, edition 2024)
rust-toolchain.toml     pinned toolchain (1.99.0)
deny.toml               cargo-deny: licence allowlist, bans, sources
crates/
  sim/                  deterministic core: Q16.16 fixed point, SoA ECS, tick
                        loop, seeded RNG streams, incremental state hash.
                        No Godot, no floats in sim state.
  simbench/             kernel (LOS ray, ORCA) and composed-tick benchmarks
                        for the M0 spike gates (§2.2)
  godot-bridge/         GDExtension (godot 0.5.5, api-4-7): sim thread,
                        snapshot -> batched MultiMesh, Rust-side culling,
                        procedural rigged soldier + bone-texture animation
godot/                  Godot 4.7.2 project: M0 render spike scene + bench
sdk/                    open-source SDK (MIT OR Apache-2.0, §7.9)
  crates/content/       def and scenario types, TOML loading, JSON Schema
  crates/cli/           frontline-cli: simulate, validate, schema, ... (§8.1)
bench/
  scenarios/            benchmark and determinism scenarios (battalion.toml, ...)
  results/              benchmark output (JSON results are git-ignored)
tools/                  helper scripts (licences, notices, hash comparison)
.github/workflows/      CI (ci.yml)
```

`crates/sim` and `crates/simbench` are proprietary; everything under `sdk/` is
MIT OR Apache-2.0 and will be mirrored to the public `frontline-sdk` repo.

## Build and test

Rust is pinned by `rust-toolchain.toml`; rustup installs it on first use.

```sh
cargo build --workspace
cargo test --workspace --release
```

Release mode matters for the tests: the determinism tests run long scenarios.
On machines with little RAM, cap parallel jobs: `CARGO_BUILD_JOBS=4 cargo build`.

## Run a headless simulation

```sh
cargo run --release -p frontline-cli -- simulate \
    --scenario bench/scenarios/battalion.toml \
    --ticks 1800 --seed 7 --threads 3 \
    --report bench/results/battalion.json
```

The report holds casualties, timings and the final state hash (`final_hash`).
For a fixed scenario, seed and tick count the hash must be identical on every
OS and for any `--threads` value; CI checks this (below). Run
`frontline-cli simulate --help` for all options.

## Run the benchmarks

```sh
cargo run --profile bench-release -p frontline-simbench -- --help
```

`bench-release` is the release profile with `codegen-units = 1` and thin LTO.
`simbench` runs the LOS-ray and ORCA kernels and the paced composed tick that
the §2.2 M0 gates measure. Results go to `bench/results/`.

## Run the Godot render spike (M0, §2.2 gate 7)

Needs Godot 4.7.2 (standard build). Build the extension (this also copies
it into `godot/bin/`, where `godot/frontline.gdextension` loads it), register
it once, then run the scene:

```sh
tools/build_bridge.sh                                    # cargo build --release + copy
GODOT=~/Applications/Godot.app/Contents/MacOS/Godot     # adjust per OS
$GODOT --headless --path godot --import                  # first time only
$GODOT --path godot                                      # windowed; WASD/QE/wheel/right-drag
$GODOT --path godot --rendering-method mobile            # Mobile renderer A/B
```

`FrontlineSim` (in `godot/main.tscn`) runs the sim on its own thread and draws
soldiers (3 LODs, skinned in the shader from a baked bone-matrix texture on a
skeleton-less MultiMesh), 300 placeholder tanks (bridge-driven, **not**
simulated) and tracers, with at most one `multimesh_set_buffer` per MultiMesh
per frame. Visible MultiMeshes draw without shadows; separate shadow-only
caster MultiMeshes hold the units whose shadow can land in view (needs the
`sun` property). Motion is interpolated between snapshots on their scheduled
tick times, one tick + `interp_margin_ms` behind real time. Its properties
(`soldiers`, `vehicles`, `projectile_cap`, `map_size_m`, `seed`,
`sim_threads`, `tick_hz`, LOD distances) are read when it enters the tree;
`get_stats()` returns sim and presenter statistics (call it a few times a
second at most) and `get_frame_counters()` cheap per-frame counters.

Benchmark (frame times, render CPU/GPU time, draw calls, LOD and caster
counts, sim step times to JSON):

```sh
$GODOT --path godot --resolution 1920x1080 --disable-vsync res://bench.tscn -- \
    --soldiers 2000 --vehicles 300 --seconds 20 --renderer forward_plus \
    --view rts --out "$PWD/bench/results/render.json"
tools/render_bench.sh               # Regiment + Battalion, Forward+ + Mobile, 1080p + 800p
VIEW=close tools/render_bench.sh    # camera near the battle (LOD0/1/2 mix)
PRESET=deck tools/render_bench.sh   # §2.2 Deck presenter profile (FSR1 0.75, 30 fps, 1280x800)
```

Run it windowed and in front (macOS throttles hidden windows); the bench
refuses a headless display unless `--allow_headless`. Each JSON records the
effective vsync mode, refresh rate, rendered resolution and host load, and
marks display-locked runs `valid_for_cost_comparison: false`. Godot 4.7.2 on
Metal reports 0 for GPU timings (and MoltenVK reports stale values), so on
macOS only CPU-side frame times are meaningful; GPU ms and GPU-busy % need
Vulkan/D3D12 on the target hardware.

## Licences and third-party notices

```sh
tools/deny.sh                                     # cargo deny check licenses bans sources
python3 -I tools/gen_third_party_notices.py       # writes THIRD_PARTY_NOTICES
```

`deny.toml` holds the licence allowlist (permissive licences plus MPL-2.0; no
GPL family). `tools/deny.sh` falls back to a licence-only scan when
`cargo-deny` is not installed. `THIRD_PARTY_NOTICES` is generated from
`cargo metadata` and the licence files in the local cargo registry; it is a
build output, so regenerate it instead of editing it.

## CI

`.github/workflows/ci.yml` runs on `ubuntu-latest`, `windows-latest` and
`macos-14` (Apple Silicon):

1. build and `cargo test --workspace --release`;
2. `frontline-cli simulate` on `bench/scenarios/battalion.toml` with seed 7 at
   1, 2, 3 and 8 sim workers, uploading each report JSON;
3. a final job downloads all 12 reports and runs `tools/compare_hashes.py`,
   which fails if any final hash differs across OS or thread count;
4. `cargo-deny` via the official action (advisories reported, not blocking),
   and a generated `THIRD_PARTY_NOTICES` artifact.

SteamOS native and SteamOS + Proton runs (§5.11, §15 M0) are **manual**, not
in CI: run the same `simulate` command on a Deck and compare those reports
with the CI ones using `python3 -I tools/compare_hashes.py <dir>`.
