# S10a-ai: fortification AI and balance

Status: spec (Plan, 2026-11-11). Branch `s10a-ai` from main `fa45332` (S10a mechanics merged, owner OK 2026-11-11).
Targets `docs/slices/targets/S10a-ai.toml`; log `docs/slices/S10a-ai-log.md` (each Build stage writes its hand-over).
AI-AND-BALANCE half of S10a (owner rule 2026-11-09): the skirmish AI uses the S10a mechanics through the four S10a
commands and the existing ones, and the game stays in balance. Built on every open owner answer's (A) (S9b-1..3,
S10a-1..11). Design: `roster-expansion.md` S10 done-when; `infantry-structures.md` 7.1, 7.2, 7.6, 7.7, A.5.

## 1. Scope

**In.** AI buys of the three engineer cards; AI materials (carried, trucked, crated); AI building of MG nests, wire,
sandbags at gun sites, dugouts with shelter under mortar fire, the HQ roof; AI wire knowledge and breaching (go round,
cut, Bangalore, suppress and smoke first, through the gap); the breach row driven by the AI (T93 real); S10a open items
1 (small wire gaps in path-finding, S17) and 2 (lost-material counter); CLI counters; ablation switches; balance.
**Out.** Mines, AT obstacles, bunker (S10b); presenter work; the truck Materials headless UI check (S10a open item 3:
presenter only, the AI does not depend on it; next presenter pass); S9b-1 US gun sight gap (section 3.6); structure
selection (S6-18); `[fortify]` and blueprint values (mechanics, owner-decided in S10a-1..11).

## 2. Baseline (S10a verify, `target/s10a-verify`, binary of `6d7b98c` = main `fa45332` code)

Germany pooled 48.75 % of 480 (52.50 / 46.25 / 47.50), no match without a winner, HE share 20.0 / 19.1 / 18.1.
282 pass, 12 miss, 10 hard: T8b 19.07 (floor 20), T31b 30.53 (cap 30), T40b 49.4, T40c 54.4, T40d 46.3, T72b 49.4
(floor 60), T54e 76.9 (80), T54f 70.6 (75), T88b 0.375, T88d 0.338 (0.45). Tick p99 0.413 all / 0.518 peak. s10 counters
all 0. Fortify rows: wire inside 20 %, gaps 100 %; roof 100 %; breach (scripted) defenders 7.0 %, attackers 88.7 %, gap
0 % (control 7.6 / 87.7). S3b row: defenders 2.7 %, attackers 71.5 %; S3c two bases of fire: attackers 41.4 / 40.8 %.

## 3. What gets built (all distances, times and shares in `[ai.fort]`, section 3.7)

### 3.1 Buying (`ai.rs` `buy`; Build B, except the urgent breach buy in C)

* The three cards get `ai_weight` above 0 and buy through the existing `engineer` mix slot (their first tag;
  S4 weight rule): Pioniergruppe / Engineer Squad keep 100; `ger_baupioniergruppe` 50; `ger_sturmpioniergruppe` 35;
  `usa_assault_section` 35 (S10a-12 (A)). The engineer slot weight (15) is unchanged.
* **Urgent breach buy** (C): while a breach is wanted (3.4) and no own squad with `breach` or `clear_wire` lives, the
  next buy is the faction's assault card (as `urgent_at`), at most one such squad alive (`breach_cards_max` 1).
* `ai_kind` stays Engineer for all; but trench digging takes only squads with `dig_full` or `build_structure`, and
  the own-point engineer bonus in `assign` skips `breach` squads (assault engineers go to attacks, not to digging).
* Switch `cards` 0: the three cards are skipped as at `ai_weight` 0 (ablation).

### 3.2 Materials (Build B)

* **Carried.** A builder squad (`build_structure` or `place_wire`) in its own base with a fort job (3.3) or carrying
  under `load_min_kg` gets `LoadMaterials` for its full carry (`carry_kg_per_man` x men: 160 kg for 8 men, 16 MP).
* **Budget.** Materials MP (`FortAccount::materials_micro`) stay under `materials_max_pct` (3) of the income earned
  (start + income per minute so far); over it nothing is loaded. Taken from the infantry budget after the gun pot.
* **Truck run.** A work in step `Wait` (no kg): an own supply truck idle in base (`ai::truck` Home) loads
  `truck_material_kg` (200), drives to within `material_reach_m` - 3 m of the site, stays until the work leaves Wait or
  `material_wait_s`, then returns to its habits. One run at a time per player.
* **Crate run.** No truck: an own bearer squad (S6) takes one `IssueCrate { Construction }` per man needed and sets it
  down at the site (the S6 crate path with the site as client). **Walk back:** no truck and no bearers: the builder walks
  to base, loads, returns (task `Refill`, the S5a staggering).
* Lost kg (dead carriers, wrecked trucks, removed works) are counted, never refunded (3.5).

### 3.3 Building (`ai/fort.rs`, new; Build B)

A **fort job** (`AiFortJob`, saved, keyed by builder squad) runs Load -> Go -> Order -> Watch -> Done. Its squad is
claimed (as hunts are): `order_infantry` never re-orders it, so no work is cancelled by a re-plan (S10a-6). Sites are
chosen once, by integer maths, held points in ascending index, ties by index; no HashMap order; no RNG at all.

| Work | When | Site (deterministic) | Limit |
|---|---|---|---|
| MG nest | own point, AI trench finished, point threat < `build_threat_max` | `nest_offset_m` (6) past the trench end chosen by point index parity, facing the trench line's enemy direction (`trench_line`'s `toward`) | 1 per point |
| Barbed wire | own point with nest done or started, same threat rule | one line parallel to the trench, `wire_out_m` (35, design 7.6 "30-50 m out") toward the enemy, `wire_len_m` (30) long, clamped to the map; refused spot: try 25 m, then give up | 1 line per point |
| Dugout | own point, trench done, point marked shelled (below) or `enemy_mortar_seen` | `dugout_back_m` (8) behind the trench middle, away from the enemy | 1 per point, 2 per player |
| HQ roof | own HQ built and unroofed, `enemy_mortar_seen` | `over_hq` (snaps) | 1 |
| Sandbag wall | own gun in Hold for `gun_hold_s` (60) | 2 segments (4 m) across the gun's arc, `gun_wall_m` (4) in front | 1 per gun |

* **Who.** Free `build_structure`/`place_wire` squads assigned to that own point (or within `builder_reach_m` 200 of a
  gun or the HQ), at most `builders_max` (2) jobs at once, at most `works_per_point` (3); a squad below 60 % strength
  or with a known enemy within `threat_radius` drops its job (the work is then removed by the mechanics).
* **MG team to its nest.** An MG team defending a point with a built own nest gets the nest centre as its Defend
  target (`order_infantry`), one team per nest.
* **Shelled.** A held point is marked shelled (`AiState::shelled`, tick by point) when at least `shelled_pct` (50) of
  its defenders are ducked or pinned while no known enemy soldier is within `shelled_clear_m` (150): fire it cannot
  see is indirect (S10a-15 (A)). `enemy_mortar_seen` is set once a known enemy soldier is in a `fire_mission` squad.
* **Shelter.** While a point with a built own dugout was marked shelled within `shelter_s` (20) and no known enemy is
  within `shelter_clear_m` (100), its defender squad nearest the dugout (one) moves into it; it goes back when the mark
  is older or an enemy comes within that range (counter `shelter_moves`).
* Switch `works` 0: no nest, dugout, roof or wall; `wire` 0: no wire. Both 0 and `cards` 0 and `breach` 0: the AI
  emits nothing new (proved by the all-off ablation, section 9).

### 3.4 Breaching (`ai/breach.rs`, new; Build C)

* **Knowing wire.** An enemy section becomes known when an own soldier within `wire_seen_m` (150) has a clear terrain
  line (1.5 m eye to 0.5 m over the section's middle); at most `wire_rays` (16) rays per plan, sections ascending from a
  saved cursor. `AiState::wire_known` (sorted ids, cap 256) forgets a section that no longer stands.
* **Want.** For each non-own point with assigned attackers: a known section within `breach_near_m` (80) of the point.
  One `find_foot_path` (existing node budget, wire costs) from the nearest attacker's lead to the point, at most one
  per point per `recheck_s` (60). Path crosses no known section: **go round** (A* already did; `wire_bypassed` += 1,
  no job). Else a **breach job** (`AiBreach`, saved, keyed by the breach squad) at the crossing point.
* **Breach squad.** Own squad with `breach` (Bangalore carried) first, else `clear_wire`, nearest; none: urgent buy.
* **Phases.** *Stage*: the breach squad holds `stage_m` (60) short of the crossing on the approach, crouched.
  *Suppress*: the point's base of fire (`ai::assault` fire-and-move; a second base if three squads) aims at the known
  defenders nearest the crossing, MG-team men and own-known MG nests first, not at the point's middle. *Smoke*: a man
  with smoke in the breach squad, else in an assigned squad within throw range, throws it at `smoke_frac_pct` (50) of
  the way from the crossing to that MG, capped at the smoke range. *Go*: `suppress_lead_s` (6) after the base fires
  or the smoke lands: `PlaceBangalore` at the crossing if carried and a known defender is within `cut_fire_m` (100) of
  it, else `CutWire`. *Retry*: no charge laid or cut done within `place_wait_s` (30): suppress and smoke again, resend,
  at most `breach_tries` (3). *Through*: gap open (no standing section within `gap_clear_m` (2) of the crossing): the
  job ends `opened`; attackers path through it (A*, with the S17 fix). Ends also: squad gone or under 50 %, point not
  attacked, `breach_max_s` (180).
* Switch `breach` 0: no knowledge, no jobs, no urgent buy (squads attack as on S10a; A* still prices wire).

### 3.5 Mechanics items this slice closes (Build A)

1. **S17 small gaps (in).** The AI's gap must be a path: A* pays the wire cost for a *step* whose segment comes within
   band/2 of a standing section (exact test, only for steps touching the coarse layer as prefilter), not for every
   node within band/2 + 2 m. Gaps from about 2 m up are found; no step crosses standing wire unpriced. Smoothing keeps
   its rule. Without wire nothing changes (neutral).
2. **Wire index (in, derived).** `in_wire` and the path layer read a 16 m cell index of sections (not saved, rebuilt
   when `(wire.len(), next_wire)` changes, also after load): AI wire makes 30-100 sections a match, and a full scan per
   man per tick is a p99 risk.
3. **Lost materials (in; the layout changes anyway).** `FortAccount::materials_lost_g` (carried kg trimmed on deaths,
   truck kg at a wreck, kg of removed works), `men_lost_in_wire` (an own man killed inside an enemy band),
   `works_done: Vec<u32>` (completed works by fort def), `materials_used_crate_g`, `materials_used_truck_g`.
4. **Truck Materials UI check (deferred):** presenter only; nothing in the AI path reads the HUD.

### 3.6 S9b-1 (US guns' sight gap, T88b/d): deferred

Not cheap here: it is a siting search (guns see armour 1.8 % of ready time), unrelated to fortifications, and a gun
change measured in the same 480 as the fort habits mixes two levers. A sandbag wall at a gun adds cover, not sight.
Defer to the next AI gun pass (S10a-17 (A)); T88b/d are carried and may not get worse (section 10).

### 3.7 `[ai.fort]` (`AiFortTuning`, a `tuning_table!`, `--set` works; values starting, Tune may move them)

Switches `works`, `wire`, `breach`, `cards` (1/0). `builders_max` 2, `works_per_point` 3, `build_threat_max` 4,
`builder_reach_m` 200, `load_min_kg` 40, `materials_max_pct` 3, `truck_material_kg` 200, `material_wait_s` 120,
`nest_offset_m` 6, `wire_out_m` 35, `wire_len_m` 30, `dugout_back_m` 8, `gun_hold_s` 60, `gun_wall_m` 4, `shelled_pct`
50, `shelled_clear_m` 150, `shelter_s` 20, `shelter_clear_m` 100, `wire_seen_m` 150, `wire_rays` 16, `breach_near_m`
80, `recheck_s` 60, `stage_m` 60, `smoke_frac_pct` 50, `suppress_lead_s` 6, `cut_fire_m` 100, `place_wait_s` 30,
`breach_tries` 3, `breach_max_s` 180, `gap_clear_m` 2, `breach_cards_max` 1. `docs/modding/tuning.md` lists them.

## 4. AI state, save, hash, protocol

```rust
pub struct AiFortJob { fort: u16, point: u8, line: Vec<FxVec2> /* <= MAX_FORT_POINTS */, phase: FortPhase,
    since: u64, work: Option<u32>, tries: u8 }                    // AiState::forts: BTreeMap<SquadId, _>, cap 16
pub struct AiBreach { point: u8, crossing: FxVec2, section: u32, phase: BreachPhase, since: u64, tries: u8,
    suppress_at: Option<u64>, smoke_at: Option<u64> }              // AiState::breaches: BTreeMap<SquadId, _>, cap 8
// AiState also: wire_known: Vec<u32> (cap 256), wire_cursor: u32, shelled: BTreeMap<u8, u64>,
// enemy_mortar_seen: bool, bypass: BTreeMap<u8, u64>, material_run: Option<(SquadId, u32)>
// AiCounters: fort_jobs, fort_refused, fort_build_plans, infantry_plans, shelter_moves, material_runs (truck,
// crate, walk), breaches_started, breaches_opened, breaches_given_up, breach_smokes, breach_suppress, wire_bypassed
```

**SAVE_VERSION 22 -> 23**: `AiState` (saved and hashed), `FortAccount` (3.5) and the `[ai.fort]` setup (saved, S9b
precedent) change layout. One layout commit at the start of Build A adds every field, enum and setup key of this spec;
no later stage changes the layout. `AiState`/`FortState` validation on load: caps, sorted `wire_known`, jobs only for
own live squads, `works_done.len() <= forts.len()`. **PROTOCOL_VERSION stays 13**: no new command, kind, body or limit;
the AI emits existing commands and its state is not on the wire (S9b precedent: SAVE bumped, PROTOCOL not).

**RNG.** The new AI code draws nothing from `AI_PLAN` (S9b decisions stay where they were); it needs no randomness.
If a Build stage finds it must, it uses a new purpose key `Purpose::AI_FORT` keyed (seed, tick, player).

## 5. Measures (CLI `skirmish.rs`, `json_out.rs`; Build A, numbers from B/C)

`factions.<f>.s10` gains (sums): `works_done.<def>`, `materials_lost_kg`, `materials_used_crate_kg`,
`materials_used_truck_kg`, `men_lost_in_wire`, the AI counters of section 4. `factions.<f>.s10_derived`:
`matches_with_works_pct` (a non-wire work completed), `matches_with_wire_pct`, `matches_with_gap_pct` (own
`wire_cut + wire_blown > 0`), `breach_when_enemy_wire_pct` (`breaches_opened > 0` over matches where the enemy built
wire), `builder_share_pct` (`fort_build_plans / infantry_plans`), `materials_mp_pct` (materials MP over income earned),
`materials_loaded_kg_per_match`, `fort_refused_per_match`, `new_cards_per_match`, `works_per_match`,
`wire_sections_per_match`, `breaches_started_per_match`, `breach_opened_pct`, `men_lost_in_wire_per_match`,
`materials_lost_kg_per_match`, `hq_roofed_pct` (of matches with an HQ built), `shelter_moves_per_match`,
`wire_bypassed_per_match`, `material_runs_per_match`. Text summary line "fort:" per faction; `tools/pool.py` prints the
four "uses it" shares and `builder_share_pct` in its block. Each derived field has a test on synthetic per-match records.

## 6. The breach row driven by the AI (fortify-test, `fortify/s10a.rs`; Build C) and T93

New row `breach_ai` beside the scripted `breach` (kept unchanged for comparison): the S10a fortified staging (trench
with a rifle squad, MG in an MG nest, 30 m wire 35 m out) against the AI's attack force: the assault card, an MG team
and a rifle squad (the S3c two-bases composition), 150 m out. A hook `World::drive_breach(player, breach, support,
point)` runs `ai::breach` alone (fresh `AiState` for the attacker, the planner's own phase code, commands through the
pending queue, one step per `replan_ticks`) until the job ends; then the row's scripted assault phase runs as in
`breach`. `control` = the same force and hook without wire or nest. Fields `gap_opened_pct`, `placed_pct` (charge laid
or cut done), `defender_loss_pct`, `attacker_loss_pct`, `control.*`.

**T93 bounds and why.** Hard: gap opened >= 60 % (roster done-when, 6 of 10); charge placed >= 60 %; attacker losses
<= 60 % (the two-bases row reads 41 %, wire and a nest add the walk up). Report: defender losses and the S3b bar
(T93a/b): the S3b row itself reads 2.7 / 71.5 % and the two-bases row 0-15 % defenders, so no AI work reaches
defenders >= 75 % before S11 artillery (S10a-16 (A)).

## 7. Tests (named; sim in `crates/sim/src/s10a_ai_tests.rs`, unit tests in the new modules)

**Build A:** `ai_fort_state_saves_loads_and_hashes`, `load_rejects_bad_ai_fort_state`,
`fort_account_counts_lost_materials` (dead carrier, wrecked truck, removed work), `men_killed_in_enemy_wire_are_counted`,
`path_finds_a_2m_and_a_4m_wire_gap`, `path_never_steps_across_standing_wire_beside_a_gap`,
`wire_index_matches_a_full_scan` (random sections, adds, removals, after load), `ai_fort_tuning_parses_and_validates`
(content), CLI `s10_ai_fields_zero_with_the_ai_unchanged`, `s10_derived_counts_matches_with_works_wire_gap` (synthetic
records, every derived field of section 5).
**Build B:** `ai_buys_engineer_cards_by_weight`, `ai_cards_switch_off_buys_none`,
`ai_builders_load_materials_in_base`, `ai_materials_budget_stops_loading`, `ai_builds_mg_nest_after_trench`,
`ai_lays_wire_across_the_approach`, `ai_does_not_build_under_threat`, `ai_builder_limits_hold`,
`ai_builder_is_not_reordered_mid_work`, `ai_never_orders_without_the_capability`, `ai_mg_team_mans_its_nest`,
`ai_sandbags_an_emplaced_gun`, `ai_marks_a_shelled_point_and_digs_a_dugout`, `ai_shelters_in_the_dugout_and_returns`,
`ai_roofs_its_hq_once_mortars_are_seen`, `ai_truck_brings_material_to_a_waiting_work`,
`ai_bearers_carry_a_construction_crate_to_a_waiting_work`, `ai_builder_walks_back_for_materials`,
`ai_fort_is_deterministic_across_threads_and_save_load` (1/2/3/8 threads, save/load at 5:00, as `ai_guns_...`).
**Build C:** `ai_learns_enemy_wire_only_by_sight`, `ai_goes_round_wire_when_the_path_avoids_it`,
`ai_breach_suppresses_the_nest_and_smokes_before_the_carrier_goes`, `ai_breach_blows_with_a_bangalore_else_cuts`,
`ai_breach_retries_a_pinned_carrier_then_gives_up`, `ai_assault_goes_through_the_gap`,
`ai_buys_an_assault_card_when_a_breach_is_wanted`, `ai_breach_is_deterministic_across_threads_and_save_load`,
`ai_all_switches_off_emit_nothing_new` (15 min AI-vs-AI: `FortState` empty, no new AI fields set),
CLI `fortify_breach_ai_row_runs_the_ai_job`.
Baselines (`resolved_defs.rs`, `simbench/report.rs`, canonical snapshot) refreshed with each TOML change.

## 8. Build stages (about 50 agent turns each; checkpoint commit after each step; hand-over in the log)

**Build A: layout, mechanics items, measures.** Layout commit (section 4, SAVE 23); `[ai.fort]` content and
`tuning.rs`; `path.rs` step test (S17), wire cell index (`fort.rs`), lost / in-wire / by-kind counters; CLI s10 and
s10_derived fields and pool.py lines; Build A tests. No AI behaviour and no `ai_weight` change yet. Ends: quick green;
`neutral.py` on the 40 set **same 40/40** against a 40-only base dir holding `target/s10a-verify/skirmish40.json`
(`target/s10a-base40` exists; the layout and S17 change nothing without wire). Hands to B: layout commit id, field
names, the 40-set line, path-search cost before and after (a 20 m line, 4 m gap).

**Build B: buying, materials, building.** `ai.rs` buy weights and the dig/assign rules (3.1), `ai/fort.rs` (new) jobs,
sites, shelter, MG to nest; `ai/truck.rs` material run; `ai/supply.rs` crate run; card `ai_weight`s; Build B tests.
Ends: quick green; a 40 set (`tools/batch.sh`, background) with T94, T95, T97, T98, T99 read and logged (40-set noise
noted). Hands to C: which works the AI completes how often, refusals, materials MP %, what was cut.

**Build C: breaching, the AI breach row, ablation tooling, docs.** `ai/breach.rs` (new), urgent breach buy, base of
fire aim at the nest, smoke (and the Sturmpioniere leader's `nbhgr39`, S10a-13 (A)); `World::drive_breach`, `breach_ai` row and JSON; override files
`docs/slices/targets/S10a-ai-ablations/{all_off,works_off,wire_off,breach_off,cards_off}.toml`; `tools/ablate.sh`
(runs measure.sh per file with `BASELINE` = the on-dir, prints one line per ablation); `docs/modding/tuning.md`,
`docs/modding/fortifications.md` "AI" section, STATE. Ends: `tools/check.sh tests` all pass (ignored included),
`frontline-cli targets S10a-ai.toml --runs fortify10` with T93f-h read, 40 set read. Hands to the orchestrator:
measure 480, run the ablations, verify.

Then: orchestrator measure + ablations -> Review -> Fix -> Tune only by the rule of section 9 -> verify -> Docs.

## 9. Ablations (by script; they decide whether Tune runs)

Each is a full `tools/measure.sh` 480 with one override, `BASELINE` = the slice head's measure dir:
1. `all_off` (works, wire, breach, cards = 0): `neutral.py` must say same 160/160 on all three sets against
   `target/s10a-verify` (the switches are clean; the S17 and index changes act only with wire).
2. `works_off`; 3. `wire_off`; 4. `breach_off`; 5. `cards_off` (existing engineers still build and cut).

**It matters** when an ablation moves pooled Germany by 2 points or more, or moves `men_lost_in_wire_per_match`,
attacker losses per match or median length beyond the 480 noise (pool.py prints the change). **Tune runs** only if:
(a) pooled Germany is outside 45-55 %, a match has no winner, a hard miss is added or one of the ten worsens past
section 10; or (b) an ablation moves pooled Germany by 4 points or more (a feature favours one side); or (c) a "uses
it" target (T94-T96, T100) misses and an `[ai.fort]` value is its cause. Tune knobs: `[ai.fort]`, the three new cards'
`ai_weight`, and their price, cooldown, cap within +-15 %. Never armour, penetration, guns, ammo, lethality, existing
cards' prices, `[fortify]` or blueprint defs. If every ablation moves less than 2 points the log says so (used, not
decisive) and no Tune runs.

## 10. Done when

1. T94, T95, T96 (uses it, 6 of 10), T97 (builders <= 12 %), T98 (materials <= 3 %), T99, T100 pass on both 160 sets.
2. T93f-h pass (AI breach row); T91, T92 still pass; fortify-test `pass` true.
3. Germany pooled 45-55 % over 480; no match without a winner.
4. No new hard miss; the ten carried may not move further out than: T8b 1.0 point, T31b 1.0 point, T40/T72 and T54
   5 points, T88b/d 0.05.
5. Clean tick p99 (verify.sh only) at most 25 % above 0.413 / 0.518 ms; T14a holds.
6. Every test of section 7 passes; SAVE 23, PROTOCOL 13; `all_off` ablation same 160/160 x3.

## 11. Risks (honest)

* **Breaching may still fail.** The scripted row lost every carrier 25-35 m short. Suppressing the nest needs a known
  nest (spotted MG men); smoke exists only in the US section and (if S10a-13 (A)) one German grenade. T93f at 60 % is
  the likeliest miss; the fallback is the owner's choice of a report bar.
* **T96 depends on the enemy's wire** lying on the approaches the attacker uses; matches with wire elsewhere count in
  the denominator. Few matches may qualify on one set (wide noise).
* **Balance.** Wire 35 m out and MG nests favour the holder; the AI is the attacker as often as the defender, but
  base side (T6) may shift. HE share (T8b) may fall further if men shelter in dugouts.
* **Own wire hinders own counter-attacks** (S10a-14 (A) has no lane); A* goes round at a cost.
* **Tick time.** Wire per man per tick and wire path steps; the cell index is the guard. Path searches in the breach
  want-check are capped (one per point per minute).
* **Builders as targets.** Builders at 35 m out under fire lose men and kg; `build_threat_max` may need Tune.
* **Scope.** Build B is the largest stage (five works, three material paths); the crate run is the first to drop to a
  report if B runs out of turns (then the walk-back covers it).
* **Shelled proxy** misreads hidden MG fire as mortar fire (it builds a dugout either way; harmless but wasteful).

## 12. Owner questions (work proceeds on each (A); also in `docs/TODO.md`)

- **S10a-12 Engineer card buys.** (A) In the engineer slot by weight: Pioniere 100, Baupioniere 50, Sturmpioniere and
  Assault Section 35, plus an urgent breach buy. (B) Assault cards only by the urgent rule (weight 0). (C) Own `fort` slot.
- **S10a-13 Smoke for the Sturmpioniere.** (A) The leader gets one `ww2:nbhgr39` (the Pioniergruppe's grenade), so both
  assault cards can screen a carrier. (B) No smoke; the German AI only suppresses.
- **S10a-14 Lanes in AI wire.** (A) One unbroken line; own men go round it. (B) A 6 m lane in the middle (now visible
  to path-finding), usable by the enemy too.
- **S10a-15 "Under mortar fire".** (A) The ducked/pinned proxy with no visible enemy near. (B) A sim-side record of HE
  bursts near own men (more saved state).
- **S10a-16 T93.** (A) Hard on the AI-driven row: gap >= 60 %, placed >= 60 %, attackers <= 60 %; the S3b bar stays a
  report until S11. (B) Keep all of T93 a report.
- **S10a-17 S9b-1 sight gap.** (A) Defer to the next AI gun pass. (B) Fold a sight-first re-site into this slice.
- **S10a-18 Materials budget.** (A) At most 3 % of income; builders load full at base. (B) No cap.
- **S10a-19 HQ roof.** (A) Roofed once enemy mortars are seen. (B) Always, as soon as the HQ stands.
