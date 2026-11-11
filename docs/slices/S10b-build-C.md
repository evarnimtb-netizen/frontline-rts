# S10b Build C: sweeping, lifting, mines set off by HE

Read first: `S10b-obstacles.md` 4 (R8-R13, R27), 5 "Clearing", 6 (`Order::ClearMines`); the log's top block.
About 60 turns. No layout change.

## 1. The command and order (`command.rs`, `fort.rs`, `squad.rs`)
* `ClearMines { squad, a, b }` accepted for an own `clear_mines` squad with a living man; `|b - a|` <=
  `lane_max_m` and both on the map, else `BadSpot`; no capability -> `CannotBuild`. Sets `Order::ClearMines { a, b,
  along: 0, lift: None, work: 0 }`; `sweeps += 1`.
* Sweepers: the two living members nearest `a` at the order (ascending `SimId` on ties), recomputed when one dies.
  They walk abreast at +-1 m from the lane centreline, their target `a + dir * along`, at `sweep_speed_pct` of
  their speed (the steering speed factor where `wire_speed_pct` and `carry_speed_pct` apply). `along` advances only
  while both (or the one left) are within 1 m of their spot and no `lift` is set. Other members hold at `a`.
* Detection (R8), every tick, per sweeper, mines in ascending id within `detect_cm` not yet known to his team:
  draw `MineDetect` keyed `(mine id, sweeper id, tick)` (a u32 `roll`); found when `roll * 100 * hz <
  detect_pct_per_s * 2^32` (u64, no float): per-tick p = `detect_pct_per_s / (100 * hz)`. Found: set the team's `known` bits, `mines_found += 1`, `lift = Some(id)` if none.
* Lifting (R10, R11): the nearest sweeper walks to 1 m from the mine (never within `trigger_cm + 50 cm` of a
  known mine on the way: his spot is clamped back along the lane), then adds his tier's `cut_pct` per tick (x0.3
  suppressed, 0 pinned) toward `lift_s x 100 x hz`; done: mine removed, `mines_lifted += 1`, next known mine in
  the lane or `lift = None`. Own mines in the lane are known and lifted too.
* Ends (spec 5): `along` at the lane length and no known mine within `lane_half_cm` of the lane; another order;
  no living sweeper. An unknown mine under a sweeper's foot still goes off (Build B trigger): that is the risk.
* Cards (R27, `clear_mines`): `ger_engineer_squad`, `usa_engineer_squad`, `ger_baupioniergruppe`,
  `ger_sturmpioniergruppe`, `usa_assault_section`. Check the AI never reads the new bit (A's grep list).

## 2. Mines set off by HE (R12, R13; `combat.rs` `burst`, `fort.rs`)
* At the end of `burst`, when `mines` is not empty: mines within the burst's `blast_radius` (2D distance, ascending
  id) with `terrain.line_of_sight(lift, mine pos + 10 cm)` get `boom_tick = Some(tick + 1)` unless already set;
  `mines_blasted += 1` on the mine owner when it bursts. Mine bursts chain this way one tick per hop (bounded by
  `MAX_MINES`). Applies to every HE: mortar, shell, grenade, satchel, Bangalore, mine.
* A mine that goes off (any cause) is known to all (it is removed; the snapshot event in F shows the burst).

## 3. Tests (in `s10b_tests.rs`; each with its failing control)
* `sweep_finds_at_expected_rate`: 400 single-mine passes at offsets 0..140 cm over fixed seeds; found share within
  3 points of the mean of `1 - (1 - p)^n` (per-tick p, n ticks in range, from the run). Control: `detect_pct_per_s` 25 in the sim only.
* `sweeper_stops_short_of_a_found_mine`: a found mine on the centreline: no sweeper comes within `trigger_cm + 50`
  cm before it is lifted; it never goes off. Control: drop the clamp -> it goes off in some seeds.
* `lift_time_follows_tier`: field 20 s, assault 10 s, construction 40 s (+-1 tick). Control: ignore the tier.
* `clear_mines_needs_capability`: a rifle squad is refused `CannotBuild`. Control: give it the bit.
* `clear_ends_at_lane_end_with_no_known_mine_left`: order back to idle, `along` = length, every mine in the lane
  lifted or unknown. Control: end at the lane end with a known mine left -> assert fails.
* `found_mine_known_to_finder_only`: after a find, the finder's team bit only; after it goes off, removed.
  Control: set all bits on find.
* `burst_sets_off_mines_next_tick`: an 81 mm on a 3-mine row: mines in its radius burst on tick + 1, the one
  beyond does not. Control: radius x 0.5.
* `bangalore_clears_mines_along_its_length`: a Bangalore across a 2-row AP field removes the mines within 150 cm
  of its bursts. Control: Bangalore laid 3 m short.
* `sweep_is_deterministic`: two runs and a mid-run save/load give equal hashes. Control: key the roll by tick only.

## 4. fortify-test row `sweep` (`fortify/s10b.rs`)
A US or German engineer squad (per faction) sweeps a 30 m lane with 10 enemy AP mines at offsets 0, 15, ... 135 cm
from the centreline. `found_pct`, `expected_pct` (`1 - (1 - p)^n` per mine, n its measured ticks in range), `found_vs_expected_pts`
= |found - expected|, `lifted_of_found_pct`, `sweeper_losses`; `bangalore_cleared_pct` from the Bangalore case.

## 5. Ends when
Tests pass; `check.sh quick 8acc534` green; `sweep` row filled, T104 read; 40-set `neutral.py` same 40/40 (bursts
now look for mines: the empty-list check must keep the AI matches identical).

## 6. Hand-over
T104 numbers, the detection formula as built, the tick-order point of the HE hook, anything for D.
