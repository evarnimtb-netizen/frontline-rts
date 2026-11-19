# S11b log (field howitzers as base guns, light mortars, HE rifle grenades, A0 fixes; at most 150 lines)
Spec: common `docs/slices/S11b.md`, stages `S11b-build-A.md`, `-B.md`, `-C.md`; targets `targets/S11b.toml`; ablations
`targets/S11b-ablations/`. Branch `claude/project-thread-e7nsak`, base `<BASE>` = `8d2731b` (also main). Another agent may
work in this checkout: commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked
`.claude/`, `target/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**GUARD RAILS TRIPPED on the default 40 set (report, not fixed; spec 8 stop rule is the 480's):** Germany 11/40
(27.5 %; s11b_off 19/40, guard 12-28 wins); HE share mean 31.06 % (max 47.8; s11b_off 23.00): above the T8 hard 26.3.
Ablations on the same seeds: rg_off Germany 11/40, HE 26.63; how_share_10 9/40, HE 29.54; C1-40 (how, buy_pending,
cb_alternate on; light and grenades off) 16/40, HE 23.03. So the light mortars carry most of the HE jump (US bombs a
match 53 -> 82) and the Germany drop; grenades add about 4.4 HE points. The orchestrator's 480 + `light_off` decide.
**State: Build C done (both chunks).** SAVE 29, PROTOCOL 15. All five switches on (`how`, `buy_pending`, `cb_alternate`,
`light`, `rifle_grenades`); howitzer cards `ai_weight` 5, light mortars 50 (half the heavy card's DEFAULT 100).
- Code map: `ai/arty.rs` `buy_arty` (infantry gun apart from `field_howitzer` cards; `buy_pending`) then `buy_how`;
  `is_how`, `mp_less_pending`; `ai/truck.rs` `of_role_where`, `buy_role_where` -> `RoleBuy` (the old callers pass
  "every card, raw manpower"); `ai.rs` `buy` L1 filter. Tests `crates/sim/src/s11b_ai_tests.rs` (registered in
  `world.rs`). JSON `json_out::s11b_blocks`, `skirmish::S11bPlayer/S11bFaction`; rows `cli/src/fortify/s11b.rs`.
- Review first: the infantry-gun buy path (filter, spent sum, gun count; `buy_pending`); the HE share's new term
  (`skirmish.rs`: mortar + tank HE + rifle grenade); rows `rg` (strips the kind in the setup for the control),
  `save_fuzz` `setup()` now forces every S11b switch 0 for the played saves.
- `cb_alt_holds` counts planner passes that held (A-4, no saved field): JSON also writes it as
  `cb_alt_hold_passes_per_match` and `cb_fired_matches_any_pct` (matches where the AI still counter-fired).
- Checks: release CLI rebuilt; `check.sh quick 8d2731b`: fmt, clippy, neutral.py, ablate dry runs ok; tests 1135
  passed, 1 failed `ipv6_loopback_handshake` (no IPv6 in this container; not S11b code; not skipped).

## Build C (2026-11-19; commits `14be6a3`, `c70e04e`, `714b70e`, `2f5875f`, `9583d3a`, `0bd224a`, `8088935`, docs, log)
Built: M1 (`s11b`, `s11b_derived` with spec 7 names plus `how_first_on_map_s_mean/min`, `light/how/rg` sums, side
detail), HE share + rifle grenade, `neutral.py` skips `s11b` keys BASE lacks (+ test); M2 rows `how_mask`, `how_barrage`,
`rg`, `light`; S11a1 `mask_once`/`barrage_once` take card, distance, side (S11a1 values unchanged: same scene at 530 m,
gun player 1); H1, H2 (no planner code), H3 (test only: the AI's ammo share reads rounds, not items, so no code; its
control is "rounds spent: a trip", not "no hand grenades", which never trigger a trip either), L1;
`save_roundtrip_carries_s11b_state` (howitzer record and a grenade mid-aim at tick 23441). Snapshot re-blessed (the five
switches, four weights). BASELINE untouched (frozen fixture; `setup_bytes_baseline` green).
Decisions: light weight 50 (heavy DEFAULT 100; "fewest buys per weight": heavy, heavy, light). `how_mask` keeps S11a1's scene: gun at (50, 500), point 600 m east, crest middle 80 m before it (no trench dug:
the mask does not depend on it). `how_barrage` = S11a1 barrage at 500 m with 8 rounds; the German gun fires the
mirror image from the west. `rg`: the target squad's own grenades taken away (only the firer's count).
`how_first_on_map_s` is the first second a howitzer squad is on the map (the arrival, not the order).
Fixes from the stage B review: B-1 push wording in both howitzer defs (crawl about 0.5 m a minute) and the H2 test
asserts no `PushGun`/`Hitch` names the howitzer (the arty AI emits only Emplace, FireMission, Stop; `take_guns` parks
infantry guns out of every list); B-2 with `rifle_grenades` 0 the rifle squads' setup bytes and match hashes changed
(a member slot and an item index shifted) while behaviour did not (neutral.py ignores `hash`); B-3 the numbers-only
assert removed (`range_m` is the AI's band; the solver reaches about 1,124 / 1,800 m); B-4 pack howitzer
`spot_size_pct` and shield box marked verify (owner to confirm); the leFH 18 muzzle `[90, 0, 5]` stays the leIG's,
verify: no barrel length in the spec or design.
Tests: `s11b_ai_tests` 10 (spec 4 H1 x4, H2 x2, L1, H3, all-off, determinism; names as the spec); cli
`s11b_json_names_match_the_targets_file`, `fortify_s11b_rows_run`, `s11b_rows_one_off`; save_fuzz +1; neutral +1.
Neutral: `neutral.py A0-40 C1off40`: `same 40/40`; `neutral.py A0-40 Coff40` (`-o s11b_off`): `same 40/40 (note:
overrides 0 -> 5 items)`.
Set C1-40 (how, buy_pending, cb_alternate): Germany 16/40, no winner 0, HE 23.03 / 39.6; G / U: window 95 / 95 %,
howitzer bought 100 / 92.5 % (window 100 / 97.4), first on map 693 / 760 s, fired 90 / 91.9 %, rounds 7.55 / 6.75 a
match, pct holds 90 / 126, mp holds 15.3 / 21 a match; cb share 31.2 / 51.1 %, cb answered 71.1 / 92.3 %, cb_alt
passes 67.8 / 95.2; M10 25 %, M3A1 7.5 %, StuG 35 %; gun refused 0.
Default C40: Germany 11/40, no winner 0, HE 31.06 / 47.8 (s11b_off 23.00 / 35.6). G / U: howitzer bought 97.5 / 95 %,
first on map 698 / 759 s (min 662 / 723), fired 89.7 / 84.2 %, missions' rounds 9.6 / 6.25 a match, kill share 5.4 /
1.9 %; light mortar bought 100 / 92.5 % of matches, buy share 33.5 / 40.4 %, bombs 62.9 / 81.7 a match; grenades
fired 5.15 / 7.33 a match, kill share 3.4 / 6.6 %; cb share 27.7 / 55.7 % (s11b_off 27.7 / 62.3), answered 63.2 /
92.3 %, still counter-fires in 60 / 90 % of matches, cb_alt passes 46.3 / 112.8 a match; T81 85.7 / 88.9 %;
M10 22.5 %, M3A1 10 %, Pz IV 67.5 %, StuG 32.5 %; refusals 0 / 0, gun refused 0 / 0, own-fire deaths 0.1 / 0.025 a match.
Rows (full `fortify-test --seeds 10`, `target/s11b-C40/fortify10.json`, PASS): how_mask leFH / M2A1 masked 100 / 100 %
of 20 orders (flat 0 / 0); how_barrage trench vs open leFH 3.4 % (open loss 50 %, control 63.4), M2A1 3.8 % (53 %,
control 75.5), 240 bursts each; rg G / U fired 1.0 / 1.4 a run of 3 / 6, open loss 25 / 95 %, trench vs open 0 / 3.2 %,
control fired 0 / 0, open loss 19.2 / 99 %; light kz vs heavy 101.7 %, 60 mm 86.5 %, 80 bombs each. S11a1 mask, gun_fm,
barrage rows unchanged; breach_prep moved (grenades on in its scene).
Targets on `target/s11b-C40` (fortify10, s11b_off and rg_off copied in as controls): 108 pass, 19 miss (4 hard: T5a/b,
T6a, T8c; the rest reports already missing before), 497 skipped (160 sets, other controls). T150-T152 all PASS. Not PASS
among T149-T158: T149d 55.7 (report, bound 50), T157a 31.06, T157b 27.5, T157c 27.5 (reports). No MISSING-METRIC,
CONTROL-MISSING or EXPOSURE-MISSING.
Shipped switches on broke 22 older tests (scenes with grenadiers, S11a2 counter-fire twice, content defaults): new
`MatchSetup::s11b_off` / `rifle_grenades_off` (content `setup.rs`); `s6_tests::flat_world_with`, the S5a, S5b, S6b, AI,
variants setups, the S10c front world and the played fuzz saves call it (they test older rules); content tests read
the shipped switches and weights. Files outside the contract's list, needed to keep the tree green.

## Build B (2026-11-19; commits `d3d3170`, `b81c462`, `e35144d`, `6bb4ea6`, `64ff872`, `cbbd276`, docs, `b645588`, log)
New defs: `ammo/105mm_fhgr38`, `ammo/gsprgr30`, `cartridges/60mm_m49a2`, `items/schiessbecher_sprgr`, weapons
`lefh18`, `m2a1_105mm`, `m1a1_75mm`, `kz_grw42`, `m2_60mm`, `schiessbecher`, vehicles `ger_lefh18`, `usa_105mm_m2a1`,
`usa_75mm_m1a1`, units `ger_lefh18_{crew,squad}`, `usa_105mm_m2a1_{crew,squad}`, `usa_75mm_m1a1_crew`,
`ger_kz_grw42_team`, `usa_60mm_mortar_squad`. Changed: the two line squads (C5), the four gun cards (`limit_group`),
`skirmish.toml` `[limit_groups] "ww2:artillery" = 2`, the rosters (appended).
Figures (design line; "verify" = S11b-13 plan value and its source): leFH 18 1,985 kg, crew 6, -5..+42, 10,700 m,
4/min, charge 1 200 (D 143, 757), 12 FH Gr 38 (D 395), 90/54/25 s (D 721), re-lay 25 (D 726-727), shield (D 809),
130 % (D 812), 480 / pop 2 / 180 s / 8:00 / max 1 (D 837, 842); top charge 325 verify (10,700 m at 42 deg drag-free);
traverse +-22, 9 and 6 deg/s verify (M3 def); spread 6 mrad verify (M3). M2A1 2,260 kg, crew 8, -5..+65, 11,300 m,
3/min (D 156), charges M3's + 333 verify (D 750 charge 1), 12 M1 HE (D 396), 90/54/25/25 (D 722), 500 / 2 / 180 s /
8:00 / 1 (D 838). FH Gr 38 19,000 g verify (as M1 HE; D 509), Sprgr 34 x 1.42: 170 / 160 rays / 1,700 / 17 m /
85 / 43 (D 143, 1051). kz. GrW 42: 15/min, 60-275 m (GA 726, game range verify), charges 75, 105 and band 45-87
(grw34), grw34 `[he]`, 18 bombs (GA 592), 181 MP, 45 s (RE 135). 60 mm M2: 18/min, 90-370 m (GA 730), charges
70/95/117/133 verify, band 45-85 (m1_mortar), 1,330 g (GA 730), `[he]` m1_mortar x 0.75 verify, 30 bombs (GA 591),
240, 45 s (RE 179). Pack howitzer (no card): 653 kg, crew 5, -5..+45, 8,800 m, 3/min (D 157), 30/18/15, push 90 cm/s
(D 720); existing `ww2:75mm_m48` (its `[he]` is the Sprgr 34's); one charge 294 verify (8,800 m at 45 deg, as the
howitzer tops); re-lay 10, cost/cooldowns, 10 rounds, traverse verify (leIG / M3). Gewehr-Sprenggranate 250 g, 250 m,
60 m/s, Mk 2 block with 48 rays, blast 160, reach 800 (S11b-7, owner to confirm); 30 m / 3 s / 3 s / 15 mrad as the M7.
Decisions: "no push": the def check allows `speed_cm_s` 0 for a towed gun but the setup check refuses a card speed of
0, so 1 cm/s with `push_men` = crew + 1 (7, 9). kz. GrW 42: 4 men with "2 + 4 + 6 + 6": the leader carries the fourth
6 (one bearer fewer than the heavy team). The rifle-grenadier slot is a split (no fallback needed: B2 = REF1).
Review fixes (coordinator, stage A review): A-1 `kill` and boarding clear `rg` (man, target, aim, stow); A-2 the aim
needs an enemy in the target's slot (ids are not generational), `RgState::target` doc; A-3 `class_of`: rifle
grenades leave `ammo.fired` (no Panzerfaust count); A-4 not fixed (above); A-5 for C. Also a Build A miss:
`how_rounds` was never counted (fixed, `6bb4ea6`).
Tests: sim `s11b_tests` +7 (`killed_grenadier_mid_aim_saves_and_loads`, `boarding_grenadier_saves_and_loads`,
`grenade_aim_breaks_when_the_target_slot_turns_friendly`, `rifle_grenade_is_not_counted_as_a_panzerfaust`,
`a_howitzer_card_takes_a_fire_mission`, `light_mortars_reach_their_game_range_and_minimum`, `artillery_group_caps_two`);
content `s11b_content` +3 (`s11b_defs_load_with_the_cited_figures`, `howitzers_are_inf_guns_with_a_low_angle_table`,
`rifle_grenadiers_carry_three`); `test_neutral.py` +1. Changed: `rifle_grenadier_fires_at_seen_infantry` counts 9.
Neutral: `neutral.py target/s11b-A0-40 target/s11b-B1-40`: `skirmish40.json: same 40/40`; `... target/s11b-B2-40`:
`skirmish40.json: same 40/40` (Germany 19/40 as REF1). BASELINE untouched (it resolves the frozen S3c fixture).
Checks: release CLI rebuilt; `tools/check.sh quick 8d2731b`: fmt, clippy, neutral.py, the five ablate dry runs ok;
tests 1122 passed, 1 failed: `udp` `ipv6_loopback_handshake` (no IPv6 in this container; not skipped). The new cards
broke 6 roster/type-count tests (fixed in `b645588`: 20 vehicle defs, the artillery group is index 0, two appended
cards a side) and the canonical snapshot (re-blessed; only the new defs, rosters, squads and limit group differ).

## Build A (2026-11-19; commits `3edf0cc`, log commit after it)
REF0 `target/s11b-base40` (base `8d2731b`), Germany 19/40; the targets file parses unchanged.
Built (one code commit; the steps were written together, so layout, keys, X1, X3 and G share it: deviation
from "each its own commit"): SAVE 29 / PROTOCOL 15 (`ItemKind::RifleGrenade` 6, `ITEM_KINDS` 7), `Squad::rg`
(`RgState` with one field more than spec 3: `target`, the man aimed at, so the aim breaks when he dies or
leaves the band, spec 5), `PlayerState::s11b`, `kill_class::RIFLE_GRENADE` 11 (`COUNT` 12), `AiArty::cb_last`,
four `AiCounters`, `ItemSetup::stow_ticks`, `Purpose::RIFLE_GRENADE` 40; keys of spec 2 (`[items]` keys live
in `ItemRulesDef`, not `tuning.rs`: the `[items]` table is not a tuning table); X1; X3; G1-G4; three US defs.
Code map: `combat.rs` `fire_rifle_grenades` (called after the Panzerfaust loop in `fire_launchers`);
`game.rs` `RgState::for_type` (call-in and `bail.rs` crew, as `pzf`); low root `indirect::low_root`; issue
rule `setup.rs` (member item lowering, `rules.items.rifle_grenades == 0` skips the kind); `cb_last` set in
`counter_fire`, cleared in `arty_fire`, read in `cb_candidate` after the stale gate (`cb_alt_holds`); X1 in
`guns.rs` `gun_mobility` (Emplacing ends) and `combat.rs` `kill` (slot man lost); refill exclusion in
`supply.rs` `refill_men` and `items.rs` (legacy free refill); `base_class`: launcher + HE ammo = rifle grenade.
Rule choices: the aim breaks on the man's single-shot rule, the target dead or out of the band (not on
sight lost: a lone man is seen about 13 % of ticks at 100 m and would never be hit); spread = the weapon's
`spread_mrad` x training `spread_pct` (no movement or range factor); ttl from the solved flight x 1.25.
Aim error, spread 0: at most 10 mm at 60-180 m (level and 5 m up); control flat aim: over 8 m at 150 m.
X1 (REF0 -> REF1): `s9_derived.gun_in_cover_pct` Germany 59.286 -> 86.458, USA 57.627 -> 87.179;
`s9.guns_emplaced` 140 -> 96 / 118 -> 78; `s9.gun_crew_lost` 92 -> 52 / 79 -> 67 (per match 2.3 -> 1.3,
1.975 -> 1.675). `neutral.py base40 A0-40`: "same 40/40" per match; the aggregate keys that differ are exactly
`s9.{guns_emplaced,gun_crew_lost}`, their `s8a.s9` copies, `s9_derived.{guns_emplaced_per_match,
gun_crew_lost_per_match,gun_in_cover_pct}` and `s5a.kills_by` 11 -> 12 items (tail 0, the new class).
REF1 holds all of stage A (not X1 alone): the stronger proof, and the base B and C need. Set 3
`target/s11b-A40` = default with `-o S11b-ablations/s11b_off.toml`: `neutral.py A0-40 A40` "same 40/40
(note: overrides 0 -> 1 items)". `cb_alternate` is not measured here (diagnosis 62.3 -> 47.3 %).
Tests: `s11b_tests` 17 (layout, load rejects 17 cases, X1, X3 x3, G x10), `s11b_content` 3.
Checks: workspace build ok (bridge); `check.sh quick 8d2731b` ok but `ipv6_loopback_handshake` (1112 passed).

## Plan and plan revision (2026-11-19; condensed by Build C, the spec `S11b.md` holds every rule)
Scope: A0 fixes X1 (infantry guns leave the AT-gun cover and crew-loss counts) and X3 (`cb_alternate`); X2 (vehicle
buys) is a finding, no key (S11b-10). Field howitzers leFH 18 and M2A1 as `inf_gun` base guns (no tow); the AI's second
gun inside its own 15 % share (`how`, review 4). Light mortars in the mortar slot (`light`). HE rifle grenades (new item
kind, launcher mode, own kill class inside the HE share, crate-only: no base refill). Pack howitzer data only; the
artillery limit group of 2. Review fixes folded into the spec: PROTOCOL 15 in the layout commit; `buy_pending`; low-root
grenade aim; minimum ranges 60 / 90 m; complete-round FH Gr 38; the two spent sums apart; unsourced figures in S11b-13.
Left out (spec 1): tows, siting, displacement, crates (S11a3), HEAT, Pzgr rot, the map flag, neutral guns in the cap,
`howitzer_pit`, the AT rifle grenade, authentic mortar ranges, sIG 33, Nebelwerfer, models and HUD.
Owner questions S11b-1..13 run on (A) ("assumed 2026-11-19, owner asked not to be asked", `docs/TODO.md`).
