# S11b log (field howitzers as base guns, light mortars, HE rifle grenades, A0 fixes; at most 150 lines)
Spec: common `docs/slices/S11b.md`, stages `S11b-build-A.md`, `-B.md`, `-C.md`; targets `targets/S11b.toml`; ablations
`targets/S11b-ablations/`. Branch `claude/project-thread-e7nsak`, base `<BASE>` = `8d2731b` (also main). Another agent may
work in this checkout: commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked
`.claude/`, `target/`, `tools/__pycache__/`. Do not push.


## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: Fix stage done.** SAVE 29, PROTOCOL 15. Shipped: `how`, `buy_pending`, `cb_alternate`, `rifle_grenades` 1;
`light` 0 (AI-only: the light mortar cards stay buyable). Default 40 set `target/s11b-I-def` (with the howitzer pot):
Germany 20/40, HE share 25.5 % (cap 26.3), howitzer bought in 100 / 95 % of matches.
- Howitzer pot (orchestrator decision after C-1 starved the buy: 0 / 0 % of matches): `ai/arty.rs` `how_pot`, derived,
  in `held_pots`; `buy_how` spends bank - pending - AT pot - infantry-gun pot. Rule built as `arty_pot`: income SO FAR x
  `how_income_pct` (not "income since 8:00", which would fill 480 MP only at about minute 21 and miss spec 7's minute 11-12
  window), held from `how_after_s`, 0 with `how`/`buy` 0 or `how_max` alive or pending, clamped to the card's price.
- The light-mortar and grenade sets below predate the pot (no howitzers bought); replay them in B2 on this head.
- Light mortars (owner question S11b-14 (A) off; levers `targets/B2-levers/s11b_light_*.toml`, B2 log line): Germany's own
  kz. GrW 42 carries the drop (Germany-only 9/40, USA-only 17/40); the review's smaller 60 mm burst did not help (13/40).
- Known weakness (not fixed): a crewless howitzer still counts as alive (`how_max`) and is never replaced.
- Code map: `ai/arty.rs` `buy_arty`, `how_pot`, `buy_how` (guards `how`, `buy`, `how_after_s`, `how_max`; manpower less
  pending call-ins and the other two pots), `is_how`, `mp_less_pending`; `ai/truck.rs` `buy_role_where` -> `RoleBuy`; `ai.rs` `buy` L1 filter;
  setup `rifle_grenades` 0 drops the item kind (content). Tests `crates/sim/src/s11b_ai_tests.rs`; JSON
  `json_out::s11b_blocks` (now with `light_bombs`), `skirmish::S11bPlayer/S11bFaction`; rows `cli/src/fortify/s11b.rs`.
- Checks: see the Fix block.

## Fix (2026-11-19; commits `027a5ba`..`94239b0`, logs)
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
Pre-pot default `target/s11b-H-def` (= set (a)): 19/40, HE 25.7. **Howitzer pot** (`cc02537`; tests
`ai_keeps_the_howitzer_pot_from_other_buys` (controls `how` 0, before `how_after_s`), `ai_buys_the_howitzer_once_its_pot_covers_it`
(control `how_income_pct` 10)). Default `target/s11b-I-def`: Germany 20/40, no winner 0, HE 25.5 / 39.6, length 20.1 min;
howitzer bought 100 / 95 % of matches (window 100 / 100), first on map 679 / 747 s (min 662 / 723), fired 85 / 86.8 %, kill
share 3.7 / 1.4 %, `how_mp_holds` 8 / 12 a match; vehicles bought 139 / 101 (pre-pot (a) 157 / 111), heavy mortars 150 /
99, line 275 / 155; grenade kill share 3.7 / 7.2 %. Passes both bounds: shipped as is, no further set. All-off
`target/s11b-I-off`: `neutral.py target/s11b-A0-40 target/s11b-I-off`: `same 40/40 (note: overrides 0 -> 4 items)`.
Review findings: C-1 `buy_how` subtracts `held_pots` (test `ai_howitzer_buy_leaves_the_gun_pots_alone`, control pot 0);
C-2 no howitzer with `buy` 0 (`ai_buys_no_howitzer_with_gun_buy_off`, control `buy` 1; the how tests now run `buy` 1 with
`after_s` 3600); stale `b_off` / `ff_off` comments fixed in `S11b.toml` and `B2.toml`; C-3 `tests/ai.rs` plays the shipped
switches, all 9 pass, no pin kept; C-5 Build C's "light mortar bombs 62.9 / 81.7" were every mortar bomb (`s5b`): JSON now
has `s11b.light_bombs` and `light_bombs_per_match` (observer of the tick's shots, no save change); C-6 the leaders carry
no bombs: kz. 4 + 7 + 7, 60 mm 6 + 8 + 8 + 8 (totals as cited). Snapshot re-blessed; BASELINE untouched.
Docs: `tuning.md`, `fire-missions.md` (light 0, the pots); TODO S11b-14; B2 levers `s11b_light_on`, `_ger_only`,
`_usa_only`, `_on_60mm_burst`; fix sets in `targets/S11b-fix/` (`how_rg_on`, `light_ger_only`, `light_usa_only`,
`light_on_60mm_burst`; `ablate.sh` takes only off values in `S11b-ablations/`, so not there).
Tests changed: `load_rejects_bad_s11b_keys` reads light 0; `save_roundtrip_carries_s11b_state` turns light on (held at
tick 29061). Spec H1 (`S11b.md`, 300 lines), `tuning.md`, `fire-missions.md` describe the pot.
Checks: release CLI rebuilt after the pot; `check.sh quick 8d2731b`: fmt, clippy, neutral.py, every ablate dry run ok;
tests 1139 passed, 1 failed `ipv6_loopback_handshake` (no IPv6 here).
