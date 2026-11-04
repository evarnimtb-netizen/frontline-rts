# S6 HQ and crates log

Spec: `docs/slices/S6-hq-crates.md`. Baseline: its "Baseline" section
(`1575568`; JSON in the scratchpad `s6/baseline/`). Branch `s6-hq-crates`.

## Build stage

Commits: `c4bcb32` (content, sim, CLI, AI, bridge, HUD), `5ba1178` (T20
check, AI crate targets, HUD box positions, Godot checks, baselines),
`6ffa773` (AI sends only acceptable crate orders, clippy, tests, log).

### What was built

- **Content.** New def kind `defs/supply/` (`StockDef`: `kind` hq / dump /
  crate, `cost`, `pit`, `radius_m`, `cap_kg`, `rebuild_s`, `built_by`,
  `supply`, `kg`, `carry_speed_pct`, `cookoff`, `boxes`), lowered to
  `MatchSetup::stocks`, checked (`StockDef::validate`, `check_stocks`,
  setup validation for saves) and exported as `supply.schema.json`. Per
  faction: HQ (Gefechtsstand / Command Post, 100 MP, 25 m, 300/200/600 kg),
  dump (50 MP, 15 m, half caps), crates shells 32 kg, small arms 25 kg,
  explosives 20 kg. Cards: Zugtrupp / Platoon HQ (tag `hq`, 5 men, digs at
  100 %, `max_alive` 1), Munitionsträger / Ammunition Bearers (tag
  `bearers`, 4 men). Sources in the TOML comments; weights marked verify.
- **Pits.** `DigProfile` 2 mortar pit, 3 HQ pit, 4 dump pit:
  `[dig.profiles.*]` with `all_round` spoil and `pit_length_cm` (the dug
  line is the length minus floor and walls; work per metre set by volume).
- **Rules.** `[supply]` S6 keys (all with defaults, so S5a fixtures load):
  `crate_feed_m`, `base_crate_issue_s`, `crate_cap`, `shell_crate_cap`,
  `hq_max`, `dump_max`, `structure_bomb_s`, `crate_bomb_s`, `fwd_rearm_s`,
  `rare_rearm_fwd_s`, `serve_men`, `serve_period_ms`, `unfired_s`,
  `bullet_box_damage`, `shell_box_damage`, `cookoff_*`, `pickup_m`;
  `base_bomb_s` 0 now means the base refills no bombs (ww2: 0, owner Q4).
  `[ai.supply]` (7 keys).
- **Sim** (`crates/sim/src/structures.rs`, module docs give every rule):
  structures, crates, cook-offs and per-player `S6Account` in
  `MatchState::supply6`; commands `Build`, `IssueCrate`, `PlaceCrate`,
  `PickUpCrate` with `SupplyEvent` refusals; base issue priced kg x price
  through `pay`; carriers slow their squad and hold fire; sources (stock in,
  mortar bombs, men's small arms and items, D3 tank rearm with the rack
  credit); static boxes (`StaticBox`, derived per tick) stop bullets
  (`Blow::Static`), shells, fragments and take blast; HE that breaks a box
  with shells or explosives starts a `CookOff` (bursts of the faction's
  75 mm HE, owned by the breaker); retreat goes to the nearest own HQ.
  `SAVE_VERSION` 15, `PROTOCOL_VERSION` 9, relay doc updated. RNG purposes
  30/31 (cook-off crater, fragments).
- **AI** (`ai/supply.rs`): buys the HQ squad after `hq_after_s` and the
  bearers; sites the HQ behind the best own point on the ray-tested
  reverse slope; crate runs (shells to low mortars outside an HQ ring, to
  stranded tanks, to the HQ; small arms to the lowest Defend squad);
  budget `shell_mp_max_pct` of income; dry mortars walk to an HQ with
  shells. Tank rearm trips go to an HQ or dump with shells, else base
  (Move, so the retreat-to-HQ rule cannot trap them). Mortar squads make
  no base refill trip for bombs.
- **CLI.** `s6_derived` per faction (HQ built %, crates issued, delivered,
  lost, crate MP, stock in, the T22 balance `stock_in_not_crate_kg`, base,
  structure, crate and unsourced bombs, forward small arms, items and tank
  rounds, rack credit, cook-offs, tank crate runs) and top-level `s6`.
- **Bridge and HUD.** Intents `build`, `issue_crate`, `place_crate`,
  `pick_up_crate`; state `structures`, `crates`, soldier `crate`, player
  `crate_ready_s` / `crates`, squad `tags`, `supply_refused` feed lines.
  HUD: supply grid mode (Build HQ, Dump, Shells, Ammo, Expl., Place, Pick
  up), the detail panel's supply strip (stock bars with kg and the low
  marker, radio, digging %), roster `CRATE×n` tags, radius and 5 m feed
  rings and the build ghost, `supply_view.gd` boxes. Contract in
  `godot/docs/match_state.md` "S6". Scene `hq_supply`
  (`docs/slices/screens/s6_hq_supply.png`).

### Fixes found on the way

- `supply::mortar_bombs` counted a gunner's pistol magazine as bombs while
  he was on his sidearm (S5a bug, AI and HUD read it); it now counts the
  tube's bomb only while the mortar is the weapon in hand.
- The T20 check first summed every squad's main feed (rifle refills read as
  3,300 "unsourced" bombs per run); it counts mortar squads only.

### Tests

- Sim: 13 S6 tests (`crates/sim/src/s6_tests.rs`): T20 base and dry
  mortar, T21 crate (9 bombs from 32 kg, crate 10 m away feeds nothing)
  and HQ rate, T22 stock in only from crates and up to the cap, T23 only HE
  cooks off (2 bursts from 32 kg) and a bullet stops on a crate (60 -> 56
  hp), T24 D3 rates and stock grams, empty HQ rearms nothing, T25 caps and
  the dead carrier, T26 price 32 MP, no manpower refused, draws free,
  build -> pit dug -> HQ stands -> retreat goes there, place / pick up
  and the carry pace, T12 one hash at 1/2/3/8 threads and save/load/
  continue, every S6 field hashed (52 edits) and 21 invalid saves
  rejected. Two S5a base-bomb tests now set `base_bomb_s = 4` (the S5a
  rule stays for mods that use it).
- Content: setup baseline hashes and the canonical view re-blessed (S6
  layout and content on purpose).
- Godot: `test_match_view` 291/291 (16 S6 checks), `test_data_reload`
  21/21, `test_net_ui` 143/143, settings.cfg unchanged.

### Targets (40-match set, `--runs skirmish40`, the S6 file with the 160
metrics read on the 40 set; scratchpad `s6/S6-40.toml`, `targets40.txt`)

Final binary (`6ffa773`): 39 pass, 8 miss (7 hard), 5 skipped (armour,
fortify, timing not run). Misses: T4a 1 `no_bombs` refusal (as baseline);
T5c/d Germany 65.0 % (in the 40-set band 35-65, outside the 160 band;
baseline 40.0 %); T8 HE share 13.6 % (baseline 13.3 %); T9 mortar share
4.9 / 4.0 % (baseline 160: 5.3 / 4.6). Passes of note: T20 base and
unsourced bombs 0/0, T22 0 kg, T27 HQ built 100 / 100 %, crates delivered
4.7 / 7.5 per match, length median 19.2 min (10.9-27.8), base side
57.5 %, no match without a winner. Reported: crate MP 178 / 204 per
match, forward tank rounds 1.5 / 4.2, cook-offs 0, tank crate runs 0.
(The first run, before the AI checked the issue timer and price, refused
crate orders and spent 259 / 372 MP a match: Germany 67.5 %.)

### What the Tune stage must know

- **Faction balance moved.** Germany 65 % on 40 (baseline 40 %), well
  past 40-match noise. Suspects: the S6 overhead per side (HQ card 160 vs
  200 MP, bearers 80 vs 100, HQ 100, crates ~180-200 MP), the US delivering
  more crates, the end of the base bomb refill. Measure on 160+160 first.
- **Mortars are weaker** without the base refill, as the spec expected
  (bombs per match down ~20 % for Germany). Levers: crate runs to mortars,
  `crate_run_below_pct`, mission size, bomb load, map fire.
- **Never seen in AI play:** cook-offs (0) and tank crate runs (0; the AI
  only brings a crate to an immobile tank under 25 %).
- Open questions S6-11..16 in `docs/TODO.md`.

## Tune stage

Autotune run (spec `tools/autotune/specs/s6_tune.toml`, 17 batches, 680 matches):
see spec "Tune results". Applied: German cards x1.03, `[ai.mix] mortar = 50`.
Targets (40 + 160 + oos): 41 pass, 6 miss (5 hard): T4a 2 `no_bombs`
refusals (AI), T5d 55.6 %, T8a/b HE 18.3 / 17.6 %, T9b USA mortar 7.9 %.
Unreachable by numbers: HE 20-25 % and USA mortar 10 %; mission size and
crate budget do not move them. Needs AI work (mortars fire later in a match,
USA mortars vs Germany) or a code change.
