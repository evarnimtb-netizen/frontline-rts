# S15 log (USSR: riders, plates, captured bombs, bayonets, matchups; at most 150 lines)
Spec: `docs/slices/S15-ussr.md`; stage `S15a-build-A.md`; targets `targets/S15.toml`; ablations `targets/S15-ablations/`.
Branch `claude/project-thread-aj1tww`, stage base `5202390`; check base `96d4b4b`. SAVE 34 / PROTOCOL 17.
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`,
`godot/match/*.gd.uid`. Do not push. Owner rule: no balance runs, no tuning; every number a TOML key with a range, switches off.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S15a Build A done; Build B (USSR data) next.** SAVE 34 / PROTOCOL 17. Every S15 switch is 0 in
`mods/ww2/defs/rules/skirmish.toml` (`[riders] enabled`, `[body_armour] enabled`, `[supply] foreign_ammo`, `[melee] bayonets`,
`[ai.riders] jobs`); the setup drops every S15 def part while its switch is 0 (`MatchSetup::s15_off`, N2).
**T210:** `target/s15-A-40` vs `target/s14-F-40`: `neutral.py` "same 40/40" (run once, at dfcff03; later commits only add
switch-gated code, JSON blocks, snapshot fields and saturating counters, and every existing test still passes).
**Rows (hard, `fortify10.s15.*`, seed 1):** W1 plate stops 100 / control 0, ratio 3333 %, rifle and back diff 0; W2 riders
boarded 10, off 900 ms, 0 on the deck after the KO (10 stunned), control refused 1; W3 drawn 22, fired 20, miss 913 vs own 682 cm
(wider 1), German drawn 0; W4 bayonet 100 of 100 at 1.5 m, 0 at 1.9 m, switch 0: 0.
**Code map.** Rules: `crates/sim/src/s15.rs` (counters, `rider_exit`, `RiderOff`, `plate_damage`, `burst_riders`, rider causes),
`transport.rs` (seats, R3 exits), `combat.rs` (plate on hits and fragments, foreign spread, burst -> riders), `flame.rs` (bayonet
reach, plate on strikes), `supply.rs` + `structures.rs` (`Kit::foreign`, `feed_foreign`, `take_foreign`). Content: `defs.rs`
(`ForeignFeedDef`, `WeaponMeleeDef.bayonet`), `variant.rs` (`MemberDef::armour`), `setup.rs` (`PlateSetup`, `ForeignFeedSetup`,
`WeaponSetup::bayonet`, `TransportSetup::exposed`, `s15_off`), `modset.rs` (cross-checks), matchups in `resolve`. Rows:
`sdk/crates/cli/src/fortify/s15.rs`; JSON `s15` blocks and `s15_*` checks in `json_out.rs`. Snapshot: `DECK`, `RIDERS_OFF`,
`soldier_plate` (`godot/docs/match_state.md` "S15"; the bridge does not send them yet).
**Build B needs:** the USSR defs (no def in the mod carries a plate, rider slots, a foreign feed or a bayonet yet: the rows use
stand-ins made in code); the Pz IV / T-34 `[[rider_slots]]` boxes; the Mosin `[melee.bayonet]`; the 82-PM foreign feed; risk 4
(`ptrd_engages_a_halftrack`); T211 (switches on = base) after the data lands; T212-T218 are comments in `targets/S15.toml` until then.
**Deviations** (details in "S15a A"): `VehicleState::rider_off` and `AiRiders` added to the layout; hull-only rider box check;
plates skip riders and crews; bayonet via `WeaponMeleeDef` (setup keeps `MeleeDef` + `bayonet`); bursts suppress deck riders
(new, `[riders] enabled` 1 only); W2 KO staged as an engine fire; supply account counters saturate (fuzz); setup grew three times
after the layout commit (BASELINE refreshed each time); ablation files inherit the lists; batch.sh, pool.py, lobby FACTIONS unchanged.

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
