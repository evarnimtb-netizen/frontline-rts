# S11b log (field howitzers as base guns, light mortars, HE rifle grenades, A0 fixes; at most 150 lines)
Spec: common `docs/slices/S11b.md`, stages `S11b-build-A.md`, `-B.md`, `-C.md`; targets `targets/S11b.toml`; ablations
`targets/S11b-ablations/`. Branch `claude/project-thread-e7nsak`, base `<BASE>` = `8d2731b` (also main). Another agent may
work in this checkout: commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked
`.claude/`, `target/`, `tools/__pycache__/`. Do not push.


## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: Fix stage done.** SAVE 29, PROTOCOL 15. Shipped: `how`, `buy_pending`, `cb_alternate`, `rifle_grenades` 1;
`light` 0 (AI-only: the light mortar cards stay buyable). Default 40 set: Germany 19/40, HE share 25.7 % (cap 26.3).
**Decide first (orchestrator): fix C-1 starves the AI howitzer.** With the AT and infantry-gun pots held from it (as
`buy_fo` and `buy`), the default set buys a howitzer in 0 / 0 matches (was 97.5 / 95 %): `how_mp_holds` about 270 a
match a side. The general buy spends everything above the pots and no pot is held for the howitzer (spec), so it never
reaches pots + 480 + 300. The grenades-on choice rests on this: before C-1 the same set read 16/40, HE 26.9 (fails the cap).
Ways out (spec change): a howitzer pot in `held_pots`, or C-1 for the leIG pot only; then replay `how_rg_on` and the default.
- Light mortars (owner question S11b-14 (A) off; levers `targets/B2-levers/s11b_light_*.toml`, B2 log line): Germany's own
  kz. GrW 42 carries the drop (Germany-only 9/40, USA-only 17/40); the review's smaller 60 mm burst did not help (13/40).
- Known weakness (not fixed): a crewless howitzer still counts as alive (`how_max`) and is never replaced.
- Code map: `ai/arty.rs` `buy_arty`, `buy_how` (guards `how`, `buy`, `how_after_s`, `how_max`; manpower less pending call-ins
  and `held_pots`), `is_how`, `mp_less_pending`; `ai/truck.rs` `buy_role_where` -> `RoleBuy`; `ai.rs` `buy` L1 filter;
  setup `rifle_grenades` 0 drops the item kind (content). Tests `crates/sim/src/s11b_ai_tests.rs`; JSON
  `json_out::s11b_blocks` (now with `light_bombs`), `skirmish::S11bPlayer/S11bFaction`; rows `cli/src/fortify/s11b.rs`.
- Checks: see the Fix block.

## Fix (2026-11-19; commits `027a5ba`..`04feeba`, log)
What each switch gates: `[ai.arty] how` (the AI's howitzer buy), `light` (light mortar cards in the AI's mortar slot),
`buy_pending` (the AI's gun buys count pending call-ins), `cb_alternate` (the AI's counter-fire planner): AI only, every
card stays buyable. `[items] rifle_grenades` is content: 0 drops the `rifle_grenade` kind from every squad type at setup,
so no player, human or AI, gets them. Per-side light: `[defs.units."<card>"] ai_weight = 0` in an override (no code).
Sets (seed 1, 20 x both bases, 45 min, `BATCH_JOBS=4`; Germany wins / 40, HE share mean %). Before C-1 (Build C code):
C1-40 (how, light off, grenades off) 16, 23.0; (a) `how_rg_on` 16, 26.9; Germany-only light (grenades on) 10, 27.6;
USA-only 20, 28.2. After C-1, C-2, C-6 (`target/s11b-G-*`; howitzer bought in at most 5 matches a side in every set):
| set | Ger | HE | light buys G/U | light bombs a match G/U | heavy mortar buys G/U | line G/U | vehicles G/U |
|---|---|---|---|---|---|---|---|
| light off, grenades off | 20 | 23.1 | 0/0 | 0/0 | 190/121 | 342/182 | 174/121 |
| (a) grenades on (shipped) | 19 | 25.7 | 0/0 | 0/0 | 179/109 | 323/174 | 157/111 |
| light both, grenades off | 11 | 23.1 | 68/48 | 17.5/41.2 | 132/78 | 263/157 | 145/121 |
| (b) Germany only | 9 | 20.9 | 65/0 | 17.6/0 | 118/119 | 209/147 | 113/97 |
| (c) USA only | 17 | 22.8 | 0/51 | 0/39.4 | 191/93 | 351/189 | 173/112 |
| (d) all on, 60 mm burst 55 rays / 900 cm | 10 | 29.0 | 68/49 | 16.8/40.0 | 141/78 | 257/152 | 136/120 |
| light both + that burst, grenades off | 13 | 23.6 | 71/51 | 15.5/42.0 | 144/79 | 268/157 | 145/122 |
Reading: a kz. team fires about 10 of its 18 bombs (the 60 mm about 34 with resupply); Germany's light buys displace
heavy mortars first (190 -> 132), then line squads and vehicles; German bombs a match 93 -> 72 while US bombs rise 68 ->
99. The review's diagnosis (the 60 mm better per MP) holds for bombs, but Germany's own kz. GrW 42 costs more (b vs c).
Rule applied (Germany >= 15/40 and HE <= 26.3, most features on): `light` 0 (every light set fails); grenades 1 ((a)
passes, margin 0.6 HE points); (d) fails, so no option (C) in S11b-14 and nothing shipped from it.
Default set `target/s11b-H-def`: Germany 19/40, no winner 0, HE 25.7 / 39.6, length 20.2 min; grenades fired 6.3 / 8.3 a
match, kill share 3.4 / 6.5 %; counter-fire in 57.5 / 90 % of matches; = set (a) `same 40/40`. All-off
`target/s11b-H-off`: `neutral.py target/s11b-A0-40 target/s11b-H-off`: `same 40/40 (note: overrides 0 -> 4 items)`.
Review findings: C-1 `buy_how` subtracts `held_pots` (test `ai_howitzer_buy_leaves_the_gun_pots_alone`, control pot 0);
C-2 no howitzer with `buy` 0 (`ai_buys_no_howitzer_with_gun_buy_off`, control `buy` 1; the how tests now run `buy` 1 with
`after_s` 3600); stale `b_off` / `ff_off` comments fixed in `S11b.toml` and `B2.toml`; C-3 `tests/ai.rs` plays the shipped
switches, all 9 pass, no pin kept; C-5 Build C's "light mortar bombs 62.9 / 81.7" were every mortar bomb (`s5b`): JSON now
has `s11b.light_bombs` and `light_bombs_per_match` (observer of the tick's shots, no save change); C-6 the leaders carry
no bombs: kz. 4 + 7 + 7, 60 mm 6 + 8 + 8 + 8 (totals as cited). Snapshot re-blessed; BASELINE untouched.
Docs: `tuning.md`, `fire-missions.md` (light 0, the pots); TODO S11b-14; B2 levers `s11b_light_on`, `_ger_only`,
`_usa_only`, `_on_60mm_burst`; fix sets in `targets/S11b-fix/` (`how_rg_on`, `light_ger_only`, `light_usa_only`,
`light_on_60mm_burst`; `ablate.sh` takes only off values in `S11b-ablations/`, so not there).
Tests changed: `load_rejects_bad_s11b_keys` reads light 0; `save_roundtrip_carries_s11b_state` turns light on and starts
with 20,000 MP (with C-1 no howitzer is bought on the shipped economy; held at tick 19401).
Checks: release CLI rebuilt after C-5; `check.sh quick 8d2731b`: fmt, clippy, neutral.py, every ablate dry run ok
(S11b after the fix sets left `S11b-ablations/`); tests 1137 passed, 1 failed `ipv6_loopback_handshake` (no IPv6 here).

## Build C (2026-11-19; commits `14be6a3`, `c70e04e`, `714b70e`, `2f5875f`, `9583d3a`, `0bd224a`, `8088935`, docs, log)
Built: M1 (`s11b`, `s11b_derived`, side detail), HE share + rifle grenade, `neutral.py` skips `s11b` keys BASE lacks; M2
rows `how_mask`, `how_barrage`, `rg`, `light`; S11a1 `mask_once`/`barrage_once` take card, distance, side; H1, H2 (no
planner code), H3 (test only), L1; `save_roundtrip_carries_s11b_state`. Light weight 50 (heavy DEFAULT 100).
Stage B review fixes: B-1 push wording, the AI emits no `PushGun`/`Hitch` for a howitzer; B-2 with grenades 0 setup bytes
and hashes changed, behaviour not; B-3 an assert removed; B-4 pack howitzer spot size and shield marked verify.
Tests: `s11b_ai_tests` 10; cli `s11b_json_names_match_the_targets_file`, `fortify_s11b_rows_run`, `s11b_rows_one_off`.
Set C1-40 (how, buy_pending, cb_alternate): Germany 16/40, HE 23.03; howitzer bought 100 / 92.5 %, first on map 693 /
760 s. Default C40 (all five on): Germany 11/40, HE 31.06 / 47.8; light bought 100 / 92.5 % of matches; every mortar
bomb 62.9 / 81.7 a match (not light only, review C-5). rg_off 11/40, HE 26.63; how_share_10 9/40, 29.54.
Rows (`target/s11b-C40/fortify10.json`, PASS): how_mask 100 / 100 % masked (flat 0 / 0); how_barrage trench vs open 3.4 /
3.8 %; rg fired 1.0 / 1.4 a run, open loss 25 / 95 %; light kz vs heavy 101.7 %, 60 mm 86.5 %.
Targets on C40: 108 pass, 19 miss (4 hard: T5a/b, T6a, T8c), T150-T152 PASS; T157a-c reports out of band.
`MatchSetup::s11b_off` / `rifle_grenades_off` keep older-rule tests on the old behaviour (22 tests broke with the switches on).

## Build B (2026-11-19; commits `d3d3170`, `b81c462`, `e35144d`, `6bb4ea6`, `64ff872`, `cbbd276`, docs, `b645588`, log)
New defs: ammo `105mm_fhgr38`, `gsprgr30`, `60mm_m49a2`, item `schiessbecher_sprgr`, weapons `lefh18`, `m2a1_105mm`,
`m1a1_75mm`, `kz_grw42`, `m2_60mm`, `schiessbecher`, vehicles `ger_lefh18`, `usa_105mm_m2a1`, `usa_75mm_m1a1`, units
`ger_lefh18_{crew,squad}`, `usa_105mm_m2a1_{crew,squad}`, `usa_75mm_m1a1_crew`, `ger_kz_grw42_team`,
`usa_60mm_mortar_squad`; line squads (C5), the four gun cards' `limit_group` ("ww2:artillery" = 2), rosters appended.
Figures and their sources: the defs' comments and `docs/modding/fire-missions.md`; unsourced ones are S11b-13 "verify".
Decisions: "no push" as 1 cm/s with `push_men` = crew + 1; the rifle-grenadier slot is a split. Stage A review fixes A-1..A-3
(kill and boarding clear `rg`; the aim needs an enemy in the target's slot; rifle grenades leave `ammo.fired`); A-4 open
(`cb_alt_holds` counts passes); `how_rounds` counted (`6bb4ea6`).
Tests: `s11b_tests` +7, `s11b_content` +3, `test_neutral.py` +1. Neutral: A0-40 vs B1-40 and B2-40 `same 40/40`.

## Build A (2026-11-19; commits `3edf0cc`, log commit after it)
REF0 `target/s11b-base40` (base `8d2731b`), Germany 19/40. One code commit: SAVE 29 / PROTOCOL 15 (`ItemKind::RifleGrenade`
6, `ITEM_KINDS` 7), `Squad::rg` (`RgState` + `target`), `PlayerState::s11b`, `kill_class::RIFLE_GRENADE` 11,
`AiArty::cb_last`, four `AiCounters`, `ItemSetup::stow_ticks`, `Purpose::RIFLE_GRENADE` 40; spec 2 keys; X1; X3; G1-G4.
Code map: `combat.rs` `fire_rifle_grenades`; `game.rs` `RgState::for_type`; `indirect::low_root`; issue rule `setup.rs`;
`cb_last` in `counter_fire` / `arty_fire` / `cb_candidate`; X1 in `guns.rs` `gun_mobility` and `combat.rs` `kill`.
X1 (REF0 -> REF1): `gun_in_cover_pct` 59.3 -> 86.5 / 57.6 -> 87.2; only the X1 counter keys differ (`neutral.py`).
Set 3 `target/s11b-A40` (s11b_off): `neutral.py A0-40 A40` "same 40/40". Tests `s11b_tests` 17, `s11b_content` 3.

## Plan and plan revision (2026-11-19; condensed, the spec `S11b.md` holds every rule)
Scope: X1, X3; field howitzers leFH 18 / M2A1 as `inf_gun` base guns; light mortars; HE rifle grenades (new item kind,
crate-only); pack howitzer data only; artillery limit group 2. Left out: spec 1. Owner questions S11b-1..14 run on (A)
("assumed 2026-11-19, owner asked not to be asked", `docs/TODO.md`).
