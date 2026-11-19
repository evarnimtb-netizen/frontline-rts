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
  crates/cli/           frontline-cli: simulate, skirmish, fortify-test,
                        validate, schema (§8.1)
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

## Run a headless skirmish (S1)

`skirmish` plays a whole match on Kessel Ridge (rules and content from
`mods/ww2`), AI against AI, and prints the winner, the match length, per-player
stats (tickets, manpower, kills, losses, squads fielded, point time, AI
commands) and tick timings for the whole match and for its peak:

```sh
cargo build --release -p frontline-cli
F=target/release/frontline-cli
$F skirmish --ai-vs-ai --seed 3                       # Germany (SW base) vs USA (NE base), Normal
$F skirmish --ai-vs-ai --seed 3 --difficulty hard --difficulty1 easy --minutes 20
$F skirmish --ai-vs-ai --seed 3 --report json         # full report on stdout
$F skirmish --ai-vs-ai --seed 3 --faction0 ww2:usa --faction1 ww2:germany
$F skirmish --ai-vs-ai --seed 3 --swap-bases          # player 0 starts at the NE base
```

It exits 0 when someone won and 3 on a draw or when `--minutes` (default 30)
ran out. `--threads` never changes the result. To check save → load →
continue on a whole match, save at an absolute tick and continue the save on
another thread count; the final hash must match the straight run:

```sh
$F skirmish --ai-vs-ai --seed 3 --threads 2 --save /tmp/s3.sav --save-at 18000
$F skirmish --load /tmp/s3.sav --threads 8
```

Timing: `--paced` runs at the real 30 Hz tick rate (a 20-minute match takes
20 minutes); `--start-manpower 20000` lets both AIs fill the 120-soldier pop
cap early, for a worst-case peak of about 240 soldiers (a different match, so
different hashes). Results on the M3 are in
[`bench/results/S1-sim-apple-m3.md`](bench/results/S1-sim-apple-m3.md).

`fortify-test` checks that trenches work as cover. Over seeds 1–5, with each
faction entrenched in turn, it stages a rifle squad in a finished trench
against a rifle squad in the open 150 m away (pass: the trench side's losses
are at most 50 % of the open side's) and a 10-round mortar barrage on a rifle
squad in a trench and in the open (pass: at most 40 %). It prints every run
and exits 0 only when both pass:

```sh
$F fortify-test                  # --seeds N --first-seed S --json
```

Tuning (S3b): every gameplay value is in `mods/ww2` TOML. One command
A/B-tests a change over 10 seeds × both base orders. `skirmish`,
`armour-test` and `fortify-test` take `--rules-override` (see
[`docs/modding/tuning.md`](docs/modding/tuning.md)):

```sh
$F skirmish --ai-vs-ai --seeds 10 --both-bases --summary --ab \
    --rules-override mods/ww2/tests/overrides/example.toml
```

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

SteamOS native and SteamOS + Proton (§5.11, §15 M0) are checked **by hand**,
not in CI, because they need a Deck. The same script also checks a Mac
locally. On that machine run:

```sh
tools/platform_check.sh            # build, tests, simulate at 1/2/3/8 workers
tools/platform_check.sh --quick    # the same without the slow debug sim tests
```

It runs the same commands as CI and fails if the four final hashes differ.
The release tests include the committed golden hash, so a pass already ties
that machine to the CI platforms. To compare with a CI run directly, download
the `sim-reports-*` artifacts of the run on the same commit, unzip them into
a folder and add `--ci-reports <folder>`.
