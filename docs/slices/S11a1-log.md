# S11a1 log (gun fire missions, infantry guns, artillery AI; at most 150 lines)
Spec: common `docs/slices/S11a1.md`, stages `S11a1-build-A.md`, `-B.md`, `-C.md`; targets `targets/S11a1.toml`;
ablations `targets/S11a1-ablations/`. Branch `claude/project-thread-e7nsak`, base `730e67f` (S10c-ai; code
`c1302b3`). Another agent may work in this checkout: commit only your files (`git commit -m ... -- FILES`), never
`git add -A`, never the untracked `.claude/`, `target/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: all builds, review fixes C-1..C-7 and the 480 measure done 2026-11-17 (at `e149258`, see Measure and docs). Next: the owner's merge decision (S11a1-9, S11a1-10); `tools/verify.sh` not yet run.**
- SAVE 27, PROTOCOL 14. `[ai.arty]` `buy`, `fo`, `prep` 1 in `skirmish.toml`; gun cards `ai_weight` 5.
- Planner `crates/sim/src/ai/arty.rs` (notes in the "Build C" comment), called from `ai.rs` `plan`; `held_pots()` = `gun_pot` + `arty_pot`.
- P1 `breach.rs` `prep_focus`, `arty.rs` `prep_target`; P2 `breach.rs` `prep_wait`; P3 `arty.rs` `lift_mortar_prep`.
- Tests: `crates/sim/src/s11a1_ai_tests.rs` (13). Sets: `target/s11a1-C40/` (default), `target/s11a1-C40off/`, `target/s11a1-C40-{arty,prep,fo}_off/`.
- Open: T131a/b reports until S11a1-10; B-5 not done; `ai_gun_orders_nothing_the_sim_refuses` not written (the 40 set reads refused 0).

## Fix (2026-11-17, review C-1..C-7, after S11a2 Build A; each verified in the code first)
Commits: `8cb2c4d` C-1 (gun_dry drop -> `fm_dry_drops`, not a mission), `7f878b5` C-2 (`buy_fo` minus pending call-ins;
tests `arty_pot_is_kept_from_other_buys`, `ai_rebuys_a_lost_gun_inside_the_pot`; s5a_ai keeps arty off: with it on no
seed 1-8 has the refill+ambush scene), `c9a480b` C-3 (post reset once a `repath_s`), `28c6425` C-4 (P1 uses `[ai.arty]
danger_close_m`; P3 lifts only missions aimed at the focus), `cbfa456` C-5 (`NO_POINT` loads), `f695368` C-6 (`prep` read
by C-1; switches-off test checks the whole stream), `cddb069` C-7 (row mortar `place_observing` 250 m: prep_fired 100 %
on seed 1, gap still 0 %; S11a1-10 in TODO; T131a/b reports). Each new test seen to fail with its fix undone.
Check `tools/check.sh quick 730e67f` all ok, 961 tests. New default set `target/s11a2-base40-fix` (the neutral base for
S11a2 Build B): Germany 15 of 40 (37.5 %); gun missions Germany 246 (6.2 a match; 238 observed, 7 map), USA 130 (3.3;
124 observed, 5 map); `neutral.py s11a2-base40 s11a2-base40-fix` same 36/40 (the fixes change AI play).

## Build C (2026-11-17)
Commits: `afd7891` B-2, `25436fa` planner A1-A6, `dafa3ef` B-1, `1c70fad` B-3/B-4, `8015d6e` tests, `fe3ff69` weights + `buy`/`fo`
on, `4df680c` P1-P3 + withdraw, `54bfb12` breach rows, `f09d1c9` `prep` on; log commit.
Review fixes. B-1: CLI counts every refusal event of a gun squad (`S11a1Player::gun_refused`) -> `gun_refused_total`.
B-2: the mask reads steps 2..=16 at +0.5 m, 17..=19 and the point 5 m short of the aim at -2 m (a flat shot into a trench
passes under the lip; a crest's shadow does not); an aim point (with the aim error) in the shadow ends the mission unfired.
Test `accepted_leig_points_behind_a_crest_are_hit` (control: the Build A rule accepts x 523 and bursts 14 m short). Rows
after B-2: T128 600 %, T129 M3 masked 100 % (control 0), leIG fired over 90 % (one seed holds: no clear arc), T130 11.8 %
(T130b has min 0, it cannot fail). T129 note: the gun is 530 m from the target (the high-root band starts near 482 m).
B-3: only lists named `refused` may grow; B-4: an `s11a1` block the base lacks must be 0 but for `matches`, `kills`,
`income_mp`, `gun_kills_in_tank_he` (tests with controls in `test_neutral.py`). B-5 not done (barrage control unchanged).
Rules as built (tests, control in brackets): A1 `ai_buys_an_infantry_gun_inside_its_budget` (buy 0; income 1 %); A2
`ai_emplaces_its_infantry_gun` (buy 0); A3 `ai_buys_one_observer_for_its_gun` (fo 0; also: only A3 orders),
`ai_observer_post_search_fails_behind_a_ridge_and_is_bounded` (flat: the test above), `ai_observer_is_rebought_once` (rebuy
600 s); A4 `ai_gun_fires_an_observed_mission_through_its_observer` (radio man dead); A5 `ai_gun_stops_at_its_shell_budget`
(100 %); A6 `ai_lifts_gun_fire_when_own_men_close` (80 m); `ai_all_s11a1_switches_off_emit_nothing_new` (buy 1);
`ai_arty_deterministic_threads_and_save_load` (1/4 threads, saves at 1, 12, 25 s; control radio dead); P1
`ai_mortar_fires_prep_for_a_breach` (prep 0); P2 `ai_breach_waits_for_prep_fire` (prep 0), `ai_breach_prep_wait_is_bounded`
(control: the run before, no timeout). Mortar decisions unchanged: the neutral run (no separate unit test).
Decisions: the post search has its own `post_rays` (spec 6), not the mortars' budget; observer cards found by trait
(observer + radio, no vehicle), bought outside the pots but never into them; gun band uses the table `range_m` (the
weapon's direct `range_max` hid every target); A6 lifts any AI gun mission; a P2 timeout or the rounds set `prep_at` once.
40 sets (seeds 1-20 both bases; Germany wins of 40): base 16, default 15 (37.5 %), arty_off 16, prep_off 15 (equal to
default: no breach job starts, spec 0), fo_off 11 (27.5 %: guns without observers fire 0.3 / 0.25 rounds a match).
`neutral.py base40 C40off` (a_off): `same 40/40`; also after chunk 1 (`C1off40`). HE share mean 20.3 (max 31.4).
Default 40, Germany / USA: gun bought 100 / 100 %, fired 95 / 85 % of those, missions 6.1 / 3.2 a match (observed 235 / 122,
map 8 / 5: the sim falls to map when the observer is lost in the delay), rounds 20.1 / 9.1 a match, observed 86.3 / 80.1 %,
kills 86 / 99 (3.2 / 2.9 % of kills), shells 2.7 / 4.5 % of income (budget holds 0), near-own bursts 0.05 / 0.2 a match,
refused 0 / 0, observers lost 0.08 / 0.1 a match, post fails 43 / 45 %, no-observer plans 95 / 97 %, lifts 42 / 40,
prep missions 0 / 0, breaches started 0. Balance: the guns do not tilt to Germany on 40 (15 vs 16 of base).
Breach rows (`fortify-test`, 10 seeds x both sides, value / control prep 0): mortar gap 0 / 0, placed 0 / 0, attackers
lost 76.8 / 76.8 %, prep fired 0 %; gun gap 0 / 0, placed 0 / 0, attackers 77.7 / 77.7 %, prep fired 35 %.
Other files touched: tests found with the artillery AI off keep it off (`s10a_tests` empty-fort fight, `tests/s5a_ai.rs`
`ai_setup`, `tests/save_fuzz.rs` `setup`); `tests/ai.rs` accepts an infantry gun's mission; content test card weights.
Check: `tools/check.sh quick 730e67f` all ok, 929 tests (release CLI rebuilt). Full `fortify-test --seeds 10` (1,616 s)
`target/s11a1-C40/fortify10.json`; `targets --from-json target/s11a1-C40`: 84 pass, 9 miss (hard 0), 2 unexposed.
T128a/b, T129a, T130a/b, T137e/f PASS; T129b 90 MISS (report); T131a/b UNEXPOSED (mortar prep fired 0 %, min 80), T131c
76.8 MISS (report), T131d (gun) 0 MISS (report, exposure 35 %). T132-T138 on 160 sets: the orchestrator's 480.

## Build B (2026-11-17)
Commits: `6f22f00` content + fix A-1, `c2afd8c` A-2..A-6, `6a8db14` rows / JSON / neutral.py / doc, `2097289` rows
on threads, `50eedd6` one-off test, `aaae560` roster tests, `a05657a` rows into the fortify JSON, `59d8dc3` mask aim point; logs. Content as built (`artillery-on-map.md` 2.1 lines 133-136,
2.2 lines 148-151, 4.7 lines 825-840; spec 4 C1-C4): leIG 18 400 kg, crew 5, -10..+73, traverse -5..+6, charges
92-210, 3,550 m, 8 mrad, 8/min (7.5 s), 12 Igr 18 (6.0 kg), 320 / pop 2 / 90 s / 3:00, max 2; M3 1,130 kg, crew 6,
-9..+30, +-22, charges 205-311, 7,600 m, 6 mrad, 3/min (20 s), 10 M1 HE (19 kg; blast 175 cm, 160 rays to 17.5 m,
supp. 18 m, crater 90 cm, depth 44 cm = 30 x 1.46: the spec names no depth), 380 / 2 / 120 s / 4:00, max 2;
VB-Trupp 2 men 80 / 60 s / max 2; FO Party 3 men 120 / 60 s / max 2; all `ai_weight` 0. Geometry (pivot, muzzle,
shield plates) "verify". Changed from spec: the crew commander has `radio` but not `leader` (the def check allows
`leader` on slot 0 only); the crew leader is the gunner slot (first man).
Review fixes: A-1 one rule `vehicle::low_angle_top` (def check to 89, cross-check: above 60 only with a low-angle
table; setup check the same), `radio` moved to TRAITS, the test parses real defs (control: no `low_angle` -> refused);
A-2 `range_m` -> `CannotReach`; A-3 gun refusals out of `fm.refused` (event and s11 kept), JSON flag
`gun_kills_in_tank_he`; A-4a mission ends when `sv.crew` names no live squad; A-4b re-lay to the mission bearing
while holding fire; A-5 `gun_ready()` in `crew_ok`; A-6 tests `real_m3_is_masked_behind_a_crest_through_the_sim`
(control leIG, flat), `gun_refusals_take_the_first_broken_rule`, `gun_refuses_past_its_table_range`. A-4/A-5 have no
own test (no cheap scene: a round every 7.5 s hides a 10 s re-lay). Mask note: the test skips the first tenth and last
fifth of a flight; revisit with S11a2 friendly fire.
Other files touched (smallest fix): `damage.rs` test list (two guns), the 17-type test (now 18 with a clone),
`indirect::candidates` pub(crate). Solver test with real defs lives in `s11a1_tests.rs` (the solver is the sim's).
Gate (`fortify-test --seeds 10`, `target/s11a1-B40/fortify10.json`; = `s11a1_rows_one_off`): **T128** ratio 600.0 % (observed 8.3 m:
leIG 9.9, M3 6.8; map 49.8 m; exposure 160 rounds; control radio man dead 100.0 %). **T129** M3 refused masked 100 %
(control flat 0 %), leIG burst within 10 m of its aim point 80 % (control flat 100 %; report bound 100: in 2 of 10
seeds the map-fire aim error puts the aim point in the crest's shadow and the round bursts on the far slope, which
the mask test cannot see because it skips the last fifth of the flight, A's deviation), orders 20.
**T130** trench vs open 11.8 % (open 68.0 %, trench 8.0 %; control: dug, squad in front
76.5 %), trench suppressed 2.2 %. Bench: flat map; gun_fm 600 m, FO 300 m short (radio only); mask crest band 25 m,
gun 530 m from target; barrage M3 500 m, FO 350 m. Rows wall 16.8 s for 10 seeds (threads), the
added fortify time; whole `fortify-test --seeds 10` 1,676 s (no base timing exists; the S10b-ai rows dominate).
Neutral: `neutral.py base40 B40` `same 40/40`; `base40 B40off` (a_off) `same 40/40`. Tests: content `s11a1_content`
4, sim s11a1 25, CLI `fortify_s11a1_rows_run`, `s11a1_json_names_match_the_targets_file`, neutral 13.
Check: `tools/check.sh quick 730e67f` all ok, 915 tests (release CLI rebuilt; first run failed two roster-order
tests, fixed in `aaae560`). Full `fortify-test --seeds 10` (`target/s11a1-B40/fortify10.json`, with the 30-min S10b-ai
rows): the first run (1,676 s) lacked the `s11a1` key (fixed in `a05657a`); the rerun (1,660 s) gives the same
T128-T130 values; `targets --from-json target/s11a1-B40`: T128a/b, T129a, T130a/b PASS, T129b MISS (report), T131
SKIP (null), no MISSING-METRIC / CONTROL-MISSING / EXPOSURE-MISSING; T137e/f PASS (0); T132-T138 160 sets not played.

## Build A (2026-11-17)
Commits: `9ed0bf0` layout (SAVE 27, kill class u16, S11 counters, AI records), `84a972a` keys / schema / radio /
inf_gun / AT-gun split, `030e6ff` G2 solver, `067dee7` G1, G3-G7 (gun missions, radio, ends, rearm, counters).
Tests (control in brackets): `seventeen_vehicle_types_keep_their_kill_rows` (old u8 pack), `load_rejects_bad_s11a1_state`
(staged save loads), `s11a1_layout_roundtrips_and_hashes`, `at_gun_ai_ignores_infantry_guns` (at_gun clone gets a
job), `load_rejects_bad_s11a1_keys` (`gun_break_m` widened: failed), `indirect_gun_keys_and_radio_trait_parse` (band to
90), six G2 tests in `indirect.rs` (M3 masked; level ground), `gun_takes_a_fire_mission` (`gun_missions` 0),
`gun_mission_refusals_in_order` (accepts last), `gun_mission_fires_counted_he` (no HE), `gun_mission_waits_for_the_crew`
(loader check removed: failed), `gun_on_mission_fires_no_direct_round` (Idle fires), `infantry_gun_counts_only_in_s11`
(at_gun in s9), `gun_mission_deterministic_threads_and_save_load`, `gun_mission_leaves_ranging_memory` (map fire: none),
`gun_map_error_is_twice_the_observed` (mortar keys), `gun_mission_ends_when_packed` (not packed), `gun_mission_breaks_for_self_defence`
(200 m), `radio_links_an_observer_across_the_map` (three controls), `radio_does_not_reveal` (near enemy seen),
`infantry_gun_does_not_rearm_in_base_when_switched_off` (1 rearms).
Neutral: `neutral.py base40 A40` and `... Aoff40`: `DIFFERENT, same 40/40`, the only key `s5b.refused` 7 -> 9 items.
Deviations: the mask never reads the last fifth of a flight (steps 17-20, spec); `Masked` in the sim is tested
through the solver test, not a cratered map; the crew-ordered-off end is "crew link released" (`sv.crew` None);
unit orders that take one man off make the gun wait, not end; `InsideMinimum` only when no candidate exists (a
low-angle gun always has one at short range); `IndirectDef` literals in `indirect.rs` tests gained `..Default`;
the setup check accepts `[indirect]` on a `cannon` with `low_angle`, and mount elevation to 89 for it.
Check: `tools/check.sh quick 730e67f` all ok, 907 tests passed (release CLI rebuilt first).

## Plan revision (2026-11-16, after an independent review: 1 blocker, 6 major, 4 minor; all verified, none rejected)
Kill class widened (15 types), G2 mask + hull pitch, G3 ttl + load checks, A1 role split, Build C wiring; `arty_off` first.

## Plan (2026-11-16)
Scope and stages: spec 1 and 9; decisions S11a1-1..9 in `docs/TODO.md` (all (A)). Stage split A (layout, keys, solver,
missions, radio, rearm) -> B (content, rows, JSON; gate T128-T130) -> C (AI, P1-P3, breach row). The breach gap: breach
jobs do not start in matches (T123), so `prep_missions_per_match` is 0 there; that needs S10c-6 (B).

## Measure and docs (2026-11-17)
Measure: `target/s11a2-measure/measure.txt` at `e149258` (480 matches, shared with S11a2). Germany pooled 41.67 % (sets
46.25 / 38.12 / 40.62; S10c-ai 40.00 %), no winner 0. S11a1 report rows: near-own bursts 0.019 / 0.069 a match (T137a/b),
prep missions 0 (T138), breaches started 0 (T123).
Misses that the artillery may explain (cause not found; S11a2-11, S11a2-12): AT guns in cover 57.8 / 56.4 % (T81a/b, floor
60; 84-94 % at S10c-ai); Pz IV bought 89.4 % (T54a, floor 90), MP share 9.97 (T54c); M10 bought 32 % (56 %), M3A1 16 % (36 %).
Breach row T131 stays a report (S11a1-10). Checks: `tools/check.sh quick 730e67f` all ok, 961 tests at the last fix. CI on
`affb26e` (S11a1 complete plus S11a2 stage A): Linux, release and debug sim tests, simulate, clippy, rustfmt, cargo-deny
passed; Windows and THIRD_PARTY_NOTICES fail as on main (PR 1). `tools/verify.sh` NOT run.
