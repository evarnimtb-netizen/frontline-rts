# M0 baseline: Apple M3 (macOS ARM)

**Date:** Sat Oct 31 16:00–16:03 CET 2026 (2026-10-31T15:00Z)
**Status:** baseline only. The M3 is one of the four §2.2 M0 spike machines, but
the gates are written against the Steam Deck (LCD at 15 W, OLED) and the
Ryzen 5 3600. Nothing here decides a gate.

## Machine

| | |
|---|---|
| CPU | Apple M3, 8 cores (4 performance + 4 efficiency), 8 logical CPUs |
| RAM | 8 GB unified |
| OS | macOS 27.0.1 (26A434), aarch64 |
| Power | AC attached |
| Toolchain | rustc 1.99.0 (b940084d7 2026-09-28, Homebrew), cargo 1.99.0 |
| Build | `release` for tests and `frontline-cli`; `bench-release` (codegen-units 1, thin LTO, sim overflow checks on) for `simbench` |
| Load | 1-min load average 4.6 → 2.9 during the runs (desktop session with a browser playing video). simbench flags load > 4 as "tail latencies not trustworthy" on the LOS run. |

## Commands

```sh
export CARGO_BUILD_JOBS=4
cargo build --workspace --release
cargo test  --workspace --release
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings

# Determinism: thread-count fuzz (900 ticks, seed 7)
for s in battalion regiment; do for t in 1 2 3 8; do
  target/release/frontline-cli simulate --scenario bench/scenarios/$s.toml \
    --ticks 900 --seed 7 --threads $t --report bench/results/m0-det/$s-t$t.json
done; python3 -I tools/compare_hashes.py bench/results/m0-det/$s-t*.json --expect 4; done

# Determinism: save at 300 (mid-barrage), load, continue to 900
target/release/frontline-cli simulate --scenario bench/scenarios/$s.toml --ticks 300 --seed 7 \
  --threads 3 --save-at 300 --save bench/results/m0-det/$s-300.sav --report .../$s-save300.json
target/release/frontline-cli simulate --scenario bench/scenarios/$s.toml \
  --load bench/results/m0-det/$s-300.sav --ticks 600 --threads 8 --report .../$s-load300-cont.json
target/release/frontline-cli simulate --load bench/results/m0-det/$s-300.sav \
  --ticks 600 --threads 1 --report .../$s-load300-cont-t1.json

# Benchmarks
cargo build --profile bench-release -p frontline-simbench
B=target/bench-release/simbench
$B --threads 3 --out bench/results/m0-los.json   los
$B --threads 3 --out bench/results/m0-orca.json  orca
$B --threads 3 --out bench/results/m0-orca-r4.json orca --neighbour-dist 4
$B --threads 3 --out bench/results/m0-tick-unpaced.json tick
$B --threads 3 --out bench/results/m0-tick-paced.json   tick --paced --seconds 30
# diagnostic (see "Paced vs unpaced")
$B --threads 1 --out bench/results/m0-tick-paced-t1-diag.json tick --tier battalion --paced --seconds 10
$B --threads 1 tick --tier battalion --seconds 10

# Licences
tools/deny.sh
python3 -I tools/gen_third_party_notices.py
```

The JSON reports sit next to this file (`bench/results/*.json` and
`bench/results/m0-det/`). They are scratch output; the save files were deleted
after the check.

## Build and test

- `cargo build --workspace --release`: OK, 0 warnings (dev build also 0 warnings).
- `cargo fmt --check`: clean. `cargo clippy --all-targets -D warnings`: clean.
- `cargo test --workspace --release`: **56 passed, 0 failed**.

| Suite | Passed |
|---|---|
| frontline-sim unit | 17 |
| frontline-sim `tests/determinism.rs` | 14 |
| frontline-simbench unit | 12 |
| frontline-cli `tests/simulate.rs` | 7 |
| frontline-content unit | 6 |

## Determinism

### Thread-count fuzz (900 ticks, seed 7)

| Scenario | 1 thread | 2 threads | 3 threads | 8 threads | Result |
|---|---|---|---|---|---|
| battalion (700 soldiers, pool 1,800, 3 × 3 km) | `0x13f18ba95f4cb919` | `0x13f18ba95f4cb919` | `0x13f18ba95f4cb919` | `0x13f18ba95f4cb919` | identical |
| regiment (2,000 soldiers, pool 5,000) | `0xadcab3a8f9f994d0` | `0xadcab3a8f9f994d0` | `0xadcab3a8f9f994d0` | `0xadcab3a8f9f994d0` | identical |

All 30 checkpoint hashes (every 30 ticks) are identical across the four thread
counts, not only the final hash. `tools/compare_hashes.py` passes per scenario.

### Save → load → continue (save at tick 300)

Tick 300 is mid-barrage: about 1,500 (battalion) and 4,200 (regiment) live projectiles.

| Scenario | Hash at save (tick 300) | Straight run at 300 | Continued to 900 (8 threads) | Continued, `--load` only (1 thread) | Straight run at 900 | Result |
|---|---|---|---|---|---|---|
| battalion | `0xc29b6c73116bc7b4` | `0xc29b6c73116bc7b4` | `0x13f18ba95f4cb919` | `0x13f18ba95f4cb919` | `0x13f18ba95f4cb919` | identical, all 20 checkpoints after load match |
| regiment | `0x8b734c3b30638ab1` | `0x8b734c3b30638ab1` | `0xadcab3a8f9f994d0` | `0xadcab3a8f9f994d0` | `0xadcab3a8f9f994d0` | identical, all 20 checkpoints after load match |

Save sizes: 1.8 MB (battalion), 3.3 MB (regiment), postcard.

### Not done here

Cross-OS hashes. The CI workflow (`.github/workflows/ci.yml`) compares Linux,
Windows and macOS-14 reports, but it has not run yet (no remote, nothing pushed).
SteamOS native and SteamOS + Proton are manual runs on a Deck.

## Kernels (3 threads)

### LOS ray (4,096 m map, 1 m cells, 300,000 rays of 300–600 m, median of 5)

| Variant | CPU µs/ray | Wall µs/ray | 1-thread µs/ray |
|---|---|---|---|
| linear | 0.613 | 0.204 | 0.628 |
| linear + max-mip | 0.416 | 0.139 | 0.411 |
| tiled16 | 0.544 | 0.181 | 0.569 |
| **tiled16 + max-mip** (fastest) | **0.404** | **0.135** | **0.404** |
| morton | 0.557 | 0.186 | 0.556 |
| morton + max-mip | 0.407 | 0.136 | 0.405 |

All variants give identical visibility results (25.9 % visible). Heightfield
only: no building-occluder BVH yet. The max-mip pyramid cuts cells visited
per ray from 181 to 1.6.

### ORCA (Q16.16, ≤ 8 neighbours from a 16 m hash, 900 ticks)

| Agents | Neighbour radius | CPU µs/agent | Wall µs/agent | 1-thread µs/agent | Tick ms | Mean neighbours | Deterministic |
|---|---|---|---|---|---|---|---|
| 700 | 8 m (pessimistic default) | 0.337 | 0.116 | 0.244 | 0.081 | 6.66 | yes |
| 2,000 | 8 m | 0.323 | 0.111 | 0.256 | 0.221 | 6.66 | yes |
| 700 | 4 m (what the M0 sim uses) | 0.194 | 0.068 | 0.135 | 0.047 | 2.70 | yes |
| 2,000 | 4 m | 0.188 | 0.066 | 0.124 | 0.132 | 2.69 | yes |

## Composed tick (3 threads, `World::step` + global ray budget)

Battalion: 700 soldiers, pool 1,800, 2,400 rays/tick, 512 m sim map.
Regiment: 2,000 soldiers, pool 5,000, 7,500 rays/tick, 832 m sim map.
150 warm-up ticks fill the projectile pool.

### Unpaced (back to back, 600 measured ticks)

| Tier | Mean | P50 | P95 | P99 | Max | Sim step mean | Ray budget mean |
|---|---|---|---|---|---|---|---|
| Battalion | 0.581 | 0.543 | 0.697 | 0.724 | 0.858 | 0.198 | 0.383 |
| Regiment | 1.779 | 1.649 | 2.090 | 2.131 | 2.264 | 0.630 | 1.148 |

### Paced at 30 Hz, 30 s measured (900 ticks)

| Tier | Mean | P50 | P95 | P99 | Max | Sim step mean | Ray budget mean | Overruns (> 33.3 ms) |
|---|---|---|---|---|---|---|---|---|
| Battalion | 3.104 | 3.113 | 4.310 | 6.118 | 30.716 | 1.107 | 1.997 | 0 |
| Regiment | 7.406 | 7.005 | 10.728 | 13.025 | 65.576 | 2.902 | 4.504 | 1 |

All values in ms. Stage means (paced): ballistics 0.71 / 1.89, movement 0.24 /
0.77, hash 0.04 / 0.08 (battalion / regiment). Perception, damage and status
are placeholders in the M0 sim (≈ 0 ms); there are no vehicles; there is no
late-game snapshot yet.

Regiment saturates its pool: 101,796 of 299,831 shots dropped in the paced run
(68,639 of 199,859 unpaced). Battalion drops none.

### Paced vs unpaced

Pacing makes every tick 4–5× slower on this machine, at any thread count:

| Battalion | 1 thread | 3 threads |
|---|---|---|
| Unpaced mean / P95 | 1.379 / 1.535 | 0.581 / 0.697 |
| Paced mean / P95 | 6.183 / 7.743 | 3.104 / 4.310 |
| Ratio (mean) | 4.5× | 5.3× |

Because the 1-thread run shows the same ratio, this is not rayon wake-up cost.
It is most likely macOS putting a thread that sleeps for most of each 33 ms
period on the efficiency cores or at a low clock. The sim thread has no QoS
class or pinning yet.

An earlier paced run today (load average 7.3, 600 ticks) measured Battalion
P95 5.23 ms and Regiment P95 11.15 ms. That is 21 % and 4 % above this run, so
run-to-run variance on a busy desktop is above the 10 % gate.

## §2.2 M0 gates

| # | Gate | Gate hardware and limit | Apple M3 result | Status |
|---|---|---|---|---|
| 1 | LOS kernel | Deck ≤ 2.9 µs/ray (3 threads, GPU-load scene, tiled + max-mip + occluder BVH) | 0.404 CPU µs/ray, tiled16 + max-mip, no BVH, no GPU load | Indicative only. Deck not measured, no occluder BVH. |
| 1 | ORCA kernel | Deck ≤ 0.9 µs/agent | 0.337 (700) / 0.323 (2,000) CPU µs/agent at 8 m radius | Indicative only. Deck not measured. |
| 2 | Paced 30 Hz composed tick | Deck Battalion P95 ≤ 13.5, P99 ≤ 18 ms; 3600 Regiment the same, 4 threads, with presenter | Battalion 4.31 / 6.12 ms; Regiment 10.73 / 13.03 ms (3 threads) | Indicative only. No Deck or 3600, no presenter, no late-game snapshot, placeholder perception/damage, no vehicles. |
| 3 | Power and concurrency | Deck ≥ 2.6 GHz, GPU ≤ 60 %, presenter ≤ 1.5 cores, inflation ≤ 30 % | Not measured | Outstanding: needs Deck and Godot presenter. |
| 4 | Spotting cadence | ≥ 99 % of pairs within 1.0 s, priority within 0.2 s | Not measured | Outstanding: perception system not built yet. |
| 5 | Memory | Tiled layout cuts L3 misses (`perf`); Deck Regiment ≤ 5.5 GB; 8 GB PC Battalion Low ≤ 4 GB | Not measured | Outstanding: needs Linux `perf`, Deck, 8 GB PC and presenter. |
| 6 | Determinism | Same hashes on SteamOS native, SteamOS + Proton, Windows, macOS ARM; 1/2/3/8 workers; save/load mid-barrage | macOS ARM: identical at 1/2/3/8 workers and through save at 300 → load → continue, both tiers | Partial: macOS ARM only. CI cross-OS run not done yet; SteamOS/Proton manual. |
| 7 | Rendering | Bone-texture skinning on skeleton-less MultiMesh in Godot 4.7.2; Forward+ vs Mobile GPU ms | Not started | Outstanding: Godot render spike not done (Godot not installed; godot-bridge not in this workflow). |
| 8 | Benchmark reliability | SCS variance < 10 % over 10 runs | 2 paced runs today: Battalion P95 differs by 21 % | Outstanding: needs 10 runs on an idle machine and the real SCS snapshot. |

**Still outstanding:** all Steam Deck LCD/OLED, Ryzen 5 3600, Windows and
SteamOS (native and Proton) measurements, and the Godot 4.7.2 render spike
(MultiMesh units, 1,500 tracers, smoke, presenter running alongside the sim).
The M3 numbers above leave room against the limits, but §2.2 scales M3 kernel
numbers to Zen 2 with ±40 % error, so they do not predict the Deck result.

## Licences

- `tools/deny.sh` (cargo-deny present): bans ok, licenses ok, sources ok.
- `THIRD_PARTY_NOTICES` regenerated: 57 crates, 27 unique licence texts.
