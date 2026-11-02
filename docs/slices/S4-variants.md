# Slice S4-variants: the variant system and historical squads

Branch `s4-variants` (from `s3c-armour` at `37539a5`; `design-roster`
merged at `63c3e36`, docs only). Roadmap: `docs/design/roster/roster-expansion.md`
§2 (variant system), §3.1/§3.2 (rows marked S4), §4.2 "S4", "Owner
decisions (2026-11-01)" and §5; infantry detail in
`docs/design/roster/infantry-structures.md` §2, §3.

Scope, as approved by the owner (2026-11-01):

- `extends` for every def kind; the card block; `[[members]]` slots;
  capabilities and tags instead of `SquadRole`; training profiles;
  historical squad sizes, cheaper understrength cards, pop cap 150
  (Q6 (A)); every card its own cooldown (Q21 (B), `cooldown_group` unset).
- Content: the S4 rows of the roster tables, four new weapon defs, the
  Panzerfaust 60 as a squad item (S3c-29 (A)), AI capture of abandoned
  enemy tanks (S3c-33 (A)).
- Not in S4: StG 44 / VG-Sturmgruppe (S5, needs counted ammo), rifle
  grenades (S11), any new HE, Waffen-SS (Q15 (A)), in-match veterancy.

---

## Baseline (measured 2026-11-02 on `63c3e36`, release build of `37539a5` + docs)

Raw outputs in the stage scratchpad (`s4/skirmish40.txt`, `armour.txt`,
`fortify.txt`); numbers copied here.

### AI vs AI: 40 matches (seeds 1–20 × both base orders, `--jobs 3 --minutes 45`)

| Measure | Value |
|---|---|
| Wins | Germany 11 (27.5 %), USA 29 (72.5 %), no draws |
| Wins by base | base 0 21 (52.5 %), base 1 19 (47.5 %) |
| Length | 12:17–31:42, mean 18.0 min, median 16:41; 3 over 25 min |
| HE share of kills | mean 43.0 %, max 56.2 % |
| Kills / losses | Germany 3766 / 4101, USA 4101 / 3766 |
| Tanks lost / bought | Germany 86 / 135, USA 101 / 154; 187 losses (abandoned 161, rack 26 = 14 %) |
| Rockets fired / hit / pen | Germany 56 / 42 / 41 (20 of 40 matches), USA 57 / 33 / 29 (21 of 40) |
| Satchels at a vehicle | 1 + 1 (2 of 40 matches) |
| Captures | 0 (the AI never captures, S3c-33) |
| Tick p99 | peak 0.733 ms, all 0.700 ms (an orchestrator batch at `--jobs 2` ran alongside for part of the run; re-measure in a quiet machine before judging T6) |
| Wall time | 82.6 s |

The 40-match set already **misses** the faction target (Germany 27.5 %),
exactly as the S3c log records: seed block 1–20 is an outlier; blocks
21–40, 41–60, 61–80 give Germany 22 / 21 / 18 of 40, and the 160-match
set (seeds 1–80) gives Germany 45 %, USA 55 %. S4 judges faction wins on
the 160 set (S4-11 (A)) and reports the 40 set.

Per-match final hashes of the 40 set are the **Part A equality list**
(appendix "Baseline hashes").

### armour-test `--stress --seeds 10`

**All hard checks pass** (478 s; case 12 took 358 s of it while other
runs shared the machine).

| Case | Result |
|---|---|
| 1 penetration statistics | pass, 2080 hits, 0 mismatches |
| 3a 300 m frontal | Panzer IV 6, Sherman 12, none 2 of 20 |
| 3f 600 m frontal | Panzer IV 14, Sherman 6 of 20 |
| 3c / 3e flank | flanker 20 of 20 both |
| 3d crossing vs stationary | 60.0 % |
| 4 AT team | flank disabling 48.1 %, front 48.4 % |
| 5b tank on Suppress | pinned or ducked 96.7 %, kills 4 |
| 6 tank vs rifles + AT | disabled 16 of 20 (80 %) |
| 7 supported assault | attackers 28 vs 160 (17.5 % of 7b), defenders 100 % |
| 8 satchels | 40 m with rifles 7 of 20; MGs live 0 of 20; in the trench 20 of 20 |
| 9 tank HE | trench 19 vs open 151 men; lip 0.035 m per burst |
| 14 stress (100 vehicles, 650 soldiers, limits lifted) | **tick p99 1.439 ms**, max 2.620 ms (3 threads; intent 0.729, weapons 0.460) |
| 15 bail-out | 7 of 7 checks, 20 of 20 runs |

### fortify-test `--seeds 10`

**PASS** (51 s).

| Row | Result |
|---|---|
| rifles: trench vs open at 150 m | 0 vs 160 (limit 50 %) |
| 81 mm barrage: trench vs open | 44 vs 128 = 34.4 % (limit 40 %) |
| grenade assault from 25 m | defenders 160 lost with grenades vs 30 without |
| smoke vs MG | pass |
| T10 MG + rifles vs a dug-in squad | defenders 90.0 %, attackers 51.2 % (missed, as in S3c); USA attacking 66.2 % (37.5 % with two bases of fire), Germany attacking 36.2 % |
| vs a trench with its own MG | defenders 0.9 %, attackers 98.8 % |

fortify-test and armour-test spawn "the rifle squad" and "the MG team"
by role (`scenarios.rs`). After the content switch they spawn the
faction's highest-weight `line` and `mg` cards, so their squads grow
(10/12 men, one MG per line squad, one tripod MG per MG card): the S1
threshold rows stay hard checks (ratios), the T10 and assault rows are
re-baselined at B6 and reported, not compared with the numbers above.

---

## What exists today (code map)

### The S3b call-in is already most of the "call-in rework"

Since S3b a call-in buys at once and spawns at base (no queue, no
delivery time); each type has `recall_cooldown_s` and
`initial_cooldown_s`; types naming the same optional `cooldown_group`
share a timer (`game.rs:671` `call_in_check`, `game.rs:1008` cooldown
start, `ai.rs:850` AI readiness). Vehicles also obey the rules'
`[vehicles] unlock_s` (one value) and `limit` (alive vehicles,
`game.rs:687-696`). What S4 adds: `max_alive`, named `limit_group`s,
per-card `unlock_s`, dates, tags, capabilities, training, and the field
names of the card block.

### Legacy unit fields (content crate)

`SquadDef` (`sdk/crates/content/src/defs.rs:53-128`): `role`, `vehicle`,
`men`, `weapon`, `weapon_men`, `sidearm`, `cost`, `recall_cooldown_s`,
`initial_cooldown_s`, `cooldown_group`, `hp`, `speed_cm_s`, `sight_m`,
`spacing_dm`, `dig_rate_pct`, `steadiness_pct`, `loadout`
(`LoadoutDef` `item` / `count` / `leader_count`, `defs.rs:133`).

| Use | Where |
|---|---|
| Per-file validation (vehicle squads must not set soldier fields, soldier squads must) | `defs.rs:986-1050` |
| Cross-checks (weapon, sidearm, loadout ids; mortar role needs a mortar weapon; AT role needs a launcher; crew squads not in rosters) | `modset.rs:451-541`, `modset.rs:641-669` |
| Lowering into `SquadTypeSetup` | `setup.rs:868-916` (vehicle squads `vehicle.rs:2082-2086`) |
| Setup validation on load | `setup.rs:1720-1835` |
| Spawn: man k carries `weapon` if k < `weapon_men`, else `sidearm`; leader items for k = 0 | `game.rs:1178-1191`, `bail.rs:650-671` |
| Weapon by **current position** in `members` | `combat.rs:229-242` (`weapon_of`), `combat.rs:244` (`small_arms_of`) |
| Launcher hand-over (AT teams only) | `combat.rs:878`, `combat.rs:2174`, `game.rs:1575` |
| Item kits (`full_kit`, leader flag) | `setup.rs:670-690`, `items.rs:187-191, 289, 321, 470, 559` |
| Dig rate | `dig.rs:374`, `dig.rs:468`, `game.rs:923`, `unit.rs:349` |
| Steadiness | `combat.rs:180`, `combat.rs:2257` |

**A legacy fact the content switch must handle:** with no `sidearm`, every
man carries `weapon` (`weapon_of` returns `ty.weapon`). Today's MG teams
(`ger_mg_team`, `usa_mg_team`, `men = 3`) therefore field **three MG 42s /
three M1919s** each, and every man of a mortar team carries the mortar
(only the formation leader fires it, `combat.rs:709`; the other two have
no small arm). The legacy expansion keeps this exactly (one slot,
`count = 3`) so hashes stay equal; the new sMG-Gruppe and Light MG
Section have one gun and riflemen, which cuts an MG card's firepower to
about a third. This is the largest balance move of the slice (S4-2).

### `SquadRole` uses (every one)

| File:line | What the role decides | Replaced by |
|---|---|---|
| `defs.rs:36-48, 60, 993` | the enum; `vehicle` squads skip soldier fields | tags + capabilities; "vehicle squad" = has `vehicle` |
| `modset.rs:451, 480, 488` | vehicle squad check; mortar role needs a mortar weapon; AT role needs a launcher | derived from members' weapons |
| `modset.rs:641, 652, 669` | crew squads not in a roster / start squads; a vehicle's `crew_squad` must be a crew | tag `crew` (reserved) |
| `setup.rs:317, 623, 868, 891, 1734, 1824, 1835` | `AiSetup::mix_pct` in role order; `SquadTypeSetup::role`; validation of vehicle and crew squads | tag ids + capability bits in `SquadTypeSetup` |
| `game.rs:808-812` (`digs_profile`), `923`, `unit.rs:349` | AT ditch only for `Engineer` | capability `dig_full` (profile `engineers_only` → `needs = "dig_full"`) |
| `game.rs:965` | Barrage only for `Mortar` | capability `barrage` (derived: a member with a mortar weapon) |
| `game.rs:1742` | crew squads dismissed in base | capability `vehicle_crew` |
| `bail.rs:193-196` (`can_crew`), `231-239`, `852`, `1013` | who may re-crew (crew yes, AT and vehicle no, others by `any_infantry`); capture needs a crew; crew counters | capabilities `vehicle_crew` (trained), `recrew` (untrained) |
| `ai.rs:495-510` (`role_index`), `555`, `675`, `694`, `820-825`, `949` | the buy mix and its counts | `[ai.mix]` by tag |
| `ai.rs:1025-1041` | point scores by role (mortar, AT, engineer, MG) | tags `mortar`, `at`, `engineer`, `mg` |
| `ai.rs:1098, 1248, 1652, 1750, 1861` | engineers dig; mortars barrage; rifles and engineers assault; engineers satchel | capability `dig_basic` + tag `engineer`; `barrage`; tags `line` / `engineer`; satchel in the kit |
| `ai/assault.rs:235, 250-290, 374, 451, 526-572` | base of fire (MG first, then rifles), assault squads, AT and satchel hunts | tags `mg`, `line`, `engineer`, `at`; a Panzerfaust in the kit joins the AT hunt |
| `godot-bridge/intents.rs:42-63` (`role_name`, `call_in_type`) | the presenter's `type` string; call-in **by role** | `type` kept as a derived legacy string; call-in by card id (`call_in_by_id`, already there) |
| `godot-bridge/match_node.rs:531, 1124, 1659, 1875, 2412, 2602`; `match_frame.rs:458, 1044` | squad `type`, mortar lookups, MG setup flag, rifle lookups | capabilities / tags in the frame; `type` kept |
| `cli/skirmish.rs:438, 844-868`; `cli/s3c_tally.rs:188-384`; `cli/armour_test.rs:2116-2375` | report counters and staged spawns | tags |
| `net/harness.rs:577-604` | scripted re-crew and dig | capabilities |
| `sim/scenarios.rs` (25 uses), `sim/tests/*` (16), `content/tests/*` (12) | staged spawns by role | a `spawn_tag` helper; `SquadRole` stays as a test-only legacy alias |

Godot: `call_in_panel.gd` indexes the catalog by `type`
(`index_of`, line 184) and sends `type`; with two cards per role that is
ambiguous, so the HUD moves to the card id. `orders_grid.gd` shows Dig,
Barrage, Crew by the presenter's role list. The mock and autoplay
scripts (`mock_match_source.gd`, `autoplay_s1b.gd`, `autoplay_s3.gd`)
key on the `type` string: kept working through the derived `type`.

---

## Data model

### 1. `extends` (every def kind)

```toml
# defs/units/usa_rifle_squad_9.toml
id      = "ww2:usa_rifle_squad_9"
extends = "ww2:usa_rifle_squad"
name    = "Rifle Squad (understrength)"
cost    = 260
tags    = ["line", "understrength"]
[[members]]           # replaces the parent's whole list (arrays replace)
slot = "leader"
...
```

Rules (content crate only, `sdk/crates/content/src/resolve.rs`, new):

1. Each file is parsed to a raw `toml::Table` first, per kind
   (`units`, `weapons`, `items`, `ammo`, `vehicles`, `factions`, `maps`,
   `rules`). `extends` names a def **of the same kind**, in this mod (a
   later mod may extend an earlier one's when mod stacking lands).
2. Chain: child → parent → … at most **4 deep** (`MAX_EXTENDS_DEPTH`,
   the def itself not counted). A longer chain, an unknown parent (with
   did-you-mean) or a cycle (`a → b → a`, listing the cycle) is an error
   on the child's file.
3. Merge from the root down: **tables merge key by key, arrays replace,
   scalars replace** (the same `merge_tables` as rules overrides,
   `modset.rs:777`). `id` is never inherited; `extends` itself is
   dropped from the resolved def.
4. Group rule: a level that sets `[[members]]` drops the inherited legacy
   soldier keys (`men`, `weapon`, `weapon_men`, `sidearm`, `loadout`); a
   level that sets any legacy soldier key over a parent that resolved to
   `[[members]]` is an error ("parent uses [[members]]; set members
   here"). `role` is legacy sugar that only fills `tags` and
   `capabilities` left empty after the merge.
5. The merged table is deserialised into the def struct (unknown keys
   stay errors) and **validated as a whole**: per-file `validate` and the
   cross-checks run on resolved defs only. Issues name the child's file
   and, when a value came from a parent, "(from `ww2:parent`)".
6. Rules overrides (`--rules-override`, `[defs.<kind>."<id>"]`) patch the
   **raw** table of that def, then every def re-resolves: a patch on a
   parent reaches its variants. `base = ` in a rules override stays (an
   override is not a def file); a rules def file may use `extends`.
7. Parents may be abstract: `abstract = true` (units only) resolves and
   validates but cannot appear in a roster or start squads. Used for
   shared crew templates later (gun crews, S9); none in S4 content.
8. `ModDefs` keeps the resolved defs (what the sim and the hash see) and,
   for the SDK, `ModDefs::raw` (file tables and their parent) so the
   schema export and the Godot data reload work the same way.

### 2. The card block (unit defs, top-level keys)

| Key | Meaning | Default | Today's key (accepted as alias) |
|---|---|---|---|
| `cost` | manpower per call-in | required (soldier cards) | `cost` |
| `pop` | population | soldier cards: men (no override in S4, S4-9); vehicle cards: the vehicle's `pop` | vehicle def `pop` |
| `cooldown_s` | seconds before this card can be called again; the unit spawns at base at once | 0 | `recall_cooldown_s` |
| `unlock_s` | match seconds before the first call-in | 0 | `initial_cooldown_s` |
| `cooldown_group` | cards naming the same group share a timer | **unset** (owner Q21 (B)) | `cooldown_group` |
| `max_alive` | most squads of this card alive at once per player | 0 = no cap | — |
| `limit_group` | a group from the rules' `[limit_groups]`; alive units of all its cards count | none; vehicle cards are always in the rules' vehicle limit as well | `[vehicles] limit` |
| `available_from`, `available_to` | `"YYYY-MM"`; a card outside the match's `[call_in] date` is dropped from the roster at resolve | always | — |
| `tags` | AI mix and HUD filters | from `role` | `role` |
| `capabilities` | what the unit can do; orders follow | from `role` | `role` |
| `training` | training profile id | `ww2:training_regular` (rules `[training] default`) | `steadiness_pct` stays and multiplies |
| `ai_weight` | the AI's weight among ready cards of the same mix tag | 100 | — |
| `short_name` | HUD tile name (≤ 14 characters) | `name` | — |
| `sources` | citations | `[]` | — |

Alias rule: an alias and its new key in one resolved def is an error;
the alias is kept for third-party mods; the shipped `ww2` files move to
the new names at the content switch (Part B).

Vehicle cards (`vehicle = "…"`): `cost`, `cooldown_s`, `unlock_s` may be
set on the card; unset, they come from the vehicle def as today (S4-15).
The rules `[vehicles] unlock_s` stays a floor for every vehicle card and
`[vehicles] limit` its implicit limit (`tests/rules_test_unlock.toml`
keeps working).

Rules additions: `pop_cap = 150` (Q6 (A)); `[limit_groups]` (empty in
`ww2`); `[call_in] date = ""` (empty: no date filter; skirmish has none);
`[training] default`; `[ai.mix]` (below).

### 3. `[[members]]`

```toml
[[members]]
slot   = "mg_gunner"            # free name; shown on the squad card
count  = 1                      # 1..=16; sum over slots = men (1..=16)
weapon = "ww2:mg42_bipod"
sidearm = "ww2:p38"             # optional: what he fights with if he
                                # hands the crew weapon on
items  = { "ww2:stielhandgranate" = 1 }
traits = ["crew_gunner"]
```

- **Order** is spawn and formation order; the first man of the first
  slot is the squad leader (the leader items rule and `lead()` stay).
- **Per-man weapon** is fixed at spawn (new soldier state
  `Kit::weapon`, saved, hashed, validated) instead of today's position
  rule (`weapon_of`). `small_arms_of` (focus range) becomes the range of
  the squad's most numerous non-crew weapon.
- **Crew weapons** (weapon def `crew_weapon = true`: MGs, launchers,
  mortars): when the carrier falls, after the weapon's `handover_ms` the
  nearest living man of the squad picks it up, `ammo_bearer` /
  `crew_assistant` traits first, ties by ascending id. He drops his own
  weapon (no two-weapon inventory, S4-16). Today's AT-team `Launcher`
  hand-over becomes this rule (the launcher's rounds stay squad state).
  Non-crew weapons stay with the man who carried them: today's silent
  pass of a tank crew's MP 40 / M3 to the next man ends (S4-2).
- **Items** per man by id (counts ≤ `MAX_ITEM_COUNT`); at most one item
  of each kind per man. `ammo` per member is parsed and stored but
  ignored until S5 (rounds stay unlimited, documented on the key).
- **Traits** (S4 set, validated): `leader`, `crew_gunner`,
  `crew_assistant`, `ammo_bearer`. `officer`, `radio`, `medic`, `sniper`
  are reserved and refused until their slice (an unknown trait is an
  error, not a silent no-op). No officer rally (Q8 (A), S4-12).

**Legacy expansion** (content crate, exact): `men = N`, `weapon = W`,
`weapon_men = K` (default N), `sidearm = S`, `loadout` →

| Slot | count | weapon | items |
|---|---|---|---|
| `leader` | 1 | W if K ≥ 1 else S | each loadout item's `leader_count` |
| `weapon` | K − 1 (omitted if 0) | W | each item's `count` |
| `sidearm` | N − K (omitted if 0) | S | each item's `count` |

With no `sidearm` every man carries W (as today). Lowering of a legacy
squad into `SquadTypeSetup` must give **byte-identical** setup in Part A
(T7). AT teams keep `weapon_men = 1` → leader carries the launcher.

### 4. Capabilities and tags

Capabilities are engine meanings (each gates an order or a sim rule);
tags are free labels for the AI mix and HUD filters, with four
engine-reserved names.

| Capability (S4) | Gates | Default from legacy `role` |
|---|---|---|
| `dig_basic` | Dig (trench profile) | any role with `dig_rate_pct > 0` |
| `dig_full` | profiles that need it (`at_ditch`: `needs = "dig_full"` replaces `engineers_only`) | engineer |
| `barrage` | Barrage | **derived**: a member with a `mortar` weapon (set or not) |
| `recrew` | board an own abandoned vehicle as untrained crew (`vehicles.recrew.any_infantry`) | rifle, mg, engineer, mortar |
| `vehicle_crew` | trained crew; may capture when `capture_needs_crew`; dismissed after `crew_dismiss_s` in base | crew |
| `at_hunt` | **derived**: a launcher member or a single-shot AT item in the kit | at |

Reserved for later slices and refused in S4 with a hint: `build_structure`,
`build_heavy`, `place_wire`, `clear_wire`, `place_mines`, `clear_mines`,
`breach`, `demolish`, `observe`, `radio`, `medic`, `gun_crew`,
`issue_crates`, `flame`.

Tags: free strings `[a-z_]{1,24}`, at most 8 per card. Reserved:
`vehicle` (must equal "has a vehicle"), `crew` (never in a roster or
start squads, replaces `SquadRole::Crew` checks), `at` (AI gate: after the
vehicle unlock, once an enemy vehicle was seen), `understrength` (HUD
marker). Legacy `role` → tags: rifle `line`; mg `mg`; engineer
`engineer`; mortar `mortar`; anti_tank `at`; vehicle `vehicle`; crew
`crew`.

The presenter's `type` string stays (derived: first tag mapped back to
`rifle`/`mg`/`engineer`/`mortar`/`at`/`vehicle`/`crew`) so the mock,
autoplay and HUD keep working; new code reads `tags` and `capabilities`.

### 5. Training profiles (`defs/training/*.toml`, new def kind)

```toml
# defs/training/regular.toml
id = "ww2:training_regular"
name = "Regular"
spread_pct = 100            # aim error multiplier on the weapon's spread (%)
aim_time_pct = 100          # time to open fire on a new target (%; today hz/2)
reload_pct = 100            # reload time (%)
throw_scatter_pct = 100     # thrown items' scatter (%)
steadiness_pct = 100        # multiplies the card's steadiness_pct (duck/pin thresholds)
suppression_decay_pct = 100 # suppression decay per second (%)
spot_pct = 100              # sight range of the men (%)
build_rate_pct = 100        # dig_rate_pct multiplier (%)
```

Starting values (reasoning, no measured source; the profile changes the
man, never the weapon):

| Profile | spread | aim time | reload | scatter | steadiness | decay | spot | build |
|---|---|---|---|---|---|---|---|---|
| conscript | 125 | 125 | 115 | 125 | 80 | 85 | 95 | 90 |
| regular | 100 | 100 | 100 | 100 | 100 | 100 | 100 | 100 |
| veteran | 85 | 80 | 90 | 85 | 125 | 120 | 110 | 100 |
| elite | 75 | 70 | 85 | 75 | 150 | 140 | 115 | 100 |

Every S4 card is `regular` (S4-10): profiles are built and tested
(effects measured in a staged case), but the balance baseline is not
confounded. The VG-Sturmgruppe (S5) is the first conscript card.
No in-match veterancy (roadmap §2.5).

### 6. `SquadTypeSetup` additions (Part B, step B1)

`members: Vec<MemberSetup>` (weapon, sidearm, items, traits), `tags:
Vec<u16>` (dense ids over the mod's sorted tag set), `caps: u32` (bit
set), `training: u16`, `max_alive: u16`, `limit_group: Option<u16>`,
`ai_weight: u16`. `role`, `men`, `weapon`, `weapon_men`, `sidearm`,
`loadout` are removed from the setup (the content crate derives
`men = Σ count`). `AiSetup::mix_pct: [u32; 4]` + `mix_vehicle_pct` +
`mix_at_pct` → `mix: Vec<(tag u16, pct u32)>`. All validated on load.

---

## Migration plan and the resolved-def snapshot test

**Part A changes no sim code and no `MatchSetup` layout.** The setup's
postcard bytes are part of the state hash (`hash.rs:490-548`), so Part A
lowers resolved defs into today's `SquadTypeSetup` fields exactly.

1. **Before any code:** a test `setup_bytes_baseline` builds the
   standard skirmish `MatchSetup` (both factions, map `kessel_ridge`,
   rules `ww2:skirmish`, plus `rules_test_unlock`) and asserts its xxh3
   equals constants recorded at `63c3e36`. It must pass unchanged through
   Part A.
2. **Resolved-def snapshot:** `sdk/crates/content/tests/resolved_defs.rs`
   writes every resolved def of `mods/ww2` (all kinds) as sorted pretty
   JSON and compares with the committed
   `sdk/crates/content/tests/snapshots/ww2_resolved.json`; `BLESS=1`
   rewrites it. Generated **first** from the old loader (a `SquadDef`
   serialised as today), then the new loader's legacy view of the same
   files must produce the same JSON for every legacy key, plus a second
   snapshot of the canonical view (members, tags, capabilities, card
   keys) that later content changes update on purpose.
3. **Hash equality:** after Part A, `skirmish --ai-vs-ai --seed 1
   --seeds 20 --both-bases --summary` prints the **same 40 hashes** as the
   baseline list; `cargo test` golden and determinism tests unchanged;
   `content_hash` (the relay's lobby guard) is allowed to change (defs
   gain fields) and that is noted in the protocol doc.
4. **Part B step B1** (state and setup layout) changes hashes on purpose:
   new setup fields, per-man weapon state, training, card counters,
   `SAVE_VERSION` 12. The gate from B1 until the content switch (B6):
   on the old content **every per-match summary line except the hash is
   identical** to the baseline (winner, length, losses, rockets,
   satchels, HE share), with one listed exception: tank-crew squads no
   longer pass their SMG by position (S4-2); if that moves a line, the
   diff is shown match by match. New baselines are recorded at B1 and
   at the content switch.
5. Content switch (B6): the `ww2` files move to `[[members]]` and the new
   cards; hashes and results change on purpose; new baselines recorded.

---

## Call-in fold-in

Already true (S3b, kept, tested): one command buys and spawns at once at
base; per-card cooldown; no queue; refusals carry the reason.

New checks in `call_in_check` (order): roster (date-filtered at resolve)
→ `unlock_s` (card) and the vehicle unlock floor → `max_alive` →
`limit_group` → vehicle limit → cooldown → manpower → pop cap. New
`CallInRefusal` variants `MaxAlive`, `GroupLimit { group }` (bridge
words "N alive (max)", "<group> limit"). The AI and the command use the
same function. Tests: an immediate spawn at tick t; a second call of the
same card refused for exactly `cooldown_s`; a sibling card (no group)
callable at once (Q21 (B)); with `cooldown_group` set both refused;
`max_alive` and a limit group refuse and free up when a squad dies; a
card outside `[call_in] date` is not in the roster; nothing is ever
queued (pending list empty after every tick).

---

## Content (Part B, step B6)

Costs on today's scale (income 240/min + 60 per point, start 600,
Panzer IV 620, Sherman 590), **pop cap 150**. Starting values; the Tune
stage moved them (shipped prices: "Tune results"). "Real" columns: sources
below.

### Germany

| Card (file) | Men | Members | Cost | Cooldown | Notes |
|---|---|---|---|---|---|
| Grenadiergruppe (`ger_grenadiergruppe`, replaces `ger_rifle_squad`) | 10 | Gruppenführer MP 40 (2 Stielhandgranaten, Nebelhandgranate); MG-Schütze 1 MG 42 (bipod) + P 38; MG-Schütze 2 P 38 (`crew_assistant`); MG-Schütze 3 Kar98k (`ammo_bearer`); 6 Schützen Kar98k (1 Stielhandgranate each; two carry a Panzerfaust 60) | 320 | 20 s | tags `line`; 300 in the roster + 2 × 10 for the Panzerfausts |
| Grenadiergruppe (Feldstärke) (`ger_grenadiergruppe_7`) | 7 | leader, the MG team of 3, 3 Schützen (one Panzerfaust) | 230 | 20 s | `extends` + members; tags `line`, `understrength`; `ai_weight` 50 |
| sMG-Gruppe (`ger_smg_gruppe`, replaces `ger_mg_team`) | 6 | Gruppenführer Kar98k; Schütze 1 MG 42 on Lafette 42 + P 38; Schütze 2 P 38 (`crew_assistant`); Schützen 3–5 Kar98k (`ammo_bearer`) | 220 | 30 s | tags `mg`; real crew 6–7 (S4-3) |
| Pioniergruppe (`ger_engineer_squad`) | 8 | leader MP 40 (satchel, smoke), 1 MP 40, 6 Kar98k | 280 | 30 s | tags `engineer`; `dig_basic`, `dig_full`; dig 200 % as today |
| Panzerschrecktrupp, Granatwerfertrupp, Panzer IV | — | unchanged (moved to `[[members]]` / card keys) | — | — | the mortar team becomes 1 mortar + 2 Kar98k (S4-2) |

### USA

| Card | Men | Members | Cost | Cooldown | Notes |
|---|---|---|---|---|---|
| Rifle Squad (`usa_rifle_squad`) | 12 | squad leader M1; asst squad leader M1; automatic rifleman BAR M1918A2; asst AR M1 (`crew_assistant`); ammo bearer M1 (`ammo_bearer`); 2 scouts M1; 5 riflemen M1 (T/O&E 7-17: 2 of them are rifle grenadiers, M7 in S11) | 340 | 20 s | tags `line` |
| Rifle Squad (understrength) (`usa_rifle_squad_9`) | 9 | leader, asst, BAR team of 3, 4 riflemen | 260 | 20 s | `extends`; `understrength`; `ai_weight` 50 |
| Rifle Squad, two BARs (`usa_rifle_squad_2bar`) | 12 | as the Rifle Squad, a second BAR + assistant in place of 2 riflemen | 360 | 30 s | `extends`; `available_from = "1944-08"` (date unverified, S4-17); `ai_weight` 50 |
| Light MG Section (`usa_lmg_squad`, replaces `usa_mg_team`) | 5 | squad leader M1; gunner M1919A4 + M1911A1; asst gunner M1911A1 (`crew_assistant`); 2 ammo bearers M1 carbine (`ammo_bearer`) | 200 | 30 s | tags `mg`; T/O&E 7-17 LMG squad |
| Engineer Squad (`usa_engineer_squad`) | 8 | leader M1 (satchel, smoke), 1 M3, 6 M1 | 280 | 30 s | tags `engineer`; 12 historical (S4-4) |
| Bazooka Team, 81mm Mortar, M4A3 | — | unchanged | — | — | mortar 1 tube + 2 M1 carbines |

Start squads: two full line squads and the engineers (S4-14): Germany
28 men, USA 32 (today 21 each).

### New and changed weapon defs

Game ranges are compressed (rifles 300 m); the tripod/bipod gap is kept
as a ratio, not the real 2–3 km. Each def gets `sources = [...]` and a
comment per value.

| Def | Values (starting) | Source / reasoning |
|---|---|---|
| `ww2:mg42_bipod` (new) | 1200 rpm, belt 50 (drum), reload 5000 ms, spread 22 mrad, range 450 m, setup 1000 ms, mv 740, damage 45, suppression 15, `crew_weapon`, `handover_ms` 3000 | 11.6 kg with bipod; ~1,200 rpm; 740 m/s; 50-round drum (Gurttrommel); bipod used in short bursts of 3–7 (Wikipedia "MG 42", citing US comparative tests); spread wider than the tripod (no traverse and elevation mechanism) |
| `ww2:mg42` → MG 42 on Lafette 42 (id kept) | 1200 rpm, magazine 150 (barrel-change interval), reload 8000 ms, spread 12 mrad, range 600 m, setup 6000 ms, rest as today | Lafette 42 20.5 kg; tripod with MGZ 34: 3,000 m direct, 3,500 m indirect; barrel changed after ~150 rounds of rapid fire, change < 10 s (Wikipedia "MG 42"); setup = mount, lay and load |
| `ww2:m1919` → M1919A4 (id kept) | 500 rpm, belt 250 abstracted as magazine 125 (burst discipline of the air-cooled barrel), reload 7000 ms, spread 12 mrad, range 600 m, setup 4000 ms, mv 853 | 14 kg gun, 400–600 rpm, 853 m/s, 250-round belt, 5-man squad (Wikipedia "M1919 Browning machine gun"; battleorder.org "U.S. Army Rifle Company 1944" after T/O&E 7-17, 26 Feb 1944) |
| `ww2:bar_m1918a2` (new) | 450 rpm (A2 slow setting, fast 500–650), magazine 20, reload 4000 ms, spread 14 mrad, range 350 m, damage 65, mv 860, suppression 10, setup 0, not a crew weapon | 8.8 kg, 300–450 / 500–650 rpm, 20-round box, 860 m/s (Wikipedia "M1918 Browning Automatic Rifle") |
| `ww2:m1_carbine` (new) | 40 rpm aimed (semi-automatic), magazine 15, reload 2500 ms, spread 14 mrad, range 180 m, damage 45, mv 607, suppression 6 | 2.4 kg, .30 Carbine, 607 m/s, practical range ~180 m (200 yd), 15-round magazine (Wikipedia "M1 carbine") |

A tripod gun sets up slowly and fires tighter and further; a bipod gun
moves with the squad. The existing `mg42` and `m1919` ids keep their
files so legacy mods and the old content stay loadable until the switch.

### Sources for squad strengths (verify items)

| Item | Finding | Source | Status |
|---|---|---|---|
| Grenadiergruppe strength | 10 men: leader, 3-man MG team, riflemen (incl. deputy); some 1944 secondary sources give 9 | Wikipedia "MG 42" (squad organisation); battleorder.org "Volksgrenadiers 1944" (K.St.N. 131V, 1 Nov 1944: MG-Schütze 1 with MG 42, MG-Schütze 2 Kar98k); a wargaming article on Normandy (skirmishmagazine.com) gives a 9-man Schützengruppe (secondary, weak) | 10 kept (roster, Q6 (A)); 9 noted; K.St.N. 131 primary table not read (citation task) |
| Feldstärke 7 | field strength well below establishment by mid-1944 | roadmap §3.1; owner Q6 (A) | design value |
| sMG crew | optimum 6 for sustained fire; heavy MG squads of 7 in 1944 | Wikipedia "MG 42" | 6 (S4-3) |
| US Rifle Squad 12 | SL, ASL (M7), BAR, asst BAR, ammo bearer, 2 grenadiers (M7), 2 scouts, 3 riflemen; 11 M1 + 1 BAR | battleorder.org "U.S. Army Rifle Company 1944" (T/O&E 7-17, 26 Feb 1944, changes 30 Jun 1944; FM 7-10, 18 Mar 1944) | verified (secondary, cites the T/O&E) |
| Understrength 9 | typical autumn 1944 after replacements | roadmap; no table | design value |
| Two BARs | US divisions "increasingly" specified two BAR teams per squad in 1944–45, following Marine practice; no date | Wikipedia "M1918 Browning Automatic Rifle" | practice confirmed, month unverified (S4-17) |
| LMG squad 5 | SL (M1 + M7), gunner (M1919A4 + pistol), asst gunner (pistol), 2 ammo bearers (M1 carbine); section = HQ 2 + 2 squads | battleorder.org (T/O&E 7-17) | verified (secondary) |
| Engineer squads | US combat engineer squad 12 (T/O&E 5-15, 13 Mar 1944, not read); game 8 by the roadmap | patcosta.com scan of T/O&E 5-15 (unreadable text) | 8 kept (owner roadmap); weapons from memory (S4-4) |
| Panzerfaust 60 | 60 m, 200 mm, 45 m/s, 6.1–6.8 kg; full production from Sep 1944 (Pzf 30: 30 m, from Aug 1943) | Wikipedia "Panzerfaust" (citing Rottman, *Panzerfaust and Panzerschreck*, 2014) | verified (secondary); weight sources differ |
| Panzerfaust issue | 36 per rifle company in late-1944 tables of issue; 18 per Panzergrenadier company | *Tactical and Technical Trends* (Oct 1944) as summarised by lonesentry.com / battleorder.org | secondary; primary not read |

---

## Panzerfaust 60 (S3c-29 (A))

Physical, like the Panzerschreck: a HEAT round with a jet and spall,
fired by a man, flying a real arc; no hit chance, no damage table.

- **Content:** `defs/ammo/pzf60_heat.toml` (HEAT, calibre 149 mm,
  `muzzle_velocity_m_s` 45, `penetration = [[0, 200]]`, jet and spall as
  the RPzB round (jet 100 cm, 7 spall rays), `[he]` small, sources);
  `defs/items/panzerfaust60.toml` with the new item kind
  `single_shot_at` (`ammo`, `range_m` 60, `engage_still_m` 60,
  `engage_moving_m` 40, `spread_mrad` 15, `aim_ms` 1500,
  `fire_reveal_pct` 300, `fire_reveal_ms` 3000).
- **Counts and cost (S4-5 (A)):** 2 per Grenadiergruppe (two Schützen),
  1 per Feldstärke; card +10 MP each (Grenadiergruppe 300 → 320,
  Feldstärke 220 → 230). The late-1944 issue (36 per company, about 3–4
  per Gruppe) is the (B); Normandy supply was thinner. The roadmap's
  separate "Grenadiergruppe (Panzerfaust)" card with 3 stays in S5.
- **Sim:** a fourth `ItemKind` (the per-man kit array grows to 4: state
  layout, `SAVE_VERSION`). A man with one fires it like the launcher
  carrier (`fire_launchers` path: sees a vehicle within
  `engage_still_m` / `engage_moving_m`, LOS tries, lead estimate, aim
  time, one shell slot); his own weapon is idle meanwhile; the tube is
  spent. At most one Panzerfaust per squad per `items.single_shot_gap_s`
  (rules, 3 s) so a squad does not empty its kit at one tank. `AttackUnit`
  on a vehicle sends the nearest carrier. Base refill as any item today
  (free until S5 prices it, S4-6). Rounds at 45 m/s drop ~8.7 m over 60 m:
  the drop compensation is exact (no drag), and a test fires at 30 / 60 m.
- **AI:** automatic fire covers most use. Squads with Panzerfausts get
  `at_hunt` and join the AT hunt (`ai/assault.rs`) for a tank seen within
  `ai.assault.pzf_hunt_m` (100 m) when no enemy infantry is seen within
  `pzf_clear_m` (40 m) of the path; `urgent_at` counts a squad with a
  Panzerfaust as half an AT team.
- **Report:** Panzerfausts fired / hit / pen / knock-outs per faction;
  the tank-loss credit "infantry AT" = rockets + satchels + Panzerfausts.

## AI capture (S3c-33 (A))

AI crew squads (`vehicle_crew`) whose tank is gone may capture an enemy
abandoned tank: target the nearest one within `ai.armour.capture_m`
(150 m, 0 = off) of the crew, not burning, not useless, with no seen
enemy within `recrew_clear_m`, and only while the rules allow it
(`vehicles.recrew.capture = 1` and `capture_needs_crew` as today).
Else the crew retreats as in S3c. The captured tank joins the AI's tank
logic. Counters: capture orders, captures, captured tanks lost. A/B:
`capture_m` 0 vs 150 on the 160 set; default stays on unless T1 moves
by more than 5 points (S4-7).

## AI tag mix

```toml
[ai.mix]          # percent of bought squads per tag (replaces mix_*_pct)
line = 50
mg = 20
engineer = 15
mortar = 15
at = 20           # active after the vehicle unlock once an enemy vehicle was seen
vehicle = 20      # active after the vehicle unlock, under the vehicle limit
```

A card counts toward the first of its tags that `[ai.mix]` lists; a card
with none is never bought by the AI. The deficit ranking is today's
(`ai.rs:879-887`). Within the chosen tag the AI picks among cards that
are ready, affordable and under their caps by `ai_weight` (one AI RNG
draw); it saves up only for the tag's highest-weight card. Same numbers
as today for the old content (the legacy keys map 1:1; a test checks
that the old `mix_*` and the new table buy the same squads on seed 1).
Expected starting shares: line cards 2 : 1 full to understrength.

## HUD (HUD v2 layout, military UI direction)

- **Call-in tiles:** one per card, keyed by card id (bridge catalog
  gains `id`, `short_name`, `men`, `tags`, `understrength`; the intent
  sends the id). Tile: short name, men count ("10"), cost, the existing
  cooldown fill as a ring around the count, an understrength marker (a
  half-filled chevron) and the date/lock state. Tooltip: period name
  with an English gloss (Q7 (A)): "Grenadiergruppe — rifle squad, 10
  men: MG 42 team, 6 riflemen, 2 Panzerfaust 60".
- **Squad card:** member rows by slot: role word, weapon short name,
  items (Panzerfaust count), the crew weapon's current carrier marked;
  "MG picked up by Schütze 3" in the feed.
- **Order buttons by capabilities:** Dig (`dig_basic`), AT ditch
  (`dig_full`), Barrage (`barrage`), Crew K (`recrew` / `vehicle_crew`),
  Panzerfaust as an ability button (fires at the hovered vehicle) when a
  selected squad carries one. The bridge sends per squad `tags`,
  `capabilities`, `members[]`; the old `type` stays.
- **Godot checks:** `test_match_view` rows for the tile fields, ring,
  marker, member rows and capability-gated buttons; `test_data_reload`
  with an `extends` variant edited and reloaded (F5); the mock catalog
  gains two cards per role. Settings tests detach the Settings node;
  `settings.cfg` untouched.

---

## Targets

Standard set: skirmish seeds 1–20 × both bases (40, `--jobs 3 --minutes
45`); faction decisions on seeds 1–80 × both bases (160); armour-test
`--stress --seeds 10`; fortify-test `--seeds 10`.

| # | Target | Baseline | How |
|---|---|---|---|
| T1 | each faction wins 35–65 % | 40: G 27.5 % (miss); 160 (S3c final): G 45 % | summary; judged on 160 (S4-11) |
| T2 | no base side wins > 62 % | 40: 52.5 % | summary |
| T3 | no match < 10 or > 40 min; median 15–30 | 12.3–31.7, median 16.7 | summary |
| T4 | HE share of kills: mean < 50 % (S3c-31 (A)); report max | 43.0 %, max 56.2 % | summary |
| T5 | fortify-test PASS; armour-test all hard checks pass; golden ballistics 46/46; `validate mods/ww2` ok | pass | as listed |
| T6 | tick p99 at peak < 1.5 ms (`--jobs 3`, quiet machine); stress p99 < 10 ms; Battalion-tier: armour-test stress case and a 700-soldier `simulate` with 10–12-man squads inside budget | 0.733 ms; stress below | summary; armour-test case 14 |
| T7 | Part A: `MatchSetup` bytes and all 40 match hashes equal to the baseline; every test unchanged | list above | test + summary diff |
| T8 | every existing unit file loads unchanged and resolves to the same def (resolved-def snapshot) | — | `resolved_defs.rs` |
| T9 | a variant made only of `extends` + a member list loads and plays: `mods/ww2/tests/variants/` example file, a test that calls it in, it fights and its MG is picked up | — | test |
| T10 | call-in: spawn at once; per-card cooldown; sibling cards independent (Q21 (B)); `cooldown_group`, `max_alive`, limit groups, `unlock_s`, dates refuse with reasons; nothing queued | partly (S3b) | tests |
| T11 | the AI buys every new S4 card: each line card in ≥ 50 % of its faction's matches, understrength cards in ≥ 25 %; MG cards in ≥ 75 % | new | summary (cards bought per match) |
| T12 | Panzerfausts fired in ≥ 10 of 20 normal-base matches; infantry AT (rockets + satchels + Panzerfausts) ≥ 10 % of tank losses (S3c-28 (A)) | rockets in 14 of 20; 9.7 % (160) | summary |
| T13 | AI capture: ≥ 1 capture per 10 matches with `capture_m` 150; T1 within ±5 points of `capture_m` 0 | 0 | A/B on 160 |
| T14 | crew-weapon pickup works: a staged case and skirmish counter (pickups per match) | new | test + report |
| T15 | determinism: 1/2/3/8 threads, save → load → continue, net loopback, soaks; every new field saved, hashed (test per field), validated on load | pass | tests |
| T16 | pop: report mean own pop at 5/10/20 min and pop-cap refusals per match (cap 150) | new | report |

---

## Implementation order

### PART A: content crate data model, loader, migration (no behaviour change; hashes equal on the old content)

1. Baseline tests first: `setup_bytes_baseline` (setup xxh3 constants)
   and the resolved-def snapshot from the old loader; commit.
2. `resolve.rs`: raw tables per kind, `extends` (depth 4, cycles,
   unknown parent with did-you-mean), merge rule, group rule, whole-def
   validation; overrides patch raw tables and re-resolve; `abstract`.
   Unit tests: chain of 4 ok / 5 refused; cycle of 2 and 3; table merge;
   array replace; legacy-over-members error; override on a parent
   reaching a child.
3. Card block keys and aliases on `SquadDef` (and vehicle-card fields);
   `[[members]]` (`MemberDef`), traits, `tags`, `capabilities`,
   `training`, `ai_weight`, `short_name`, dates, `max_alive`,
   `limit_group`; legacy expansion; validation (counts, Σ = 1..=16,
   one item per kind per man, reserved tags and capabilities, aliases
   not doubled, dates `YYYY-MM`, `available_from ≤ available_to`).
4. `defs/training` kind (four profiles in `mods/ww2`, unused by the sim
   yet); rules `[training] default`, `[limit_groups]`, `[call_in] date`,
   `[ai.mix]` parsed beside the old `mix_*` keys (exactly one form; the
   old form maps 1:1), `pop_cap` unchanged (120) in Part A.
5. Lowering: resolved defs → today's `SquadTypeSetup` fields exactly
   (`setup.rs:868-916`); a def that cannot be expressed in today's
   setup (two weapons beyond weapon/sidearm, per-man items differing from
   the leader/rest rule, non-regular training, `max_alive`) is accepted
   by `validate` but refused by `resolve` in Part A with "needs S4 Part
   B" (removed in B1).
6. JSON Schemas (`frontline-cli schema`) for the new keys and kinds;
   `docs/modding/` page "Variants" (extends, members, card block,
   capabilities, tags, training, examples, errors); `tuning.md` rows.
7. Measure: setup bytes equal; resolved snapshot equal; 40 hashes equal;
   all tests, clippy, fmt, golden, validate, fortify, armour; commit.

### PART B: sim / AI / HUD use of members, capabilities, training; new content; Panzerfaust; AI capture

1. **B1 State and setup layout** (hashes change on purpose): new
   `SquadTypeSetup` fields (§6), per-man `Kit::weapon`, the fourth
   item kind, card counters (alive per card, per limit group, captures,
   pickups, Panzerfausts); save/load/hash with a test per field and load
   validation; `SAVE_VERSION` 12 (once); `PROTOCOL_VERSION` 6 only if a
   command changes (planned: none; `CallIn` by index stays; relay limits
   unchanged; `sdk/docs/net/relay-protocol.md` notes the content-hash
   change). Remove Part A's "needs Part B" refusal. Record the B1
   baseline (results equal to the old except hash; S4-2 exception
   shown).
2. **B2 Capabilities and tags replace `SquadRole`** in every row of the
   map above (sim, AI, bridge, CLI, harness, scenarios); `SquadRole`
   kept only as the legacy TOML `role` value. Results equal to B1.
3. **B3 Members in the sim:** spawn by slots, `weapon_of` → `Kit::weapon`,
   crew-weapon pickup (AT launcher hand-over folded in), leader items by
   slot, `small_arms_of` rule; bail-out crews spawn by their members.
   Tests: MG gunner killed → the `crew_assistant` has the MG after
   `handover_ms` and fires; no pickup by a man of another squad; a tank
   crew's SMG stays with its man; determinism at 1/2/3/8 threads and
   save/load mid-hand-over.
4. **B4 Training** in `combat.rs` (spread, aim time, reload),
   `items.rs` (scatter), suppression (steadiness, decay), `spotting.rs`
   (sight), `dig.rs` (rate). A staged case: conscript vs elite rifle
   squads at 200 m, hits per minute and time to pin reported; regular
   gives B3's results exactly.
5. **B5 Call-in:** `max_alive`, limit groups, per-card unlock, dates,
   refusals, AI readiness; tests (T10).
6. **B6 Content switch:** every `ww2` unit file to `[[members]]` and card
   keys; the new cards and weapon defs with sources; pop cap 150; start
   squads; `[ai.mix]`; the example variant under
   `mods/ww2/tests/variants/` (T9). New baselines (hashes change on
   purpose). Measure the standard set.
7. **B7 Panzerfaust:** ammo and item defs, the item kind in the sim, the
   AI hunt, the HUD button, report rows; armour-test case 16 (a
   Grenadiergruppe vs a Sherman at 40 / 60 m, still and moving:
   Panzerfausts fired, hits, penetrations, knock-outs; hard check: no
   shot beyond `engage_*`, every damage by the jet or spall).
8. **B8 AI:** tag mix with `ai_weight`, Panzerfaust hunts, capture
   (S3c-33), counters in the skirmish summary and JSON (cards bought per
   match, Panzerfausts, captures, pickups, pop at 5/10/20 min).
9. **B9 Bridge and HUD:** catalog by id, tiles (name, men, ring,
   understrength), member rows, capability-gated orders, Panzerfaust
   button, feed lines; Godot checks; F5 reload with an `extends` file.
10. **B10 Measure** every target (40 and 160 sets, armour, fortify,
    stress, tests) and write `docs/slices/S4-log.md`; the Tune stage
    follows with costs and the two MG-team crews (S4-2, S4-3).

Risks: the MG teams lose two of three guns while every line squad gains
one (S4-2); 10–12-man squads raise the per-tick soldier count (pop cap
150 vs 120: up to 25 % more men; T6); `SquadRole` replacement touches AI
code widely (B2 keeps results equal to catch it); rifles and MGs fire
without limit until S5, so the BAR and bipod MG 42 are generous until
counted ammo lands.

---

## Owner questions (work proceeds on (A); also in `docs/TODO.md`)

- **S4-1. Hash gate across Part B.** New state is hashed, so hashes must
  change at the first state-layout change. (A) Part A: setup bytes and
  all 40 hashes equal. Part B: B1 records a new baseline; until the
  content switch every summary line except the hash stays equal on the
  old content (S4-2 exception shown). (B) Keep old hashes through Part B
  by not hashing new fields at default values (more code, weaker hash).
- **S4-2. Legacy crew weapons.** Today's MG teams fire three MGs and
  every mortar man carries the tube; a tank crew's SMG passes by
  position. (A) Legacy files keep the exact old behaviour through the
  expansion (one slot of N); the `ww2` content switch gives one gun plus
  assistants and riflemen, and SMGs stay with their man. (B) Keep three
  MGs per MG card (unhistorical).
- **S4-3. MG crews.** (A) sMG-Gruppe 6 (optimum sustained-fire crew),
  Light MG Section 5 (T/O&E 7-17 squad). (B) The roadmap's 4 and 4.
- **S4-4. Engineer weapons at 8 men.** (A) Germany leader + 1 MP 40, 6
  Kar98k; USA leader M1, 1 M3, 6 M1 (today both are all-SMG, 5 men).
  (B) Keep all-SMG engineers.
- **S4-5. Panzerfaust counts and cost.** (A) Pzf 60: 2 per
  Grenadiergruppe, 1 per Feldstärke, +10 MP each; in every lobby (no
  dated lobbies yet; a Pzf 30 def comes with them). (B) 3 per squad (the
  late-1944 table of issue).
- **S4-6. Panzerfaust refill.** (A) As items today: one back per
  `items.resupply_s` in base, free until S5 prices it (D1). (B) No
  refill until S5.
- **S4-7. AI capture default.** (A) On (`ai.armour.capture_m` 150)
  whenever the rules allow capture; off if T1 moves > 5 points. (B) Off
  by default.
- **S4-8. AI choice inside a tag.** (A) `ai_weight` per card (full 100,
  understrength and two-BAR 50). (B) Cheapest ready card.
- **S4-9. Pop of soldier cards.** (A) Always the men count (no `pop`
  override on soldier cards). (B) Allow a `pop` override.
- **S4-10. Training in S4 content.** (A) Every S4 card `regular`; the
  first conscript card is the VG-Sturmgruppe (S5). (B) Feldstärke as
  conscript.
- **S4-11. Faction-win gate.** (A) Judge T1 on 160 matches (seeds 1–80),
  report the 40 set (the 40 set already misses at baseline). (B) Judge
  on 40.
- **S4-12. Officer rally** (roadmap Q8). (A) Cut: no rally trait in S4.
  (B) Keep it, small and visible.
- **S4-13. Names** (roadmap Q7). (A) Period names with an English gloss
  in the tooltip. (B) English only.
- **S4-14. Start squads.** (A) Two full line squads and the engineer
  squad. (B) Two understrength line squads (closer to today's 21 men).
- **S4-15. Vehicle card fields.** (A) Stay in the vehicle def as today;
  the unit card may override. (B) Move them to the unit card now.
- **S4-16. Pickup.** (A) The picker drops his own weapon; assistants
  and ammo bearers first, then the nearest man. (B) Any nearest man.
- **S4-17. Two-BAR card date.** (A) `available_from = "1944-08"` kept as
  a field-practice date (no T/O&E; skirmish has no date filter). (B)
  From 1944-06.
- **S4-18. Tripod and bipod MG values.** (A) Tripod range 600 m, spread
  12 mrad, setup 6 s; bipod 450 m, 22 mrad, 1 s (compressed scale,
  reasoning above). (B) One MG 42 def for both.

## Appendix: baseline hashes (40 set, `63c3e36`)

| Seed | Bases | Winner | Length | Final hash |
|---|---|---|---|---|
| 1 | normal | ww2:germany | 17:55 | 0xa2e56f33102173ac |
| 1 | swapped | ww2:germany | 16:18 | 0x53362ca936a8b854 |
| 2 | normal | ww2:usa | 14:17 | 0x437d3d4a3b41933f |
| 2 | swapped | ww2:usa | 15:19 | 0x5743de8136ae2aa8 |
| 3 | normal | ww2:germany | 21:29 | 0x64290ea392cf0994 |
| 3 | swapped | ww2:usa | 16:39 | 0x435ec95392bbdddc |
| 4 | normal | ww2:germany | 17:23 | 0xcefd21dcf359e0d1 |
| 4 | swapped | ww2:usa | 15:47 | 0x1a2395a13760b20a |
| 5 | normal | ww2:usa | 17:44 | 0x41bfa760915acfe8 |
| 5 | swapped | ww2:usa | 15:18 | 0x64ae1585fdea0396 |
| 6 | normal | ww2:usa | 31:42 | 0x4319ee31decb5ab7 |
| 6 | swapped | ww2:usa | 25:50 | 0x81e70bbb4a135168 |
| 7 | normal | ww2:usa | 14:58 | 0x8262d3841d6fe711 |
| 7 | swapped | ww2:usa | 24:52 | 0x60945898b7e76411 |
| 8 | normal | ww2:usa | 23:14 | 0xbcbd79a7f84194d2 |
| 8 | swapped | ww2:usa | 15:01 | 0x0351307e310beba8 |
| 9 | normal | ww2:usa | 23:51 | 0x0f6a0a8d0293d991 |
| 9 | swapped | ww2:usa | 13:23 | 0x6e741a77a97d6954 |
| 10 | normal | ww2:germany | 13:46 | 0x9a2ebaf047b1bb77 |
| 10 | swapped | ww2:germany | 12:41 | 0xfd5db6515a9ad531 |
| 11 | normal | ww2:usa | 18:16 | 0xf5774c7228a445f4 |
| 11 | swapped | ww2:usa | 20:53 | 0x8000abb7266c48c4 |
| 12 | normal | ww2:usa | 12:17 | 0xd4518f65a1972768 |
| 12 | swapped | ww2:germany | 13:38 | 0xe49973652a7d5d36 |
| 13 | normal | ww2:usa | 22:04 | 0x29eb3e92d53e1e7c |
| 13 | swapped | ww2:usa | 20:36 | 0x3689639dc21ce668 |
| 14 | normal | ww2:usa | 13:15 | 0x448313f863b71d67 |
| 14 | swapped | ww2:germany | 23:34 | 0xce00130890fa541c |
| 15 | normal | ww2:germany | 15:05 | 0xfb379aa28bb3a754 |
| 15 | swapped | ww2:usa | 27:24 | 0x45a65d0b4c7b3549 |
| 16 | normal | ww2:germany | 13:40 | 0xf3d19920f9f18131 |
| 16 | swapped | ww2:usa | 18:13 | 0xcfd1d88fb3473489 |
| 17 | normal | ww2:usa | 13:02 | 0xb59885afeb68ac12 |
| 17 | swapped | ww2:usa | 18:53 | 0x4f344f21894cccb3 |
| 18 | normal | ww2:usa | 14:50 | 0x9fd016b5d0f40a16 |
| 18 | swapped | ww2:usa | 17:42 | 0x08470e8696e0d9b3 |
| 19 | normal | ww2:usa | 22:24 | 0x66a625cee6cae39e |
| 19 | swapped | ww2:germany | 16:43 | 0x83ee07aea3972551 |
| 20 | normal | ww2:usa | 14:27 | 0xcc659c9d5da45ec0 |
| 20 | swapped | ww2:usa | 15:09 | 0x82ef55069baa36b7 |

## Tune results

Stage TUNE (Part B left T1, T2, the fortify barrage row, T12 and T13 open).
All numbers: release build, `--jobs 3 --minutes 45`. "40 set" = seeds 1-20 x
both bases; "160 set" = seeds 1-80 x both bases. A/B runs used
`--rules-override` files (scratch), then the winning values went into the
TOML with their reasons in comments. Runs are deterministic: the 160-set
run of the shipped mod reproduced the A/B run of the same overrides line for
line.

### What changed (data only, one test-helper fix)

| Where | Was | Now | Why (measured) |
|---|---|---|---|
| `ger_grenadiergruppe` cost | 320 | 280 | per-man price parity with the US squad (28 / 30 MP) |
| `ger_grenadiergruppe_7` | 230 | 200 | same price per man |
| `ger_smg_gruppe` | 220 | 195 | |
| `ger_engineer_squad` | 280 | 250 | |
| `ger_panzerschreck_team` | 200 | 180 | equals the Bazooka team |
| `usa_rifle_squad` | 340 | 360 | 11 Garands + BAR out-shoot 10 Germans |
| `usa_rifle_squad_9` / `_2bar` | 260 / 360 | 275 / 380 | same price per man; +20 for the 2nd BAR |
| `usa_lmg_squad` | 200 | 280 | the best-value card of the game (5 buys a match at 200) |
| `[ai.armour] capture_m` | 150 | 300 | 133 capture orders and 9 captures in 160 matches, was 9-23 and 1-3 |
| `[dig.profiles.trench] depth_cm` | 160 | 180 | fortify barrage row (below) |
| `kessel_ridge` ridge | (160,870)-(870,160) | (157,861)-(861,157) | base lean (below) |

Unchanged on purpose: tag mix, training (all regular), pop cap 150, start
squads, the mortar cards (280 both), tank costs, Panzerfaust and weapon
stats, every sim line.

Test-only edits: the content tests' cost expectations and the example
override (`mods/ww2/tests/overrides/example.toml`, restates the mod), the
`trench_lip` unit test (reads the floor depth from the rules instead of
-1.6 m), and `crates/sim/tests/variants.rs` (the staged fight ends once the
MG changed hands and men fell on both sides; with the ridge moved a lone
survivor stayed out of the Americans' sight for 15 minutes). The canonical
snapshot `ww2_canonical.json` was re-blessed (15 changed lines, exactly the
table above); `ww2_resolved.json` was not touched.

### Targets

| # | Target | Part B | Tune | |
|---|---|---|---|---|
| T1 | each faction 35-65 % | 40 set Germany 8 (20 %); 160 set 31 (19.4 %) | 40 set Germany 16 (40 %), USA 24; **160 set Germany 69 (43.1 %)**, USA 91 | pass |
| T2 | no base side > 62 % | 40 set base 0 26 (65 %); 160 set 95 (59.4 %) | 40 set base 0 18 / base 1 22; 160 set 79 / 81 (49.4 %) | pass |
| T3 | 10-40 min, median 15-30 | 11.6-30.4, median 15.7 | 40 set 12.3-27.5, median 17.3; 160 set 10.1-28.8, median 17.0, none over 40 | pass |
| T4 | HE share mean < 50 % | 45.0 %, max 64.0 % | 40 set mean 43.7 %, max 55.2 %; 160 set mean 46.0 %, max 65.5 % | pass |
| T5 | fortify, armour-test, golden, validate | fortify FAIL (barrage 44.9 %) | fortify PASS (barrage 46 vs 136 open = 33.8 %, limit 40 %; rifles 0 vs 170); armour-test all hard checks pass; golden ballistics in the test suite; `validate mods/ww2` ok; 498 workspace tests, 0 failed; clippy `-D warnings` and fmt clean | pass |
| T6 | tick p99 peak < 1.5 ms (quiet, `--jobs 3`); stress < 10 ms | 0.516 ms; 1.465 ms | skirmish 40 set peak **1.030 ms** (quiet machine); armour-test case 14 (100 vehicles, 650 soldiers) p99 1.119 ms, max 2.726 ms | pass |
| T11 | cards bought | pass | every line card in 158-160 of 160 matches, MG cards 160/160 | pass |
| T12 | Panzerfausts fired in >= 10 of 20 normal-base matches; infantry AT >= 10 % of tank losses | 6 fired in 4 of 40 matches; 7.8 % | 10 fired in 10 of 160 matches (6 of 80 normal-base); rockets knocked out 61 of 644 tank losses (9.5 %); 2 satchels thrown at vehicles | **miss** |
| T13 | >= 1 capture per 10 matches; T1 within 5 points of `capture_m` 0 | 2 in 40 | 9 captures in 160 (2 Germany, 7 USA), 103 capture orders (28 / 75); wins at 150 m vs 300 m: Germany 80 vs 82 of 160 at the same costs | **miss** (5.6 %) |
| T16 | pop | G 53/55/69, US 60/74/86 | mean pop at 5/10/20 min: Germany 60/66/82, USA 56/66/72; pop-cap holds 0.0 / match | reported |

T7-T10, T14, T15 are covered by the test suite (all pass). Godot was not
re-run: no production code changed in this stage (data and tests only).

### How the balance was found

1. **Why the US won 80 %.** Kills were even (17.3k to 18.0k over 160
   matches) but the US held more points (40 matches: 2.3k s vs 1.8k s held
   a match) and fielded 20-30 % more men at minute 10 (pop 72 vs 56). The 12-man
   Garand squad costs 28 MP a man against Germany's 32, and the AI
   buys by card count (tag mix), so a cheaper man means a bigger army. Pop at
   minute 10 tracked the win rate in every run.
2. **What moved it.** Single-change A/B runs on 160 matches (Germany
   wins, baseline 31): US line squads +50 % 45; US non-line cards +50 % 89
   (the LMG and mortar did it: LMG 300 + mortar 420 gave 84, mortar 420
   alone 63); German non-line cards -25 % 79; line-card prices only
   (German -12 %, US +24 %) 36; veteran training for German cards (a
   documented option, not used) 38 for the line squads, 50 for all five
   German cards. Start squads (a third German line squad 53-56; two
   9-man US squads 53) and a lower line share in the AI mix (54) moved it
   about as far as prices did; they were not shipped, because they change
   the opening of every match rather than a price.
3. **The pick** is the smallest price set that centred the result (A/B on
   160, Germany wins / base-0 wins): German 280 / 200 / 195 / 250 / 180, US
   360 / 275 / 380 and LMG 260 gave 74 / 92. Leaving one part out: no US
   line rise 56, no German engineer and Panzerschreck cut 60, LMG 240
   instead of 260 63. Then trench 180: 62 / 100; ridge shift: 60 / 78; LMG
   280: 69 / 79 (shipped). A cheaper German mortar (250) on top gave 73 / 85:
   inside noise, not shipped (it would price two identical weapons
   differently).
4. **Noise.** One 160-match estimate has a standard error of about 4
   points; the same cost set gave 37-50 % with other secondary changes. Every
   decision above rests on two or more neighbouring runs.

### The base-0 lean

Base 0 won 57-62 % of cross-faction matches in every Part B variant
(swapped bases too, so it is the base, not the player index). Mirror
matches (Germany vs Germany, 160) isolated it: base 0 won 108 on the map as
shipped, 94 without the noise octaves, 98 without valleys, 101 without
woods, **60 without the ridge**, 72 on a bare map. With the ridge crest at
x + y = 1036 / 1030 (shipped) / 1024 / 1012: 105 / 108 / 96 / 72 base-0
wins; at 1018, cross-faction with the Tune costs, 78 of 160 (49 %), against
100 at 1030. The crest sat 4 m on base 1's side of the centre line and the
contested centre point lies on it; a few metres decide who holds the
slope. It now runs along x + y = 1018 (12 m toward base 0). The lean was
already 97-98 of 160 before the trench change, so the two are independent.

### fortify-test barrage row

At 160 cm the 10-round 81 mm barrage killed 44.9 % as many men in the 32 m
test trench as in the open (limit 40 %). The open squad saturates (8 of
10-12 men die) while the trench losses grow with the men on the line.
Tried: spacing 2.5 m (43.5 %), parapet 45 cm (42.0 %), trench depth 180 cm
(33.3 % in A/B, 33.8 % shipped). Depth 180 cm is a 6 ft standing fighting
trench; digging work per metre is unchanged. Side effect: a dug-in squad
is harder to kill by direct fire. The T10 report rows (MG + rifles on a
dug-in squad) show the defenders losing 1.8 % (was 3.6 %) and, with two
bases of fire, 4 % (was 90 %); the row stays a "missed" report line as in
S3c (it wants 75 %), so it moves further from its threshold.

### Panzerfaust (T12) and capture (T13): why they stay missed

- 178 Panzerfaust hunts in 160 matches, 10 shots. Debug traces showed
  three causes: the carriers had already used or lost their tubes; a
  stationary tank 62 m from a far-end carrier while the leader held at
  50 m; and men pinned by the tank's MG (pinned and ducked men cannot
  fire). A separate hunt inset of 25 m (new key, built and measured:
  14 fired vs 22 in the neighbour run) made no measurable difference and
  was reverted. Panzerfaust range 70 / 80 / 100 m (40 matches, normal-base
  matches with a shot 5 / 6 / 14 of 40) and `pzf_hunt_m` 150-200 did little
  below 100 m, which is not a Panzerfaust 60. `pzf_clear_m` 0 with hunt 150
  doubled the hunts (459) and the shots (19) and changed no win rate: not
  shipped, still far from 10 of 20.
- Captures: 103 orders became 9 captures. The usual refusals are a full
  vehicle limit (the AI buys tanks up to 4), a burning hull, or an enemy
  soldier or tank near the hull. More reach (150 -> 300 m) tripled the
  orders and the captures; `capture_m` 500 was not run.
- Both need AI or sim work, not numbers: an ambush hold for carriers
  (hide and fire at 30-40 m), freeing a vehicle slot for a capture, and
  the Panzerfaust 100 (S5 list). Recorded as S4-36 and S4-37 in
  `docs/TODO.md`.


### 40-set hashes after Tune (`5cc1a80`, new baseline for later slices)

The Part B hashes above in this document's appendix are for `63c3e36`; these are the shipped S4 content. Same columns (seed, bases, winner, length, final hash).

| seed | bases | winner | length | final hash |
|---|---|---|---|---|
| 1 | normal | ww2:usa | 27:30 | 0x505ade74b85d113a |
| 1 | swapped | ww2:usa | 24:31 | 0x6f17475350e0761a |
| 2 | normal | ww2:usa | 12:37 | 0xb31a3d59c6983df9 |
| 2 | swapped | ww2:usa | 20:15 | 0x79b8987c5d8ceb61 |
| 3 | normal | ww2:usa | 19:46 | 0xf8a2283f92c6abd0 |
| 3 | swapped | ww2:usa | 14:06 | 0x69691e68f086a6a7 |
| 4 | normal | ww2:usa | 15:05 | 0xa0765b12d0923b36 |
| 4 | swapped | ww2:usa | 17:19 | 0x98ec36eeba0e066c |
| 5 | normal | ww2:germany | 19:55 | 0x088708bb10071954 |
| 5 | swapped | ww2:germany | 14:36 | 0xc6791c1f362714cf |
| 6 | normal | ww2:germany | 17:01 | 0xf5b5e878a7eb44c3 |
| 6 | swapped | ww2:germany | 17:47 | 0x0d8cea3bdf150b91 |
| 7 | normal | ww2:usa | 18:35 | 0x36cdf9acb27f7fce |
| 7 | swapped | ww2:germany | 25:01 | 0xf0ffd0d21c65ffc4 |
| 8 | normal | ww2:usa | 17:36 | 0xa2022ed063355e83 |
| 8 | swapped | ww2:usa | 15:02 | 0xa4d86285d5cab5f1 |
| 9 | normal | ww2:usa | 17:17 | 0x334dd8672d0634bb |
| 9 | swapped | ww2:germany | 13:28 | 0x48c1add2973ff5fd |
| 10 | normal | ww2:germany | 14:55 | 0xfa371b7021e6c886 |
| 10 | swapped | ww2:germany | 15:24 | 0x8d0e8fd9290f385f |
| 11 | normal | ww2:usa | 22:53 | 0xcd727f097b8f0a5a |
| 11 | swapped | ww2:germany | 13:38 | 0x753aeef7572fea36 |
| 12 | normal | ww2:usa | 17:36 | 0x0851ad8051f2db21 |
| 12 | swapped | ww2:usa | 12:46 | 0x4158c4b929377772 |
| 13 | normal | ww2:usa | 13:00 | 0x06e5d951b4063d20 |
| 13 | swapped | ww2:germany | 16:56 | 0xffc612dfddd0063f |
| 14 | normal | ww2:usa | 25:38 | 0xdffa782a6e4c1fa1 |
| 14 | swapped | ww2:germany | 14:04 | 0x1befd4fde3103899 |
| 15 | normal | ww2:germany | 19:19 | 0x14dc993fed5c9db3 |
| 15 | swapped | ww2:usa | 24:43 | 0xc3198886bc4acbee |
| 16 | normal | ww2:germany | 18:25 | 0xd303bbe8c8e376b7 |
| 16 | swapped | ww2:germany | 22:05 | 0x5bc4c6489388f906 |
| 17 | normal | ww2:germany | 15:02 | 0x4a5c613ad8a9a5ec |
| 17 | swapped | ww2:usa | 15:34 | 0x2d71dafa009ea2ac |
| 18 | normal | ww2:germany | 13:48 | 0xdb0ef258c0753039 |
| 18 | swapped | ww2:usa | 23:08 | 0x7129d214800a4527 |
| 19 | normal | ww2:usa | 12:20 | 0x7a02337456df681c |
| 19 | swapped | ww2:usa | 16:29 | 0x32de4649250096b2 |
| 20 | normal | ww2:usa | 14:35 | 0x8d711e27751a7d78 |
| 20 | swapped | ww2:usa | 18:43 | 0xbc87519041fc7b10 |

### Scratch

Overrides, outputs and digests: `.../scratchpad/s4/tune/` (`go.sh`,
`go2.sh` mirror runs, `sum.sh`, `sub.py` 40-set subset of a 160 run),
`tune1_40b.txt` (the 40-set of `5cc1a80`; `tune1_40.txt` is the same run from a build with an unused extra rules field: equal except the hashes), `tune1_fortify.txt`, `tune1_armour.txt`, `tune1_test.txt`.

## Slice log

`docs/slices/S4-log.md` (created at the start of Part A).
