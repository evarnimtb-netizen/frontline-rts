# S4-variants log

Spec: `docs/slices/S4-variants.md`. Baseline: the spec's "Baseline" section
and appendix (40 hashes at `63c3e36`).

## Part A: content crate (no behaviour change)

### A1 Baseline tests (before any loader change)

- `sdk/crates/content/tests/resolved_defs.rs`:
  - `setup_bytes_baseline`: xxh3 of the postcard bytes of six resolved
    `MatchSetup`s (skirmish both faction orders, the test-unlock rules, the
    example override, a human-vs-AI request, and an override that patches a
    unit, a weapon and the AI mix), recorded with the old loader.
  - `legacy_view_matches_snapshot`: every def of `mods/ww2` as JSON,
    written by the old loader to `tests/snapshots/ww2_resolved.json`. The
    new loader must keep every key and value in it (it may add keys).

### A2-A6 Content crate (commit after this log entry)

- `sdk/crates/content/src/resolve.rs` (new): raw TOML tables per kind
  (`RawDefs`, kept as `ModDefs::raw`), `extends` on all nine def kinds
  (units, weapons, items, factions, maps, rules, vehicles, ammo, training),
  chains up to `MAX_EXTENDS_DEPTH` = 4, cycles listed, unknown parent with
  did-you-mean (or "is a <kind> def"), tables merge / arrays and values
  replace, `id` and `abstract` not inherited. Unit rules: a level with
  `[[members]]` drops inherited legacy soldier keys; legacy keys over a
  `[[members]]` parent are an error; an alias (`recall_cooldown_s` /
  `cooldown_s`, `initial_cooldown_s` / `unlock_s`) set on a level drops the
  other inherited name, both in one file is an error. Issues name the
  child's file and "(from `<parent>`)" for inherited values.
- `sdk/crates/content/src/variant.rs` (new): `MemberDef` (slot, count,
  weapon, sidearm, ammo, items, traits), traits and capabilities with the
  reserved lists, tags (4 reserved), `TrainingDef`, the exact legacy
  expansion and its inverse projection (`project_members`).
- `SquadDef`: `role` optional (derived from tags, vehicle or weapons when
  absent; a card needs `role` or `tags`), card block (`cooldown_s`,
  `unlock_s`, `max_alive`, `limit_group`, `available_from/to`, `tags`,
  `capabilities`, `training`, `ai_weight`, `short_name`, `sources`,
  `abstract`), `[[members]]`. After loading every card carries its
  canonical view (members expanded, tags and capabilities filled, derived
  `barrage` / `at_hunt`, `cooldown_s` / `unlock_s`); the legacy keys stay as
  written.
- Rules: `[call_in] date`, `[training] default`, `[limit_groups]`,
  `[ai.mix]` beside the old `mix_*_pct` keys (exactly one form, 1:1
  mapping). `pop_cap` unchanged (120).
- `mods/ww2`: `defs/training/{conscript,regular,veteran,elite}.toml`;
  `skirmish.toml` gains `date = ""`, `[training] default =
  "ww2:training_regular"`, an empty `[limit_groups]`. No unit file changed.
- Lowering: legacy cards lower exactly as before; a `[[members]]` card is
  projected onto today's `SquadTypeSetup`. `ModDefs::resolve` refuses with
  "needs S4 Part B" what today's sim cannot run (non-projectable members,
  member sidearms, non-regular training, capabilities that differ from the
  role's, `max_alive`, `limit_group`, a vehicle card's own cost/cooldown,
  `[ai.mix]` tags beyond the six legacy ones). Date filter on rosters is
  live (skirmish has no date). Abstract units never reach `squad_types`.
- Overrides patch the raw tables and every def re-resolves (a patch on a
  parent reaches its variants; the change list shows each); a
  `[[members]]` patch replaces a legacy squad's soldier keys;
  `ModDefs::add_defs_dir` loads extra def folders (variants);
  `add_rules_override` rules are kept raw too.
- Example variant `mods/ww2/tests/variants/units/usa_rifle_squad_understrength.toml`
  (extends + members) and `tests/variants/roster.toml`; `validate` loads
  them; `crates/sim/tests/variants.rs` calls it in (spawns in the call-in
  tick, 9 men) and fights a German rifle squad to the end.
- Schemas: `training.schema.json` new; every def schema has `extends`
  (a file with `extends` needs only `id`, via `if`/`then`).
- Docs: `docs/modding/variants.md` (new), `docs/modding/tuning.md` rows,
  `sdk/docs/net/relay-protocol.md` (the content hash changed on purpose;
  protocol unchanged).

### A7 Measure (release build of the Part A tree)

| Check | Result |
|---|---|
| `setup_bytes_baseline` (6 setups) | equal to the old loader's |
| legacy snapshot (`ww2_resolved.json`) | every old key and value kept |
| skirmish 40 (seeds 1-20 × both bases, `--jobs 3 --minutes 45`) | all 40 hashes equal to the appendix; summary identical except tick timing (p99 peak 0.455 ms, quieter machine than the 0.733 baseline) |
| armour-test `--stress --seeds 10` | all hard checks pass; report identical to the Plan baseline except timing lines |
| fortify-test `--seeds 10` | PASS; report identical except timing |
| golden ballistics | 46/46 |
| `validate mods/ww2` | ok (with and without the example variant) |
| `cargo test --workspace --release -- --include-ignored` | 486 passed, 0 failed |
| clippy `-D warnings`, fmt | clean |
| Godot (bridge rebuilt) | `check_project` OK, `test_data_reload` 8/8, `test_match_view` 215/215; settings.cfg unchanged |

Changed on purpose: `ModDefs::content_hash` (new def fields and kinds),
the schema list (`training`), one test expectation for it
(`sdk/crates/cli/tests/simulate.rs`).

## Part B: the sim runs members, capabilities, training; S4 content

### B1-B3 State, capabilities, members (one change, then measured)

- **Setup layout** (`sdk/crates/content/src/setup.rs`): `SquadTypeSetup`
  drops `role`, `weapon_men`, `sidearm`, `loadout` and gains `members`
  (`MemberSetup`: slot, count, weapon, sidearm, items by kind, trait bits,
  ammo), `men` (the sum), `weapon` (main: the first crew weapon, else the
  leader's), `small_arms` (focus: the longest-reaching non-crew weapon,
  S4-23), `items` (item per kind), `tags`, `caps` (`Capability` bits),
  `training`, `max_alive`, `limit_group`, `ai_weight`, `short_name`.
  `MatchSetup` gains `tags` and `training`; `RulesSetup` gains
  `limit_groups`; `AiSetup::mix` replaces `mix_pct`/`mix_vehicle_pct`/
  `mix_at_pct` (legacy tags first in their old order); `ItemRulesSetup`
  gains `single_shot_gap_ticks`; `WeaponSetup` gains `crew`,
  `short_name`; `ItemSetup` gains `weapon`. All checked by
  `MatchSetup::validate`. Part A's "needs S4 Part B" refusals are gone;
  only an unknown limit group and two items of one kind in a squad are
  refused (S4-26).
- **Sim state** (`SAVE_VERSION` 11 -> 12; no command change, so
  `PROTOCOL_VERSION` stays 5): `Kit::slot` and `Kit::weapon` (the man's own
  weapon) replace `Kit::leader`; `Squad::drops` (crew weapons waiting to be
  picked up) and `Squad::pzf` (Panzerfaust aim and gap); `Launcher` loses
  `handover` (its hand-over is a drop now); `PlayerState::s4` (pickups,
  Panzerfausts fired); `AiCounters` gains `bought` (per card),
  `pzf_hunts`, `capture_orders`, `pop_cap_holds`. All hashed, saved and
  checked on load (`world.rs` tests `s4_squad_and_player_state_...`,
  `item_state_...`). `ITEM_KINDS` 3 -> 4 (`single_shot_at`).
- **Capabilities and tags replace `SquadRole`** in the sim, AI, bridge,
  CLI and net harness: dig needs `dig_basic` (AT ditch `dig_full`),
  barrage `barrage`, re-crew `recrew`/`vehicle_crew`, capture and crew
  dismissal `vehicle_crew`; the AI keys its S1-S3c behaviours on an
  `AiKind` read from the tags; mortars by `barrage`. `SquadRole` stays
  the legacy TOML value (and the scenario helpers' argument, mapped to a
  tag; `scenarios::spawn_tag` is new).
- **Members in the sim**: spawn by slot (weapon, items), mortar fired by
  the first man carrying one, crew-weapon pickup (`combat.rs`
  `pick_up`; the AT launcher's hand-over is the same rule), fire weight
  by carried weapon, bail-out crews by slot.
- **Gate (old content, 40 set)**: every per-match summary line except the
  hash equal to the Plan baseline (40/40, no difference at all), so the
  S4-2 exception (tank-crew SMG) moved nothing.

### B4 Training, B5 call-in, B7 Panzerfaust, B8 AI

- **Training** (aim time, spread, reload, throw scatter, steadiness,
  suppression decay, sight, dig rate) from the card's profile; regular is
  exact identity. `s4_members.rs` staged case (Grenadiergruppe vs US rifle
  squad at 140 m, 60 s): US losses conscript 3, regular 4, elite 4.
- **Call-in**: per-card `unlock_s` (refused as `Locked`, the vehicle unlock
  a floor), `max_alive` (`MaxAlive`), limit groups (`GroupLimit`); the AI
  uses the same limits. Tests: spawn at once, own cooldowns (Q21 (B)),
  sibling card at once, `max_alive` frees up when a squad dies, limit
  group across cards.
- **Panzerfaust 60** (S3c-29 (A)): `defs/items/panzerfaust60.toml` (kind
  `single_shot_at`, fires as `defs/weapons/pzf60.toml`, round
  `defs/ammo/pzf60_heat.toml`: HEAT 200 mm, 45 m/s, jet and spall as the
  RPzB's). A carrier aims `windup_ms` (1.5 s; his own weapon idle) at a
  seen enemy vehicle within 60 m (40 m moving), then fires a physical round;
  one per squad per `items.single_shot_gap_s` (3 s); `AttackUnit` aims the
  nearest carrier first; refilled in base like items (S4-6 (A)). Tests:
  fires at 40 m, never at 90 m, same hash on 1/2/3/8 threads. armour-test
  case 16 (below).
- **AI**: tag mix (`[ai.mix]`), the card inside a tag by `ai_weight` share
  (S4-25), Panzerfaust hunts (`ai.assault.pzf_hunt_m` 100,
  `pzf_clear_m` 40), a squad with Panzerfausts counts as half an AT team
  for the urgent-AT buy, crews capture enemy abandoned tanks within
  `ai.armour.capture_m` 150 (S3c-33 (A)). The new tuning keys default
  when absent so older rules files load.
- **Report** (`skirmish --summary`): per faction the cards bought (matches
  with one / total), Panzerfausts fired / hit / pen / knock-outs (all and
  normal-base matches), Panzerfaust hunts, pickups per match, capture
  orders and captures, mean pop at 5/10/20 min, pop-cap holds.

### B6 Content switch

- Units (sources in each file): Grenadiergruppe (10: MP 40 leader, MG 42
  bipod team of 3, 6 Kar98k, 2 Panzerfaust 60; 320 MP, 20 s),
  Grenadiergruppe (Feldstärke) 7 (`extends` + members; 230; weight 50),
  sMG-Gruppe 6 (MG 42 on Lafette 42; 220, 30 s), Pioniergruppe 8 (280,
  30 s), US Rifle Squad 12 (11 M1 + BAR; 340, 20 s), understrength 9
  (260, weight 50), two-BAR 12 (360, 30 s, `available_from` 1944-08,
  weight 50), Light MG Section 5 (M1919A4 tripod; 200, 30 s), Engineer
  Squad 8 (280, 30 s); mortar teams 1 tube + 2 rifles/carbines; AT teams,
  tanks and crews moved to members / card keys unchanged. Replaced files:
  `ger_rifle_squad`, `ger_mg_team`, `usa_mg_team`.
- Weapons: `mg42_bipod`, `mg42` (now the Lafette 42 gun), the new
  `m1919a4_tripod` (`m1919` stays the Sherman's MG, S4-24), `bar_m1918a2`,
  `m1_carbine`, `pzf60`; `crew_weapon`, `short_name`, `sources` keys.
- Rules: `pop_cap` 150, `[ai.mix]`, `[items] single_shot_gap_s`,
  `[ai.armour] capture_m`, `[ai.assault] pzf_hunt_m` / `pzf_clear_m`.
  Start squads two full line squads and the engineers (Germany 28 men,
  USA 32).
- Example variant (T9): `mods/ww2/tests/variants/units/
  ger_grenadiergruppe_example.toml` (only `extends` + members, with an MG);
  `crates/sim/tests/variants.rs` calls it in, fights a US rifle squad and
  sees its crew assistant pick the MG up.
- Legacy guards: the S1-S3c content is frozen in
  `sdk/crates/content/tests/fixtures/ww2_s3c`; `ww2_resolved.json` (never
  re-blessed) and the setup-bytes test load it; the mechanics tests written
  for 8-man squads (abilities, soldier control, digging, combat, vehicle
  orders, bail, armour, anti-tank, AI vehicles, AI assault; the content
  tests `ww2_mod.rs`, `vehicles.rs` and the Part A loader tests) run on it.
  The live content: `validate`, the canonical snapshot (blessed at the
  switch), `variants.rs` (`the_s4_content_resolves_with_historical_squads`),
  `s4_members.rs`, `match_mode.rs`, `match_rules.rs`, the AI-vs-AI runs.

### B9 Bridge and HUD

- Bridge: catalog entries carry `short`, `tags`, `understrength`, `gloss`
  (tooltip "Grenadiergruppe — rifle squad, 10 men: ..."); squads carry
  `tags`, `capabilities`, `understrength`, `slot_names`; own soldiers carry
  `name` (role word), `weapon` (short name), `crew_weapon`, and
  `panzerfaust` in `items`; feed `crew_weapon_picked_up`; refusal words
  "N alive (max)", "<group> limit", "unlocks in m:ss" (S4-30). Contract:
  `godot/docs/match_state.md` "S4".
- HUD: one tile per card keyed by id; a men disc with the cooldown ring;
  the understrength half chevron; gloss tooltip; squad card rows by role
  word and weapon, the crew-weapon diamond, a "Pzf" mark; orders by
  capability; the Panzerfaust in the Satchel slot (S4-31) with a tool
  (click an enemy vehicle). Mock: two cards for rifle and MG, per-card
  cooldowns, capabilities.
- Screenshots (real match, `--capture_setup s4_card`):
  `s4_hud_card_1600.png`, `s4_hud_card_1280.png` (scratch, see report).

### B10 Measure (release build, quiet machine; scratch outputs in the stage report)

40 set = seeds 1-20 × both bases, `--jobs 3 --minutes 45`.

| # | Target | Result | |
|---|---|---|---|
| T1 | each faction 35-65 % (judged on 160) | 40 set: Germany 8 (20 %), USA 32; the 160 set was not run | miss on the 40 set (baseline 27.5 %) |
| T2 | no base side > 62 % | base 0 26 of 40 (65 %) | miss |
| T3 | 10-40 min, median 15-30 | 11.6-30.4, median 15.7, mean 17.1 | pass |
| T4 | HE share mean < 50 % | mean 45.0 %, max 64.0 % | pass |
| T5 | fortify, armour-test, golden, validate | armour-test all hard checks pass; golden 46/46; validate ok; **fortify-test FAIL**: barrage row trench 62 vs open 138 = 44.9 % (limit 40 %) | fortify miss |
| T6 | p99 peak < 1.5 ms; stress < 10 ms | peak 0.516 ms; armour-test case 14 p99 1.465 ms (max 2.775) | pass |
| T9 | variant of extends + members loads, plays, MG picked up | `variants.rs` | pass |
| T10 | call-in rules | `s4_members.rs`, `match_mode.rs` | pass |
| T11 | line cards ≥ 50 %, understrength ≥ 25 %, MG ≥ 75 % of matches | every line card in 38-40 of 40, understrength 40/40, MG 40/40 | pass |
| T12 | Panzerfausts in ≥ 10 of 20 normal-base matches; infantry AT ≥ 10 % of tank losses | Panzerfausts fired 6 in 4 of 40 matches (0 normal-base), 5 hit, 4 pen, 2 knock-outs; rocket and Panzerfaust knock-outs 13 of 166 tank losses (7.8 %; one satchel thrown at a vehicle) | miss |
| T13 | ≥ 1 capture per 10 matches; T1 ±5 vs capture off | 2 captures in 40 (7 capture orders G, 6 US); A/B on 160 not run | miss |
| T14 | pickup test + counter | staged tests; pickups 13.2 / match (Germany), 3.9 (USA) | pass |
| T15 | determinism | 1/2/3/8 threads (Panzerfaust, AI-vs-AI), save/load mid hand-over, net loopback, soaks: tests pass | pass |
| T16 | pop report | mean pop at 5/10/20 min: Germany 53/55/69, USA 60/74/86; pop-cap holds 0.0 / 0.2 per match | reported |

- Old content on the Part B build (frozen S3c mod, `--mods`): with
  `ai.armour.capture_m = 0` all 40 summary lines equal the Plan baseline;
  with the default 150 seven lines differ (one capture per side, S4-35).
- fortify-test A/B (barrage row): the S4 squads give 44.9 %; the same build
  with 8-rifle line squads (override) 24.2 %; a 3-tube mortar team (override)
  44.9 %. So the 10/12-man squads with their MG team and pistols lose more
  in the trench; the row is left failing for the Tune stage (not hidden).
  fortify T10 rows: Germany entrenched loses 8 % (2 bases: 90 %), USA
  entrenched 0 % (2 bases: 6.7 %).
- armour-test case 16 (Panzerfaust, 10 seeds): 40 m still 15 fired, 15 hit,
  15 penetrated, tank disabled in 8 of 10; 60 m still and both moving
  variants 0 fired (the carriers stand 0-13 m along a 27 m line, so a tank
  60 m from the line centre is beyond 60 m from them; a tank driving past at
  40 m is in moving reach only at its closest).
- Duel reports moved by chance (more men spawn before the staged tanks, so
  entity ids and random streams differ): 3a Panzer IV 10 / Sherman 10, 3f
  14 / 6.

## Tune (round 1)

Full account and numbers: `docs/slices/S4-variants.md` "Tune results".

- Data only: nine card prices, `ai.armour.capture_m` 300, trench depth
  180 cm, Kessel Ridge crest moved 12 m (the base-0 lean). No sim line
  changed (a Panzerfaust hunt inset was built, measured as no help and
  reverted).
- 40 set: Germany 16 / USA 24, base 0 18 / base 1 22, 12.3-27.5 min (median
  17.3), HE mean 43.7 %, tick p99 peak 1.030 ms (quiet). 160 set: Germany 69
  (43.1 %), base 0 79 (49.4 %), 10.1-28.8 min, HE mean 46.0 %.
- fortify-test PASS (barrage 33.8 %), armour-test all hard checks pass
  (stress p99 1.119 ms), 498 workspace tests, clippy and fmt clean.
- Still missed: T12 (Panzerfausts in 6 of 80 normal-base matches; rockets
  9.5 % of tank losses) and T13 (9 captures per 160 matches). TODO S4-36 to
  S4-40.

## Fix and close (2026-11-02)

Two reviews of `be11eb8` (determinism and correctness; balance, realism
and play) found no high-severity problem, 7 medium and 15 low findings.
Every medium is fixed or, for T12, moved to S5 with a reason; the cheap
lows are fixed. Scratch outputs:
`/private/tmp/claude-501/-Users-evarnimtb--claude-spares-spare-4LokPM/e922ae85-a8d5-4a23-bcc4-b43ebb0b094a/scratchpad/s4/fix/`.

### Findings and what was done

| # | Finding | Done |
|---|---|---|
| D-M1 | A crew card carrying a Panzerfaust (legal content) made saves that could not load: bailed-out crews were created with `pzf: None` | Fixed. `SingleShot::for_type` builds the state from the squad type for the call-in and the bail-out crew alike. Test `a_crew_with_a_panzerfaust_bails_out_and_its_saves_load` (save, load on 2 threads, 300 ticks, same hash). Crews may not carry launchers (content error). |
| D-M2 | The tag `crew` and the capability `vehicle_crew` were not tied; saves refused late in a match | Fixed. Content error "the tag `crew` and the capability `vehicle_crew` go together", with a hint either way. |
| D-M3 | A launcher man who is not the squad's main weapon never fired; legacy `weapon_men > 1` launcher cards gained idle men | Fixed. Refused: more than one launcher man, a launcher behind another crew weapon. Legacy launcher squads expand to one launcher man plus sidearms (the S3 rule). No shipped card changes. |
| D-M4 | Pop cap 150 but 120 ids per command: select-all orders lost men silently | Fixed. `MAX_UNITS` 150, `PROTOCOL_VERSION` 5 -> 6 (the one bump of the slice), relay doc updated; the bridge splits larger selections (mod pop caps) into commands of 150 (`unit_chunks`). Tests use `MAX_UNITS + 1` instead of a literal 121. |
| B-M1 | T12 missed; Panzerfausts near-inert in AI play (6 tubes per squad: 7 shots vs 4) | Not fixed in S4: the limit is AI behaviour (an ambush hold), not numbers. Moved to S5 with the Panzerfaust 100 (TODO S4-42); T12 is reported, not gated. |
| B-M2 | fortify-test T10 rows used only the line squad's men as the attacker denominator ("225 of 220") | Fixed: every attacking squad counts (now "225 of 330"). The hard check `attacker_losses * 2 <= attacker_men` uses the same totals. |
| B-M3 | The two line tiles could not be told apart ("Grenadie… ⌄ 7") | Fixed: short names "Gren. (F)", "Rifle Sq (U)", "Rifle 2BAR" (the men disc shows the count); the tooltip explains the chevron. Checked in the 1280 and 1600 autoplay screenshots. |
| D-L1 | armour-test case 9 passes only at `--seeds 10` (37.1 %); at the default 5 it fails (40.2 %) | Not fixed; known gap (TODO S4-47). Standard runs use `--seeds 10`. |
| D-L2 | Barrage and dig intents and the AI's point assignment keyed on tags or `dig_rate_pct` | Fixed: by capability (`barrage`, `dig_basic`); a barrage-capable squad is never also a point holder. |
| D-L3 | Load-check gaps | Fixed: a drop's weapon must be one of the squad's own members' weapons; `AiCounters::bought` at most one per squad type; no thrown single-shot AT wind-up. |
| D-L4 | Tests that asserted less than they said | Fixed: `ai_buy_counters_are_hashed_and_checked` (AI world, hash, length refused on load); the Panzerfaust gap checked tick by tick; the variant fight needs a German loss beyond the staged kill. |
| D-L5 | Crew-weapon pickup had no reach | Fixed: `items.crew_pickup_reach_m` = 30 m (rules, default 30); the weapon waits where it fell until a man is that close. No shipped match changed outcome (below). |
| D-L6 | Doc drift | Fixed: `SAVE_VERSION` 12 described; `small_arms` documented as the longest-reaching non-crew weapon. |
| B-L4 | The squad card showed no weapons; strip names were role words | Fixed: squads are named after their card ("Grenadiere 2", "sMG-Gruppe 1"); rows show role and weapon when they fit, in the small size next, and a plain rifleman shows his weapon ("Kar98k"). Leaders and crew roles keep the role word (both are in the tooltip). |
| B-L5 | Grenadiergruppe citation said MG-Schütze 2 with Kar98k, the file gives a P 38 | Citation fixed, content kept: the classic MG team is gunner and assistant with pistols, third man with a rifle (German drill as summarised in secondary sources; miniatures.de "German Infantry Section 1937"); the K.St.N. 131V Kar98k is the late-war Volksgrenadier table (S5). Roster doc row corrected. TM-E 30-451's squad section was not read. |
| B-L6 | False price comment on the sMG-Gruppe; the US LMG price looks inverted against its weapon | Comment corrected; the US Light MG Section's 280 is documented as a balance price (TODO S4-41). |
| B-L7 | armour-test case 16 never exercised the 60 m range | Fixed: distances from the first Panzerfaust carrier, 40 / 55 m still and 35 / 55 m moving. 55 m still now fires (15 fired, 15 hit, 14 penetrated). |
| B-L8 | Ridge offset past symmetric may overshoot in mirror matches | Recorded as a deliberate compensation (TODO S4-44). |
| B-L9 | `capture_m` 500 not tried | Not run (a Tune experiment); T13 stays a miss (TODO S4-37, S4-40). |
| B (note) | Dug-in squads nearly immune to direct fire (1.8 %) | Recorded as an owner design call (TODO S4-43). |

### Hashes

The Fix adds one rules field to the setup, and the setup bytes are part of
the match hash, so all 40 hashes changed. Every one of the 40 matches has
the same winner and length as in the Tune table (checked row by row); the
40-set summary is the same line for line except the tick timing. New
hashes (seed, n = normal / s = swapped bases):

```
1 n 0xd0773e02bfe471b1; 1 s 0xfd4d2cb28645202c; 2 n 0x49965d0c17bfe32d;
2 s 0xab6fb50c2e8e4763; 3 n 0x0aa138427c708ce3; 3 s 0x4809139e567c1703;
4 n 0xd08ac3e5dc0702e6; 4 s 0x8fd99f1ccd22585b; 5 n 0x3ed42469d989fcca;
5 s 0x24a9c6cdd4aacadd; 6 n 0x29c5e0bab872114c; 6 s 0x29c7add7936cdc6a;
7 n 0xdf3a6483b5587da8; 7 s 0x1019a147c86a64a4; 8 n 0xc741617e4283e5ee;
8 s 0x8b4bc0aa120f389b; 9 n 0xbd468b7d1a812e6b; 9 s 0x9f25a600e8808470;
10 n 0x9c87f7727cb35115; 10 s 0xe000fa392d58422c; 11 n
0x0f33e60077fd2c75; 11 s 0x6e867c81fd243dd4; 12 n 0x64ff71a4d80396b7;
12 s 0x7cd9ce74917e1b0a; 13 n 0xc6559b14d6edeee2; 13 s
0x001f8f89d7564f97; 14 n 0x0ba341b5ac5bfc2e; 14 s 0x614e65a1b76c50c1;
15 n 0xbc9e308487ad1639; 15 s 0xf94d0f971fa0cc18; 16 n
0x6466385f72a9bf16; 16 s 0xdde46f38e3d1a459; 17 n 0x0e8895047f00b396;
17 s 0x66c5af819b883509; 18 n 0xc8bbd521f2df2154; 18 s
0xca95c691b3cf1e91; 19 n 0x57a679a2c96b9965; 19 s 0xc96e47e0aff361d7;
20 n 0x4f67f5a383319cae; 20 s 0xb4a2a5d72f29eb8e
```

The frozen S3c setup-bytes baseline moved for the same field (old and new
values in `resolved_defs.rs`); `ww2_canonical.json` re-blessed (three
short names, two source lists, the new rules key). `ww2_resolved.json`
is unchanged.

### Final targets (release build; 40 set = seeds 1-20 × both bases, `--jobs 3 --minutes 45`)

| # | Target | Result | |
|---|---|---|---|
| T1 | each faction 35-65 % (judged on 160, S4-11) | 40 set: Germany 16 (40 %), USA 24 (60 %). 160 set (Tune build, same outcomes per match on the 40 set): Germany 69 (43.1 %); out-of-sample seeds 101-180: 69 (43.1 %) | pass |
| T2 | no base side > 62 % | base 0 18, base 1 22 (55 %); 160 set 79 / 81 | pass |
| T3 | no match < 10 or > 40 min, median 15-30 | 12.3-27.5 min, median 17.2, mean 17.7 | pass |
| T4 | HE share mean < 50 % | mean 43.7 %, max 55.2 % | pass |
| T5 | fortify, armour-test, golden, validate, tests | fortify-test `--seeds 10` PASS (barrage 46 vs 136 = 33.8 %, rifles 0 vs 170); armour-test `--stress --seeds 10` all hard checks pass; golden 46 rows, 0 failed; `validate mods/ww2` ok; 498 workspace tests pass, 0 fail, 3 ignored (the 2 net soaks pass when run); clippy `-D warnings` and fmt clean | pass |
| T6 | tick p99 peak < 1.5 ms (--jobs 3); stress inside budget | 40-set peak 0.987 ms (quiet machine); armour-test case 14 p99 1.102 ms, max 2.754 ms (< 10 ms) | pass |
| T9 | a variant of extends + members loads and plays | `variants.rs`, `mods/ww2/tests` example | pass |
| T10 | call-in at once, per-card cooldowns, nothing queued | `s4_members.rs`, `match_mode.rs` | pass |
| T11 | new cards bought | Grenadiergruppe, Feldstärke, sMG-Gruppe, Rifle Squad, understrength, Light MG in 40/40; two-BAR 37/40 | pass |
| T12 | Panzerfausts in ≥ 10 of 20 normal-base matches; infantry AT ≥ 10 % of tank losses | Panzerfausts: 4 fired in 4 of 40 matches (3 of 20 normal-base); infantry AT (rocket and Panzerfaust knock-outs): 17 of 157 tank losses (10.8 %, met) | **miss, moved to S5** (S4-42) |
| T13 | ≥ 1 capture per 10 matches | 3 captures in 40 (all US), 31 capture orders | **miss** (S4-37) |
| T14-T15 | pickup, determinism | pickups 13.9 / match (Germany), 4.2 (USA); thread, save/load and net soak tests pass | pass |
| T16 | pop report | mean pop at 5/10/20 min: Germany 60/65/74, USA 54/69/80; pop-cap holds 0.0 | reported |

Godot (bridge rebuilt): `check_project` OK, `test_match_view` 237/237,
`test_net_ui` 143/143, `test_data_reload` 13/13. `--autoplay 10` vs AI at
4x (seed default, Germany): no script errors; the run ends with a
surrender at 10:00 as scripted; `s1b_frag_tool` and `s1b_frag_trench`
missing as in S3c (they need an enemy trench in throw reach), so the
script exits 3. Screenshots looked at (HUD 1600 and 1280, battle): the
Feldstärke tile reads "Gren. (F)" beside "Grenadiere", strip cards read
"Grenadiere 1", "sMG-Gruppe 1", squad-card rows "Kar98k  Pzf 100",
feed "MG 42 picked up by Rifleman (Pzf)". `settings.cfg` checksum equal
to the backup before and after.

### Open owner questions

All on their (A) defaults: S4-1 to S4-46 in `docs/TODO.md` (S4-41 to
S4-46 new in this stage: US LMG price, T12 to S5, trench direct-fire
immunity, the ridge offset, pickup reach, selections above 150), plus
S4-47 (armour-test case 9 margin).

### Known gaps

- T12 (Panzerfaust use) and T13 (AI captures) miss; the fixes are AI work
  (ambush hold, a free vehicle slot), planned for S5.
- armour-test case 9 passes at `--seeds 10` (37.1 %) but not at the
  default 5 seeds (40.2 %, limit 40 %); the S3c baseline was 12.6 % with
  8-man squads. Not investigated.
- fortify-test T10 rows still "missed": dug-in squads lose 1.8-5.5 % to MG
  plus rifles (owner call S4-43).
- Squad-card rows show the weapon only for riflemen at 1280 and 1600; the
  MG gunner's and leader's weapon is in the tooltip only.
- Citations not read at the source: K.St.N. 131 (1944), TM-E 30-451 squad
  section, T/O&E 7-17 (US squad), BAR / M1919A4 / carbine figures (values
  match common references).
- Long selections above 150 men (mod pop caps) move as several formations.
