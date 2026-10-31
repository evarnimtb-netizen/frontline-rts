# S1 sim results: Apple M3 (macOS ARM)

**Date:** 2026-10-31, 19:30–20:15 CET. **Commit:** S1 stage 5 on `s1-skirmish`.
**Status:** these runs check the sim side of the S1 "Done when" list
(`docs/slices/S1-skirmish.md`): item 2 (AI-vs-AI ends with a winner inside
30 game minutes), item 3 (`fortify-test`) and item 5 (tick P99 at peak under
10 ms). Item 1 (tests, clippy, fmt) is covered at the end.

## Machine

| | |
|---|---|
| CPU | Apple M3, 8 cores (4 performance + 4 efficiency) |
| RAM | 8 GB unified |
| OS | macOS 27.0, aarch64, on AC power |
| Toolchain | rustc 1.99.0 (Homebrew) |
| Build | `release` profile (`cargo build --release -p frontline-cli`) |
| Load | 1-minute load average 2.5–3.6 (a desktop session was running). Tail latencies are not from an idle machine. |

All runs use `--threads 3`. The JSON reports are written to
`bench/results/s1/`, which git ignores.

## Commands

```sh
F=target/release/frontline-cli
for s in 1 2 3 4 5; do
  $F skirmish --ai-vs-ai --seed $s --threads 3 --report json > bench/results/s1/skirmish-s$s.json
  $F skirmish --ai-vs-ai --seed $s --threads 3 --start-manpower 20000 \
     --report json > bench/results/s1/stress-s$s.json
done
$F skirmish --ai-vs-ai --seed 4 --threads 3 --minutes 20 --paced --report json   # paced, normal
$F skirmish --ai-vs-ai --seed 2 --threads 3 --minutes 10 --paced \
   --start-manpower 20000 --report json                                           # paced, stress
$F fortify-test --json > bench/results/s1/fortify.json
```

## Item 2: AI vs AI ends with a winner (Normal vs Normal)

Germany (player 0, south-west base) against USA (player 1, north-east base),
with the defs as committed. None needed tuning in this stage: the
`tickets_drain.per_min_per_point = 20` from stage 4 already ends every match.

| Seed | Winner | Length | Final hash | Tickets left (G / U) | Kills (G / U) | Deaths | Mortar bursts | Peak soldiers |
|---|---|---|---|---|---|---|---|---|
| 1 | Germany | 13:20 | `0x1fe3290e36a7933e` | 499 / 0 | 76 / 87 | 163 | 318 | 155 |
| 2 | Germany | 13:01 | `0xc2d4cd12e64cb868` | 500 / 0 | 78 / 84 | 162 | 305 | 155 |
| 3 | Germany | 17:20 | `0xce50eb0d3877bf14` | 461 / 0 | 103 / 131 | 234 | 561 | 164 |
| 4 | Germany | 25:44 | `0xd332a4a3f3be3817` | 238 / 0 | 208 / 225 | 433 | 1090 | 176 |
| 5 | Germany | 25:26 | `0x0bd59d297c9d3677` | 176 / 0 | 207 / 209 | 416 | 1155 | 170 |

Every match ends by ticket drain. The slowest finishes 4 min 16 s inside the
30-minute limit.

**Determinism of whole matches.** Seed 3 gives the same final hash
(`0xce50eb0d3877bf14`, tick 31,220) at 1, 2, 3 and 8 threads. It also gives
that hash when saved at tick 18,000 on 2 threads (a 2.1 MB save) and then
continued with `--load` on 8 threads.

### Balance finding (not fixed)

Player 0 at the south-west base wins far more often than chance:

| Setup | Seeds | SW base wins | NE base wins | No winner |
|---|---|---|---|---|
| Germany at SW, USA at NE | 1–12 | 10 (Germany, player 0) | 2 | 0 |
| USA at SW, Germany at NE | 1–12 | 9 (USA, player 0) | 3 | 0 |
| `--swap-bases`: Germany is player 0 but starts NE | 1–8 | 4 (USA, player 1) | 3 (Germany, player 0) | 1 |

- The faction makes no difference: whichever faction starts at the SW base
  wins 9 or 10 of 12.
- When the bases are swapped, the split is even. So the edge comes from the
  SW base and player slot 0 together.
- Capture points and woods are point-symmetric about the map centre. The
  valley (south only) and the noise octaves are not.
- Kills follow the winner only loosely. The winner often loses more men, and
  wins by holding 4 of 5 points.

**Suggested follow-up:** make the Kessel Ridge terrain symmetric (for example,
mirror the valley). Then check whether anything in the tick order favours
player 0, such as the plan stagger or command order. I left the map alone:
the combat tests stage their fights on specific open ground, so changing the
terrain would break their calibration.

## Item 3: `fortify-test`

Seeds 1–5, with each faction taking the trench in turn. For the rifles that
is 10 fights; for the mortar it is 20 barrages: 10 with the target in the
trench and 10 with it in the open.

| Experiment | Trench losses | Open losses | Ratio | Limit | Result |
|---|---|---|---|---|---|
| Rifle squad in a trench vs rifle squad in the open, 150 m | 0 | 80 | 0 % | ≤ 50 % | PASS |
| 10-round 81 mm barrage from 250 m, trench vs open | 28 | 74 | 37.8 % | ≤ 40 % | PASS |

- The trench rifle squad wins every fight without losing a man. Fights last
  34–85 s.
- The mortar result is only 2.2 points inside the limit. A barrage almost
  wipes out a squad in the open (7–8 of 8), so the ratio is set by
  near-direct hits on the trench. Any change to shell spread, lethal radius
  or the trench profile should re-run this test.

## Item 5: sim tick time at peak

**Measured:** the time of each `World::step` call, taken outside the sim.
**Peak:** every tick whose live soldier count is at least 90 % of the match's
highest count.

- **Normal** runs are the matches above, where the peak is 155–176 soldiers.
- **Stress** runs use `--start-manpower 20000`. Both AIs then buy up to the
  120 pop cap, so the peak is 238–239 soldiers: the spec's "up to 240".

| Run | Peak soldiers | Ticks at peak | P99, all ticks (ms) | P99 at peak (ms) | Max at peak (ms) |
|---|---|---|---|---|---|
| Normal, seeds 1–5, unpaced | 155–176 | 2,602–13,524 | 0.41–0.55 | 0.28–0.59 | 0.38–0.94 |
| Stress, seeds 1–5, unpaced | 238–239 | 20,448–40,172 | 0.69–0.96 | 0.77–1.03 | 1.33–11.2 |
| Normal, seed 4, **paced** 30 Hz, 20 min | 176 | 4,305 | 3.59 | **3.17** | 4.88 |
| Stress, seed 2, **paced** 30 Hz, 10 min | 238 | 11,584 | 4.12–4.53 | **4.13–4.52** | 26.8–35.3 |

The paced stress run was done twice (once for text output, once for JSON),
so its rows show a range.

**Result:** P99 at peak is under 10 ms in every run. The worst case is
4.5 ms, paced, at 238 soldiers. Item 5 passes on the M3.

- **Paced vs unpaced.** Paced ticks are 3–5 times slower than unpaced ones,
  as in M0. Between ticks the cores idle and clock down, so each tick starts
  cold.
- **Spikes.** Each paced run has a few spikes of 27–43 ms: 2–3 ticks over the
  33 ms period out of 18,000–36,000. They show up in the max, not the P99.
  Unpaced runs show single spikes of 8–14 ms.
- **Likely cause of the spikes:** the desktop load and the scheduler. They
  happen at random ticks, and no stage is to blame (`world.stage_times()` is
  available to check this).

## Item 1: tests

`cargo test --workspace --release` passes, and so do
`cargo clippy --workspace --all-targets -- -D warnings` and
`cargo fmt --all --check`.

AI-vs-AI determinism is tested in two places:

- **`crates/sim/tests/ai.rs`:** 5 game minutes give the same hashes at 1, 2,
  3 and 8 threads, checked against a full recompute along the way. A run saved
  mid-fight and continued through save → load gives the same hashes and the
  same final save bytes.
- **`sdk/crates/cli/tests/skirmish.rs`:** the same checks through
  `frontline-cli skirmish`. Over 5 minutes at 1, 2, 3 and 8 threads, and saved
  at tick 7,207 then continued on another thread count, the final hash is the
  same. The 5-seed `fortify-test` runs in release builds.
