# S11b log (field howitzers as base guns, light mortars, HE rifle grenades, A0 fixes; at most 150 lines)
Spec: common `docs/slices/S11b.md`, stages `S11b-build-A.md`, `-B.md`, `-C.md`; targets `targets/S11b.toml`; ablations
`targets/S11b-ablations/`. Branch `claude/project-thread-e7nsak`, base `<BASE>` = `8d2731b` (also main). Another agent may
work in this checkout: commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked
`.claude/`, `target/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: Plan done and revised 2026-11-19 (diagnosis and plan review folded in). Next: Build A (`S11b-build-A.md`), chunk 0 first.**
- SAVE 28, PROTOCOL 14 at the base. Build A makes SAVE 29 AND PROTOCOL 15 in ONE layout commit (the new `ItemKind` is on the
  wire in `Throw` / `ThrowEach`); B and C change no layout.
- **A0 diagnosis (measured by another session; it overrides the first plan's code reading):**
  X1 T81 is a counting error: `guns.rs` 1222 adds every emplaced served gun to `s9.guns_emplaced`, the numerator
  (`ai/guns.rs` 690 / 705) counts AT-gun jobs only; `s9.gun_crew_lost` (`combat.rs` 3570) has the same fault. A patched 40
  set played identically and read cover 86.5 / 87.2 %. Fix: skip `InfGun` in both (they count in `s11b`).
  X2 vehicle buys fall because the gun and observer cards take the manpower (USA about 500 MP a match), not through the pot
  or shells; `income_max_pct` 30 helped buys but read Germany 57.5 % on one 40 set. **No fix, no key** (S11b-10 (A)); T30,
  T40, T54 unchanged in `S11b.toml`.
  X3 counter-fire is tried first and a marker is fresh every 3 heard rounds, and the US observer rarely finds a post. Tested
  fix: saved flag `AiArty::cb_last` (set in `counter_fire`, cleared in `arty_fire`, `cb_candidate` holds while set): USA
  62.3 -> 47.3 %, counter-fire still in 97 % of matches. Built behind `cb_alternate` (0 until Build C).
- NOT checked by the Plan stage (no cargo, no CLI run): the targets file was parsed as TOML only (624 targets, 35 new, 21
  runs; every new row names a run that exists). **Build A chunk 0 runs `frontline-cli targets` on it and
  `tools/ablate.sh --dry-run` on the ablation folder, and fixes what they refuse.**
- Reference sets: REF0 `target/s11b-base40` (base), REF1 `target/s11b-A0-40` (after X1, every switch 0: equal to REF0 but
  for the X1 counter keys). Every later proof is against REF1. Set budget: A three, B one a chunk, C three.
- Traps the review found (all in the spec and contracts): `thrown()` must be false for the new kind; the bridge's
  `item_name` must know it or the workspace does not build; `gunnery::solution` is not a low-arc aim (use `indirect.rs`
  `solve_root`); `refill_men` would refill grenades in the base for about 0.7 MP (the kind is left out); `buy_role_within`
  ignores pending call-ins (`buy_pending`); the howitzer needs its own income share to appear before minute 19.
- Figures with no design source: rifle grenades (S11b-7, "owner to confirm"), the rest listed in S11b-13.
- Code map for Build A: `combat.rs` `fire_single_shot` (the pattern for G2-G3) and `base_class`; `game.rs` `SingleShot`,
  `kill_class`; `content/src/defs.rs` `ItemKind`; `ai/arty.rs` `run_arty`, `cb_candidate`, `counter_fire`, `arty_fire`.

## Plan revision (2026-11-19, after the measured diagnosis and an independent review: no blocker, 6 major, 7 minor; all verified)
Diagnosis: X1 also splits `gun_crew_lost`; X2 dropped (`pot_hold` gone; owner question S11b-10 rewritten); X3 is the
`cb_last` flag behind `cb_alternate`, the round budget dropped (a budget on rounds ordered does not bound the fired share).
Review: 1 PROTOCOL 15 in the layout commit. 2 Build A owns the bridge's `intents.rs` and `thrown()`. 3 `buy_pending`:
gun buys subtract pending call-ins. 4 The howitzer has its own share `how_income_pct` 15 (minute 11 / 12 against 19.7 / 22
inside the gun's share; median match 18-19.4 min); T153's exposure is the share of matches of 14 minutes or more. 5 No
base refill for rifle grenades (crate-only by design); the AI refill rule became "no trip for grenades". 6 Set budgets
(A 3, B 2, C 3); A keeps every switch 0, so one set proves default and `s11b_off`. 7 The grenade aims with the low root;
test `rifle_grenade_lands_on_its_aim_point`. 8 kz. GrW 42 minimum 60 m, 60 mm 90 m. 9 FH Gr 38 priced as the complete
round, 19 kg, like the M1. 10 Grenade burst 96 / 48 rays; T150 at 600 m; `scatter_pct` in the item def. 11 The howitzer's
and the infantry gun's `spent` sums are apart. 12 Rifle-grenade figures as given (M17: Mk 2 body, 0.68 kg, 180 m;
Gewehr-Sprenggranate: 250 g, 250 m, 60 m/s, about half the burst), "owner to confirm"; every other unsourced figure in
S11b-13. Ablations: `cb_budget_33` removed, `how_share_10` added; `s11b_off` holds all five switches.

## Plan (2026-11-19)
Scope (spec 0, 1), sized for three Build stages. (1) A0: three fixes from the S11a2 measure, each behind a key or proven
neutral: infantry-gun emplacements leave the AT-gun cover ratio (X1), [first plan: a pot-hold key and a round budget; see
the revision above: X2 has no fix, X3 is the alternate rule]. (2) Field howitzers leFH 18 and 105 mm M2A1 as
`inf_gun` cards with no tow: base guns that emplace where they arrive and fire flat; the AI buys one as a second gun from
8:00 [revised: with its own 15 % income share] (`how`). (3) Light mortars kz. GrW 42 and 60 mm M2 in the AI's mortar slot (`light`).
(4) HE rifle grenades: a new item kind and a launcher mode on the rifleman, two US and one German grenadier a full squad,
three grenades each, their kills a new class inside the HE share (`rifle_grenades`). (5) The 75 mm pack howitzer as data in
no roster; the artillery limit group of 2.
Left out, with the reason (spec 1): tows, siting, displacement and crates (S11a3: tow AI and crate AI are a session each,
and a human-only tow card has no use before a presenter pass); HEAT and Pzgr rot; crate-only grenades and shells; the map
flag; neutral guns in the cap; `howitzer_pit`; the AT rifle grenade; authentic mortar ranges; sIG 33 and Nebelwerfer;
models and HUD (presenter pass).
What the howitzers do without tows: arrive limbered in the base, emplace in 90 s, cannot be pushed, re-lay in 25 s, take
observed missions through the observer team's radio, are heard and counter-fired like the infantry guns, and are refused
"masked" wherever the flat arc from the base does not clear the ridge (the design's own verdict for a 1 km map).
Decisions taken for the owner (all (A), `docs/TODO.md` S11b-1..13 as revised, "assumed 2026-11-19, owner asked not to be asked"): this
scope; no tow cards; the howitzer as a second AI gun inside one share; kz. GrW 42 at 181 MP; light mortar ranges 275 m and
370 m; grenadiers 2 and 1; plan values for the grenade; grenade kills inside the HE share; [revised: no counter-fire twice in a row; vehicle
displacement accepted]; T81 without infantry guns; the artillery group of 2.
Honest expectation: the howitzers will fire little from the base (T154 reports it); rifle grenades favour the USA (two
launchers against one) with Germany at 41.67 %, so `rg_off` is the first ablation to read; the A0 causes are read from the
code, and the diagnosis may replace them.
Stage split: A (chunk 1: A0 fixes, SAVE 29 layout, keys; chunk 2: the rifle-grenade mode; every S11b switch 0) -> B (chunk
1: howitzers, their shell, light mortars; chunk 2: grenadiers, the group, pack howitzer data; all cards weight 0, default =
REF1) -> C (chunk 1: JSON, howitzer rows, the howitzer buy, `how` on; chunk 2: light mortars, grenades on, rows `rg` and
`light`, three 40 sets). Fallbacks in spec 8.
