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
