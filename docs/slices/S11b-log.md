# S11b log (field howitzers as base guns, light mortars, HE rifle grenades, A0 fixes; at most 150 lines)
Spec: common `docs/slices/S11b.md`, stages `S11b-build-A.md`, `-B.md`, `-C.md`; targets `targets/S11b.toml`; ablations
`targets/S11b-ablations/`. Branch `claude/project-thread-e7nsak`, base `<BASE>` = `8d2731b` (also main). Another agent may
work in this checkout: commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked
`.claude/`, `target/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: Build A done (both chunks; commits `3edf0cc` code, then the log). Next: Build B (`S11b-build-B.md`).**
- SAVE 29, PROTOCOL 15 are in; B and C change no layout. Every S11b switch is 0 in `skirmish.toml`.
- **REF1 = `target/s11b-A0-40`** (the whole of stage A, every switch 0; Germany 19/40). Prove B against it with
  `python3 -I tools/neutral.py target/s11b-A0-40 <B40>`: it must say `same 40/40` with nothing after it.
- Rifle-grenade ids: item `ww2:m7_rifle_grenade` (kind `rifle_grenade`), weapon `ww2:m7_launcher` (class
  launcher, 50 m/s, 15 mrad, range 180), ammo `ww2:m17_rifle_grenade` (mechanism `he`, Mk 2 `[he]`, shells
  680 g). A member slot carries them as any item: `[[members]] ... items = { "ww2:m7_rifle_grenade" = 3 }`
  (with `rifle_grenades` 0 the setup drops the kind: no item index, counts 0, no `rg` state).
- Item def limits (rifle_grenade only): `weapon` required (a launcher whose ammo is `he` with `[he]`),
  `supply.kind = "shells"`, `range_m` 1..=300 (other kinds 1..=100), `stow_ms` 0..=60000 (others 0),
  `min_range_m` < `range_m`, `windup_ms` <= 10000. German def C5: copy the three US files' shape.
- Gun def limits (vehicle.rs `validate_s9`): `push_men` 1..=16, `emplace_s`, `pack_s`, `hitch_s`,
  `relay_trail_s` 1..=600, `tow_bar_cm` 50..=1200; a towed gun may have `speed_cm_s` 0 and `reverse_cm_s`
  0 (too heavy to push: the 3-inch M5 pattern, use it for C1/C2 "no push"). Mortar `[indirect]`: the
  lowest charge's shortest high-angle range must be <= 1.1 x `min_range_m` (defs.rs, the hint prints it).
- Adding defs shifts sorted indices (items, weapons, ammo, tags): stage A added three defs and REF1 played
  identically, so index shifts alone did not move a match; B's neutral run is still the proof.
- Not done in A (by rule id): none of G1-G4, X1, X3. `skirmish.rs` HE share untouched (Build C M1).
  `ai/ammo.rs` (H3) untouched. HQ, dump and crate service (`structures.rs`) still refill rifle grenades
  (crates are their designed source); only the base (`refill_men`, the legacy free refill) leaves them out.
- `tools/neutral.py` does not yet skip `s11b` keys (Build C M1) nor allow `s5a.kills_by` to grow: REF0
  vs REF1 shows `kills_by: 11 items -> 12` (the new class, a 0 tail). Compare later sets to REF1, not REF0.
- Test helpers B/C may reuse: `s11b_tests::arm(setup, card, slot, n)` (gives a slot grenades and sets the
  switch), `rg_scene`; `s11a2_ai_tests` `cf_scene`, `plans`, `Sent`, `add_shot` are now `pub(super)`.

## Build A (2026-11-19; commits `3edf0cc`, log commit after it)
Chunk 0: release CLI current; REF0 `target/s11b-base40` from the batch cache (base `8d2731b`), Germany 19/40.
Parse check: `frontline-cli targets docs/slices/targets/S11b.toml --from-json target/s11b-base40` loads
(21 pass, 1 miss T149d report, 4 key-missing, 598 skipped): no parse error, the file is unchanged.
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
Aim error, spread 0 (unit integration of the sim's substeps): 60 m 2 mm (5 m up 10 mm), 120 m 2 mm (5 mm),
180 m 3 mm (1 mm); in the match a burst lands within 3 m of the aim point. Control: the flat
`gunnery::solution` aim misses by over 8 m at 150 m.
X1 (REF0 -> REF1): `s9_derived.gun_in_cover_pct` Germany 59.286 -> 86.458, USA 57.627 -> 87.179;
`s9.guns_emplaced` 140 -> 96 / 118 -> 78; `s9.gun_crew_lost` 92 -> 52 / 79 -> 67 (per match 2.3 -> 1.3,
1.975 -> 1.675). `neutral.py base40 A0-40`: "same 40/40" per match; the aggregate keys that differ are exactly
`s9.{guns_emplaced,gun_crew_lost}`, their `s8a.s9` copies, `s9_derived.{guns_emplaced_per_match,
gun_crew_lost_per_match,gun_in_cover_pct}` and `s5a.kills_by` 11 -> 12 items (tail 0, the new class).
REF1 holds all of stage A (not X1 alone): the stronger proof, and the base B and C need. Set 3
`target/s11b-A40` = default with `-o S11b-ablations/s11b_off.toml`: `neutral.py A0-40 A40` "same 40/40
(note: overrides 0 -> 1 items)". `cb_alternate` is not measured here (diagnosis 62.3 -> 47.3 %).
Tests: `s11b_tests` 17 (layout, load rejects 17 cases, X1, X3 x3, G x10), `s11b_content` 3.
Checks: `cargo build --release --workspace` ok (bridge included). `tools/check.sh quick 8d2731b`: fmt, clippy,
neutral.py, the five ablate dry runs (S11b's new) ok; tests 1112 passed, 1 failed: `udp`
`ipv6_loopback_handshake` ("Address family not supported": this container has no IPv6; not S11b code).

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
