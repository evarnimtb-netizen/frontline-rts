# S10b Build B: mines laid and set off

Read first: `S10b-obstacles.md` 2, 4 (R1-R7, R13, R18, R27), 5 "Laying" and "Mine burst"; the log's top block.
About 60 turns. No layout change (Build A's commit holds every field).

## 1. Laying (`fort.rs`)
* `Fortify` with a minefield blueprint: the S10a line path (`segments`, kg per segment, `work_s x work_men x 100 x
  tick_hz`), but a finished segment adds a `Mine { owner, fort, item: d.mine_item, pos: segment end, known: owner's
  team bits, boom_tick: None }` instead of pieces; `mines_laid += 1`. Cap counts standing mines plus the mines a
  running work still has to lay (the S10a line rule). R18 site check at the command (any vehicle hull within
  `vehicle_clear_m` of the line -> `BadSpot`).
* Ends (spec 5): last segment; builder re-ordered or dead (work removed via the S10a path; laid mines stay); next
  mine over the cap (work removed, `Cap` event).
* Cards (R27, `place_mines` only): `ger_engineer_squad`, `usa_engineer_squad`, `ger_baupioniergruppe`. The AI
  never orders a minefield (R29 guard from A). Laying rate is the tier's `build_pct`.

## 2. Triggering (`fort.rs` `take_mines`, called right after `take_charges` with the same `out`)
* Return at once when `mines` is empty. Mines in ascending id. AP (`trigger = "foot"`): living soldiers of another
  team, not riding, from the spatial index within `trigger_cm` (R3). AT (`"vehicle"`): vehicles of another team
  from `vderived.index`; hit when the mine is within `trigger_cm` of a track's centreline segment (its first to
  last anchor in the hull frame) or of any wheel anchor (R4); moving or towed vehicles alike, never wrecks.
* A set-off mine is removed and pushed as a `Thrown { item, owner, thrower: SimId(u32::MAX), landed: true, fuse: 0,
  pos: mine pos on the ground }` with an id from a key range disjoint from thrown ids and `CHARGE_KEYS` ranges;
  `mines_set_off += 1` for its owner. `boom_tick == tick` mines go the same way (C sets `boom_tick`).
* The items path bursts it: `ItemKind::Mine` behaves as a frag for `burst` (no trench `cut`, no inside rays);
  `blast_tracks` gives R5 for the AT mine. Count `vehicles_mined` on the mine owner's account
  when that burst takes a mobility part to 0, and `men_lost_to_mines` for the victim's player in `kill` when the
  kill comes from a mine item.
* R7: the "another team" filter is the only friendly rule; `burst` already spares own men.

## 3. Tests (in `s10b_tests.rs`; each with its failing control)
* `minefield_lays_one_mine_per_segment`: a 30 m AP line gives 15 mines at 2 m. Control: `segment_cm` 300 -> 10.
* `mine_cap_per_player`: the 61st AP mine is refused / ends the work. Control: limit 100.
* `mine_laying_ends_when_line_done_or_interrupted`: done -> work gone, `works_done` +1; re-ordered after 3 mines ->
  3 mines stand, work gone. Control: keep the work on re-order (fails the second half).
* `ap_mine_goes_off_on_contact_only`: a man walked past at 50 cm: none; at 20 cm: one burst. Control: `trigger_cm`
  60 -> the 50 cm pass sets it off.
* `ap_mine_kills_the_man_who_steps_on_it` (R6): he dies or is down in 10 of 10 seeds. Control: damage 10.
* `ap_mine_ignores_vehicles`, `at_mine_ignores_infantry`: a tank over an AP mine and a squad over an AT mine set off
  nothing. Control: swap the triggers in the item defs.
* `at_mine_immobilises_a_tank_over_it`: Pz IV H and M4A3 driven so the left track passes over one mine: track 0,
  `immobile`, `vehicles_mined` 1. Control: `vs_vehicle_pct` 10 -> the track survives.
* `own_side_never_sets_off_own_mines`: own squad and own tank cross own mines: nothing. Control: drop the team filter.
* `mine_trigger_is_deterministic`: two runs of a 300-tick crossing with 40 mines give equal hashes, also through
  save/load in the middle. Control: iterate mines through a `HashMap` (do not commit) -> differs.
* `site_refused_near_a_vehicle` (R18, minefield part). Control: `vehicle_clear_m` 0.

## 4. fortify-test rows (`fortify/s10b.rs`)
* `atmine`: per faction, the enemy medium tank (Pz IV H vs a US field, M4A3 vs a German field) on open ground.
  `direct_immobilised_pct` (one mine under the left track's path), `field_immobilised_pct` and
  `field_knocked_out_pct` (three AT rows of 30 m, 4 m apart, mines every 4 m, rows staggered by 4/3 m; the tank
  drives 120 m straight across, lateral offset `(seed * 37) % 400` cm), `infantry_triggered` (a rifle squad walks
  the same field). Log the geometric coverage estimate beside the measured field rate (T102b floor 75 is verify).
* `apmine`: `near_miss_triggered` (50 cm pass), `contact_triggered_pct` (20 cm), `vehicle_triggered` (a tank over
  a two-row AP field), `crossing_losses_pct` (a rifle squad across a two-row AP field of 30 m, mines every 2 m).

## 5. Ends when
Tests above pass; `tools/check.sh quick 8acc534` green; `frontline-cli fortify-test --seeds 10 --json ...` fills
`atmine` and `apmine`; T102, T103 read through `targets --runs fortify10`; 40-set `neutral.py` same 40/40.

## 6. Hand-over
T102/T103 numbers, the coverage estimate against T102b, the burst key range, anything C needs (`boom_tick` path).
