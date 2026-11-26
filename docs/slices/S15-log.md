# S15 log (USSR: riders, plates, captured bombs, bayonets, matchups; at most 150 lines)
Spec: `docs/slices/S15-ussr.md`; stage `S15a-build-A.md`; targets `targets/S15.toml`; ablations `targets/S15-ablations/`.
Branch `claude/project-thread-aj1tww`, stage base `5202390`; check base `96d4b4b`. SAVE 34 / PROTOCOL 17.
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`,
`godot/match/*.gd.uid`. Do not push. Owner rule: no balance runs, no tuning; every number a TOML key with a range, switches off.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S15b Build C done (USSR guns, vehicles, rounds); the USSR data is complete; presenter and S15-ai next.** SAVE 34 /
PROTOCOL 17 (no layout change in C). Every S15 switch is 0 in `mods/ww2/defs/rules/skirmish.toml`; `ww2:ussr` is not in
`[matchups] factions`: it plays only through `docs/slices/targets/S15-smoke.toml` (USSR against Germany, switches on).
**T210** (`target/s15-C-off40`) and **T211** (`target/s15-C-40`): `neutral.py` "same 40/40" against `target/s14-F-40`.
**Smoke** (seeds 1-4 both bases, 8 matches): no panic, 10-26 min, Germany won 6 (no tuning).
**For the presenter.** New vehicle ids (all `ww2:ussr_*`): `t34_85`, `is2` (tanks), `su85` (tank destroyer, casemate), `su76m`
(assault gun, casemate `open = ["top","rear"]`), `ba64b` (recon car, no main gun: its DT is a pintle on the commander seat;
a `cupola` open hull-frame box), `m3a1_scout` (transport, 6 slots, M2HB and M1919 pintles), `us6` (truck, the guns' tow); guns
`45mm_m42`, `57mm_zis2`, `76mm_zis3`, `76mm_m1943`, `122mm_m30`. Zone names as S13 (`hull_*`, `superstructure_*`, `turret_*`,
`casemate_*`). Rider decks (`[[rider_slots]]`, exposed): T-34-85 8, IS-2 8, SU-85 6; no other vehicle has slots. The vehicle
count is 37 (`kills_by_vehicle` is 38 long).
**For S15-ai.** The USSR AI bought (8 matches): M-42 23, BA-64B 20, M1943 7, M-30 3, T-34-85 2, IS-2 1 and the infantry cards;
never the ZiS-2, ZiS-3, SU-85, SU-76M or M3A1 Scout Car (card weights as the plan: T-34-85 100, SU-85 100, SU-76M 50, M3A1 100,
BA-64B 5, IS-2 0 with `heavy_wave`/`ai_heavy`); the AI rides no riders yet (`AiRiders` jobs, switch off). Every gun card is towed
by `ussr_us6_squad`. "second_bases" 72 for the USSR (Germany 1) in the smoke: worth a look. Its mortar fire and buys otherwise
look like the German AI's.
**Found, not fixed (sim, not this stage's files):** `spotting.rs` vehicle observers compute `Fx::from(v.sight).mul_int(pct)
.div_int(100)`: `sight · pct` saturates Q16.16 above 327 m, so a vehicle sees at most about 327 m times the target's
`spot_size_pct`. Every shipped sight is 150-220 m, so nothing shipped is affected; W5's 800 m cases stage around it.
T212-T218 stay comments in `targets/S15.toml`. Details in "S15b C".

## S15a A (Build A)
Commits: ae409fd layout (SAVE 34); 1e1f16f keys, `[matchups]`, ablations, targets; 6545360 matchups M1-M4; 896a91e riders
R1-R6; b807ffb plates P1-P5; dfcff03 captured bombs X1-X3; a8f7860 bayonets B1; 756c5b8 rows W1-W4, JSON `s15`; 7913096 snapshot;
0439bb1 thread-count test; e391bb2 saturating supply counters (fuzz); then this log.
Tests added: s15_layout_roundtrips_and_hashes, load_rejects_bad_s15_state, load_rejects_bad_s15_keys, matchups_table_is_checked,
riders_board_a_tank, rider_hit_on_the_deck, riders_off_when_suppressed, riders_suppressed_by_a_burst, riders_off_when_tank_is_hit,
riders_off_when_gun_fires, riders_do_not_capture, riders_save_and_load_mid_exit, plate_stops_smg_from_the_front,
plate_does_not_stop_a_rifle, plate_does_not_cover_the_back, plate_does_not_cover_head_or_legs, plate_stops_a_light_fragment_only,
plate_stops_a_strike, flame_ignores_the_plate, plates_off_is_base, soviet_mortar_draws_german_bombs,
german_mortar_draws_no_soviet_bombs, own_bombs_first, foreign_bombs_fly_wider, foreign_off_is_base, bayonet_reaches_150_cm,
bayonet_misses_at_190_cm, bayonets_off_uses_the_butt, snapshot_shows_plates_and_deck,
s15_scene_is_deterministic_across_threads_and_save_load (1, 2, 3, 8 threads, save/load at the half, every S15 switch on); content:
validate_resolves_every_pair, matchup_refuses_a_faction_not_enabled, matchup_refuses_usa_ussr_without_free, free_allows_any_two,
default_opponent_is_first_pair, shipped_matchups_keep_the_default_pair, s15_off_setup_has_no_s15_field, rider_slots_are_checked,
member_plates_are_checked, foreign_feed_is_checked, bayonet_block_is_checked; cli: s15_rows_pass, s15_rows_feed_the_hard_checks,
s15_block_names_every_counter, menu_choice_without_opponent_is_base.
**Plan's unaudited points (risk 7, read in the code).** `Kit` layout: `Kit::rounds` is per feed slot of the squad type, so a foreign
bomb has no slot: `Kit::foreign` (u16) holds them, spent after the own bombs (X2). R3 on a moving tank: `rider_out` places the man
beside his slot from the vehicle's frame at that tick and needs no halt; the tank keeps its order (`riders_off_when_suppressed`).
Risk 4 (AT rifle on the S8a rule) is Build B's (`ptrd_engages_a_halftrack`); nothing in Build A depends on it.
**Deviations.** (1) Layout grew beyond the plan: `VehicleState::rider_off` (R3 timer) and `AiRiders` (S15-ai jobs, switch off).
(2) Rider box check is against the hull only (not turret). (3) Plates apply to soldiers in squads on foot, not riders' boxes (R4 hits
them as before) or vehicle crews. (4) Bayonet: the def is `WeaponMeleeDef { .., bayonet: Option<MeleeDef> }`; the setup keeps the
butt `MeleeDef` and a separate `WeaponSetup::bayonet`. (5) Setup layout grew three times after the layout commit
(`MemberSetup::plate`, `WeaponSetup::foreign_feed`, `WeaponSetup::bayonet`): `resolved_defs` BASELINE refreshed each time
(BLESS=1 for the snapshot). (6) New rule for W2: with `[riders] enabled` 1 an HE burst suppresses enemy riders on an exposed deck
within its suppression radius (linear falloff, no cover; riders are in no spatial hash, so bursts never reached them). (7) W2's
knock-out is staged (`edit_vehicle`, `burn` 1): 20 Panzerschreck rockets at the Pz IV hull never knocked it out, and one hit already
empties the deck by R3b. (8) W4 strikes 100 m from the spawn: a squadmate's dropped MG within the S4 pickup reach swapped the
striker's rifle mid-row. (9) Stand-in defs for the rows are made in code (`fortify/s15.rs`), not in `mods/ww2/tests/`. (10) R3b
fires on any non-small-arms armour blow. (11) T212-T218 are comments only; ablation files inherit the lists; `batch.sh`, `pool.py`
and the lobby FACTIONS list unchanged. (12) Save fuzz found an overflow in `supply6.accounts[].stock_in_g` (pre-S15 code): every
supply account counter now saturates. (13) T210 was run once (owner-approved set), before the bayonet commit; not rerun.
**Save fuzz** (e391bb2): 13146 mutations, 7688 rejected, 5458 loaded and ran 300 ticks, 0 then unloadable, 0 panicked.
**`tools/check.sh quick 96d4b4b`** at e391bb2: fmt ok, clippy ok, neutral.py ok, every ablate dry run ok (S15 included), tests
(cli, content, sim, godot-bridge) 1339 passed, 0 failed (the net crate is unchanged, so the IPv6 test did not run).

## S15a B (Build B)
Commits: f3025b4 faction, 13 cards, 6 crews, 10 weapons, items, cartridges, supply defs, AT-rifle check; 142af59 tests on the real
defs, tag rename, snapshot re-bless; then the smoke override and this log.
Tests added: sim `sn42_stops_a_ppsh_round_not_a_mosin_round`, `mosin_bayonet_reaches_150_not_190_cm` (real reach 180 cm: a man's
capsule adds to it, so the miss is tested at 2.4 m), `ptrd_engages_a_halftrack`, `ai_tolerates_an_empty_mix_slot`; content
`s15_defs_sort_last` (N1, switches 0 and 1: the old entries of the eight setup lists and every other setup field equal the setup
without the USSR defs), `only_the_assault_group_wears_a_plate`, `roks3_pair_off_is_no_flame_man`, `mosin_has_butt_and_bayonet_blocks`.
Existing tests changed for the new data: `s11a1_cards_load_at_the_spec_figures` (USSR roster skipped), `variants` (the Guards section
is veteran), `s15_off_setup_has_no_s15_field` (counts the stand-ins only), `s10a_cards_are_the_build_heavy_and_breach_cards`.
**Risk 4 (read in the test, not the rules):** a rifle-class crew weapon with a `penetration` table already picks vehicles by itself under
the S8a rule: the PTR team 80 m from a US half-track (its MGs spent) fires unordered and holes it within a minute; at 600 m it does
not fire. No rule or key was needed. The PTR card needed a content check change only: an `at` team needed a launcher.
**Figures verified (read 2026-11-26, Wikipedia):** PPSh-41 (35 / 71-round, 488 m/s, about 1,250 rpm cyclic), PPS-43 (35 rounds,
500-600 rpm, about 500 m/s), DP-27 (47 rounds, 550 rpm, 840 m/s), Mosin (5 rounds, about 865 m/s), PTRD (16 kg, 1,012 m/s, 40 mm at
100 m, 30 mm at 500 m: the note's 17.3 kg and 38/33/27 mm rows are replaced), Maxim (250 belt, 600 rpm, 62.66 kg, 740 m/s), SG-43
(250 belt, 500-700 rpm, 800 m/s, May 1943), TT-33 (8 rounds, 420 m/s), F-1 (600 g, 60 g TNT, fuse 3.5-4.5 s), 82-BM-37 (56 kg, 3.05 kg
bomb, 15-25 rpm, crew 4). **Unverified (kept, defs say so):** bayonet fixed when zeroed, RG-42 (page not found), RDG smoke, the UZ
charge and the Soviet satchel (US values), captured German 8 cm bombs, ROKS-3 igniter count, every cartridge weight, DP-27 and Maxim
burst lengths, all prices, cooldowns and `ai_weight` (S15 starting values, no tuning).
**Deviations.** (1) Tag `vehicle_riders` for `riders` (above). (2) `modset.rs`: AT-rifle teams. (3) `ai/fort.rs` test list. (4) 82-BM-37
lowest charge 70 m/s (the check wants a shortest range under `min_range_m` 90). (5) The PTRD table is 42 / 40 / 35 / 30 mm
(read 40 at 100 m and 30 at 500 m; the 0 and 300 m rows interpolate). (6) PPSh rate 600 (cyclic 1,250 read; the MP 40 is 400 for
about 500). (7) The sanitary and razvedchik cards copy the US aid and observer cards (pistol with 0 rounds; radio operator). (8)
The skirmish JSON has no `s15` block, so the smoke reads buys and rounds only. (9) 40 sets: two, as approved.

## S15b C (Build C)
Commits: bc1e96e 16 rounds, 9 weapons, 5 guns, US6, gun cards, cook-off round; cce0ee0 six vehicles (S13 boxes, `hull_body_boxes`,
racks, rare caps, rider slots), cards, recon validation; f4f7f5c W5 cases 47-56, rider boarding test; then this log.
Tests added: content `s15c_rounds_cards_and_tows`; sim `riders_board_each_soviet_tank_with_slots` (decks is2 8, su85 6, t34_85 8;
Pz IV and Sherman 0; riders off 0). Changed for the data: `s15_defs_sort_last` (every list has USSR entries), s13 V1/V5/V6,
hull-body, presenter-glacis and spot-point tests (`S15_TANKS`, `three_box`), `t151h` (9 classed rounds), `s11a1` counts (37),
`multi_box_components_list_every_box`; the s13 slope and heavy lists skip `ussr_`.
**Penetration convention.** The mod's tables are 0 degrees from the normal, 50 % criterion. Soviet tables (75 %, or 80 % for
"CP") give a "meet angle 90" column: it is taken as the 0-degree row, mm unchanged (no criterion factor was read, so these rows
sit a little low). The 85 mm rows are the D-5T page's calculated 0-degree, 50 % table (no conversion). Missing ranges are spaced
as the nearest read table and marked unverified.
**Verified (read 2026-11-26; each def's `sources`):** Wikipedia ZiS-2, M-42, ZiS-3, 52-K / D-5T, IS-2, T-34-85, SU-85, SU-76,
BA-64, M3 Scout Car, M-30, M1943 regimental gun, Studebaker US6 (calibres, masses, velocities, rates, crews, main armour);
losthistory.net BR-471 (CP 152/142/133/122 mm at 500-2000 m, 780 m/s) and the F-34 table for BR-350P; armytanks.org BR-240P
(1,070 m/s, 81 mm at 500 m). **Unverified (marked in the defs):** the M-42 BR-240 table (71/61/51/42), OF-462 mass and velocity,
every cartridge and complete-round weight, lower charges of the M1943 and M-30, rear and deck plates, the BA-64B nose angle,
DT stowage, US6 tow mass, rack splits, all prices, cooldowns and `ai_weight` (S15 starting values, no tuning).
**W5 (armour-test 47-56, 5 seeds both ways, report only; disabled / rounds / pens taken):** 47 Pz IV 10 of 10 by the T-34-85,
T-34 0; 48 T-34-85 10 of 10 by the Panther, Panther 0; 49 Panther flank 9 of 10, T-34 1; 50 IS-2 800 m: IS-2 10 of 10
(superstructure front 12 hits, 0 pens; hull front 10/10), Panther 0 (2 turret pens); 51 IS-2 7 of 10, Tiger 3 of 10; 52 Tiger 8
of 10 by the SU-85, SU-85 2; 53 Panther flank 10 of 10 by the ZiS-2; 54 M-42 out in 9 of 10, Pz IV 0 (hull front 94 hits, 8 pens);
55 Sd.Kfz. 251 6 of 10 by the PTRD, team lost 10 men; 56 PTRD on the Pz IV flank (skirts) 0 of 10, team lost 20.
**Smoke** (`S15-smoke.toml`, seeds 1-4 both bases): exit 0, no panic, every match ended (10.4-25.8 min), Germany 6, USSR 2;
buys above. **T210 / T211:** same 40/40 each. **check.sh quick 96d4b4b:** all ok (fmt, clippy after a collapsible-if fix in `s15_content.rs`, neutral.py, every ablate dry run; tests 1349 passed, 0 failed; `frontline_godot` failed under load, passed alone).
**Deviations.** (1) Nine classed AP / APCR rounds (`slope_class`), the plan says seven: every Soviet AP and APCR round is
classed. (2) Every gun card is towed by the US6, the M1943 and M-30 included. (3) The BA-64B has no main gun (content
`vehicle.rs`: a recon car may have 0 or 1; the setup check lets recon go without one). (4) The IS-2 carries the coax DT only.
(5) Edits outside the stage's files, all made for the data: sim tests `damage.rs`, `s13_tests.rs`, `s11a1_tests.rs`, content
`vehicle.rs`, `armour_test.rs` registration. (6) Global aggregates kept: SU-76M `ditch_block_dm` 19 and BA-64B 10 (the minimum
over tracked / wheeled defs), SU-85 pivot and muzzle shortened (reach 510 under the Panther's 570). (7) W5 800 m cases raise
`spot_size_pct` to 300 and the cannon range to 1 km (the saturation above). (8) W5's hit log holds shell hits only: PTRD holes
show in "disabled in".
