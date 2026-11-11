# S10b Build E: the timber-and-earth bunker; all fortify rows

Read first: `S10b-obstacles.md` 4 (R14, R21-R24, R26), 5 "Bunker"; the log's top block. About 60 turns. No layout
change. The bunker is data plus the six-piece validator from A; the code work is geometry checks and the row.

## 1. The bunker (`mods/ww2/defs/fortifications/bunker.toml`, `fort.rs`)
* Point blueprint, `build_heavy` (R21 half rate for `build_structure`), pit `bunker_pit` (350 x 300 cm, 150 cm deep,
  `engineers_only`, all-round spoil, the S6 dig rate), then one build step: 250 kg, 540 s x 5 men (R22).
* Pieces (R23), `z_from = "base"` (from the undug surface), in the def's frame (+x = facing):
  rear wall `timber` 40 x 370 x 40 cm at x -190; side walls `timber` 420 x 40 x 40 cm at y +-165; front walls
  `timber` 40 x 145 x 40 cm at x +190, y +-112 (leaving the 80 cm slit at y 0); roof 420 x 370 x 80 cm at z 40.
  Walls hp 600, roof hp 1000, all armour 100 (`offset_cm` z is the box bottom above the undug surface).
* `bunkers_built += 1` when the build step completes. Spacing, cap and slope from the def; R18 applies.
* The slit is only geometry: bullets, shells and fragments pass the gap and the S10a rules do the rest; `burst`
  overpressure reaches a man only along a line clear of non-own pieces (S10a `piece_between`), so the roof shelters
  and the slit exposes. Own rounds pass own pieces (R26), so defenders fire from anywhere under the roof.

## 2. Tests (in `s10b_tests.rs`; each with its failing control)
* `bunker_digs_pit_then_builds_six_pieces`: Baupioniere with 250 kg: pit dug, six pieces at the R23 positions,
  `works_done` +1, `bunkers_built` 1. Control: a five-piece def -> count assert fails.
* `bunker_needs_build_heavy_or_half_rate_field`: rifle squad refused `CannotBuild`; US Engineers take 2x as long as
  a 100 % squad. Control: `heavy_by_field_pct` 100.
* `small_arms_do_not_damage_bunker`: 300 Kar98k and MG 42 rounds at the front: every piece keeps full hp. Control:
  armour 1 on the walls.
* `bunker_roof_survives_three_81mm`: three `drop_shell_now` 81 mm on the roof centre: roof hp 550, five men inside
  lose nobody. Control: roof hp 400 -> broken after three.
* `roof_satchel_kills_nobody_inside`: `drop_item_now` satchel on the roof: roof stands (700), no losses. Control:
  roof hp 250.
* `satchel_at_slit_reaches_men_inside`: a satchel at the slit's inner lip (slit centre, surface, 10 cm inside):
  at least two of five men die or go down in 10 of 10 seeds. Control: close the slit (front walls meet) -> none.
* `ap_shots_break_the_roof_after_seven`: seven non-bursting main-gun hits on the roof remove it, six do not.
  Control: `shot_piece_damage` 100.
* `bunker_saves_loads_and_hashes`: mid-build and built states roundtrip with equal hashes. Control: drop one
  piece after load -> hash differs.

## 3. fortify-test row `bunker` (`fortify/s10b.rs`) and the full fortify run
* Staging: `build_now` the bunker 150 m in front of an enemy MG team and a rifle squad; five men of a rifle squad
  inside the pit (the S10a `roof` row's helper).
* `small_arms_hp_lost` (the rifle squad and MG fire 60 s at the front: hp lost summed over pieces),
  `survived_81mm_x3_pct`, `sheltered_losses` (three bombs on the roof), `roof_satchel_losses`,
  `slit_satchel_losses_pct` (share of the five down or dead after the lip satchel), `ap_shots_to_break` (Pz IV H /
  M4A3 AP until the roof breaks, capped at 20), `mg_losses_pct` (60 s MG fire, share of the five lost), and
  `control.mg_losses_pct` (same men in the bare pit, no pieces).
* Run the whole `fortify-test --seeds 10 --json target/s10b-E/fortify10.json` in the background (`tools/run.sh`),
  then `frontline-cli targets docs/slices/targets/S10b.toml --from-json target/s10b-E --runs fortify10`: every
  T91-T93 row unchanged against `target/s10a-ai-verify/fortify10.json` (neutral.py does that comparison) and
  T102-T107 read. A hard miss is logged with its number; do not retune design values to pass (owner rule 1), only
  fix behaviour bugs; values marked verify may move only with a sentence in the log.

## 4. Ends when
Tests pass; `check.sh quick 8acc534` green; all six S10b rows filled; T102-T107 results in the log; 40-set
`neutral.py` same 40/40.

## 5. Hand-over
The T102-T107 table (pass, miss, values), verify values moved and why, anything F must draw (piece parts, the slit).
