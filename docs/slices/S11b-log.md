# S11b log (field howitzers as base guns, light mortars, HE rifle grenades, A0 fixes; at most 150 lines)
Spec: common `docs/slices/S11b.md`, stages `S11b-build-A.md`, `-B.md`, `-C.md`; targets `targets/S11b.toml`; ablations
`targets/S11b-ablations/`. Branch `claude/project-thread-e7nsak`, base `<BASE>` = `8d2731b` (also main). Another agent may
work in this checkout: commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked
`.claude/`, `target/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: Plan done 2026-11-19. Next: Build A (`S11b-build-A.md`), chunk 0 first.**
- SAVE 28, PROTOCOL 14 at the base. Build A makes SAVE 29 in ONE layout commit; B and C change no layout. PROTOCOL stays 14.
- **A0 diagnosis: not in this file yet.** Another session is measuring the three open items (T81a/b, vehicle buys, T148d).
  The orchestrator pastes its findings here, under this line, before Build A starts. Where they differ from spec 4 X1-X3
  they win; the form stays (a key with an off value, or a proof that only named counters moved).
- The plan's own reading of the code (not measured): X1 `s9.guns_emplaced` counts infantry guns (`guns.rs` 1212-1223) while
  `guns_in_cover` counts AT-gun jobs only (`ai/guns.rs` 689-706): the T81 ratio lost its meaning when S11a1 turned the gun
  buy on. X2 `held_pots()` keeps the infantry-gun pot (up to 320 / 380 MP) from the general buy from 4:00, the minute
  vehicles unlock (`ai.rs` 1788). X3 no cap exists on a gun's counter-fire rounds (`ai/arty.rs` `run_arty` 558-563).
- NOT checked by the Plan stage (another session was playing match sets in the tree; no cargo, no CLI run): the targets
  file was parsed as TOML only (624 targets, 35 new, 21 runs; every new row names a run that exists). **Build A chunk 0
  runs `frontline-cli targets` on it and `tools/ablate.sh --dry-run` on the ablation folder, and fixes what they refuse.**
- Reference sets: REF0 `target/s11b-base40` (base, Build A chunk 0), REF1 `target/s11b-A0-40` (after the A0 fixes are on,
  Build A step 7). Every later neutrality proof is against REF1.
- Plan values with no design figure (all marked "verify" in the defs; S11b-7): every rifle-grenade number; howitzer
  charges above the first, traverse, complete-round weights; the 60 mm bomb's burst (scaled by mass); the light mortars'
  game ranges (scaled like the heavy tubes); `how_reserve_mp` 300; `rifle_grenade_gap_s` 15.
- Code map for Build A: `combat.rs` `fire_single_shot` (the pattern for G2-G3) and `base_class`; `game.rs` `SingleShot`,
  `kill_class`; `content/src/defs.rs` `ItemKind`; `ai/arty.rs` `arty_pot`, `held_pots`, `run_arty`, `counter_fire`.

## Plan (2026-11-19)
Scope (spec 0, 1), sized for three Build stages. (1) A0: three fixes from the S11a2 measure, each behind a key or proven
neutral: infantry-gun emplacements leave the AT-gun cover ratio (X1), the gun pot's hold can be released (X2, turned on only
by a rule on the 40 set), a per-gun counter-fire round budget of 50 % (X3). (2) Field howitzers leFH 18 and 105 mm M2A1 as
`inf_gun` cards with no tow: base guns that emplace where they arrive and fire flat; the AI buys one as a second gun from
8:00 inside the same 15 % income share (`how`). (3) Light mortars kz. GrW 42 and 60 mm M2 in the AI's mortar slot (`light`).
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
Decisions taken for the owner (all (A), `docs/TODO.md` S11b-1..12, "assumed 2026-11-19, owner asked not to be asked"): this
scope; no tow cards; the howitzer as a second AI gun inside one share; kz. GrW 42 at 181 MP; light mortar ranges 275 m and
370 m; grenadiers 2 and 1; plan values for the grenade; grenade kills inside the HE share; budget 50 %; the pot hold
released only on evidence; T81 without infantry guns; the artillery group of 2.
Honest expectation: the howitzers will fire little from the base (T154 reports it); rifle grenades favour the USA (two
launchers against one) with Germany at 41.67 %, so `rg_off` is the first ablation to read; the A0 causes are read from the
code, and the diagnosis may replace them.
Stage split: A (chunk 1: A0 fixes, SAVE 29 layout, keys; chunk 2: the rifle-grenade mode; every S11b switch 0) -> B (chunk
1: howitzers, their shell, light mortars; chunk 2: grenadiers, the group, pack howitzer data; all cards weight 0, default =
REF1) -> C (chunk 1: JSON, howitzer rows, the howitzer buy, `how` on; chunk 2: light mortars, grenades on, rows `rg` and
`light`, three 40 sets). Fallbacks in spec 8.
