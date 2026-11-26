# S15 log (USSR: riders, plates, captured bombs, bayonets, matchups; at most 150 lines)
Spec: `docs/slices/S15-ussr.md`; stage `S15a-build-A.md`; targets `targets/S15.toml`; ablations `targets/S15-ablations/`.
Branch `claude/project-thread-aj1tww`, stage base `5202390`; check base `96d4b4b`. SAVE 34 / PROTOCOL 17.
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`,
`godot/match/*.gd.uid`. Do not push. Owner rule: no balance runs, no tuning; every number a TOML key with a range, switches off.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S15a Build B done (USSR infantry data); Build C (guns, vehicles) next.** SAVE 34 / PROTOCOL 17. Every S15 switch is 0 in
`mods/ww2/defs/rules/skirmish.toml`; `ww2:ussr` is a faction file but not in `[matchups] factions`, so it is played only through an
override (`docs/slices/targets/S15-smoke.toml`: the USSR enabled against Germany, switches on).
**T210** (`s15_off`, `target/s15-B-off40`) and **T211** (shipped defaults, `target/s15-B-40`): `neutral.py` "same 40/40" against
`target/s14-F-40`, both at the Build B data commits. **Smoke** (4 seeds Germany-USSR, S15-smoke.toml): no panic, 16-26 min, USSR won 3.
**What Build B made** (all `ww2:ussr_*`, all sorting after every older id; test `s15_defs_sort_last` compares the Germany-USA setup
with and without the USSR defs, switches off and on): faction file, 13 roster cards + 6 crews (tank crew 5; gun crews 45 / 57 / 76 /
76 M1943 / 122 mm), weapons `mosin_9130` (butt + bayonet), `ppsh41`, `pps43`, `dp27`, `maxim_1910`, `sg43` (no card), `tt33`, `ptrd41`,
`roks3`, `82mm_bm37` (foreign feed, German 8 cm bombs 130 %), cartridges, items (F-1, RG-42, RDG, satchel, UZ), 7 supply defs.
**Build C needs:** the five gun cards (`ussr_gun_crew_*` exist, `crew_squad` of the vehicle defs points at them), seven vehicles and
their rounds; the T-34-85 / SU-85 / IS-2 `[[rider_slots]]` and `[riders]` blocks (nothing carries them yet; the tag
`vehicle_riders` is on `ussr_otdelenie_avtomatchikov`); the supply defs' cook-off round is the German 7.5 cm HE until a Soviet HE
round exists (`ussr_*` supply, six files: change `cookoff`); the roster gets the gun and vehicle cards appended. T212-T218 still
comments in `targets/S15.toml`. The AI bought: 82 mm mortar, PTR team, MG section, rifle section (9 and 6), SMG section, sappers,
assault group; no vehicles (none exist), no medics, observers, HQ or bearers (as the German AI's smoke buys).
**Deviations** (details in "S15a B"): the plan's tag `riders` is `vehicle_riders` (a new tag shifts every later tag index: the tag list
is sorted); the AT-team check accepts a crew AT rifle with a `penetration` table (content `modset.rs`); the Soviet assault group
is an S10a-style card for the AI's fort habits (test list in `ai/fort.rs` updated); see below for the verify marks.

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
