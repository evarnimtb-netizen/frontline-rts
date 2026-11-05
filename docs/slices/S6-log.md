# S6 HQ and crates log

Spec: `docs/slices/S6-hq-crates.md` (sections: Baseline, Code map, Data model,
Save, Sim rules, AI, HUD, Content, Targets, Build order, Owner questions, Tune
results). Baseline JSON: scratchpad `s6/baseline/`. Branch `s6-hq-crates`, not
pushed. Base `1575568`.

Commits: `8305c2b` plan (spec, `targets/S6.toml`, questions S6-1..10),
`c4bcb32` content, sim, CLI, AI, bridge, HUD; `5ba1178` T20 check, AI crate
targets, HUD box positions, Godot checks, baselines; `6ffa773` AI sends only
orders the sim accepts, clippy, tests, log; `6d317f0` log with final targets;
`479244f` Tune (German cards x1.03, `[ai.mix] mortar = 50`); `d355350` review
fixes (F1-F6, F8 done, F7 part); plus the Docs commit of this stage.

## Build

### What was built

- **Content.** `defs/supply/` (new def kind `StockDef`, schema `supply`):
  per faction an HQ (Gefechtsstand / Command Post), a dump (Munitionslager /
  Ammo Dump) and three crates (shells 32 kg, small arms 25 kg, explosives
  20 kg). Pit profiles `mortar_pit` (2), `hq_pit` (3), `dump_pit` (4), all
  round spoil. Cards per faction: Zugtrupp / Platoon HQ (5 men, digs at
  100 %, `max_alive` 1) and Munitionsträger / Ammunition Bearers (4 men).
  Sources in TOML comments; weights marked "verify".
- **Rules.** `[supply]` S6 keys with defaults; `base_bomb_s = 0` = no base bombs (Q4).
- **Sim** (`crates/sim/src/structures.rs`, module docs give the rules).
  Structures, crates, cook-offs, per-player `S6Account` in `MatchState::supply6`.
  Commands `Build`, `IssueCrate`, `PlaceCrate`, `PickUpCrate` with
  `SupplyEvent` refusals. Base issue priced kg x price through `pay`. Carriers
  slow their squad. Sources: stock in from crates, mortar bombs, men's small
  arms and items, D3 tank rearm with the rack credit. Static boxes
  (`StaticBox`, derived per tick) stop bullets, shells and fragments and take
  blast; HE that breaks a box holding shells or explosives starts a
  `CookOff` (bursts of the faction's 75 mm HE, owned by the breaker). Retreat
  goes to the nearest own HQ (changed in review). RNG purposes 30/31.
- **AI** (`ai/supply.rs`). Buys the HQ squad and bearers; sites the HQ on a
  ray-tested reverse slope; crate runs to low mortars, stranded tanks and the
  HQ; small arms to the lowest Defend squad; budget `shell_mp_max_pct`; dry
  mortars walk to an HQ with shells. Tank rearm trips use Move to an HQ or
  dump, else base. Mortars make no base trips for bombs.
- **CLI.** `s6_derived` per faction (HQ built, crates, stock balance T22, bombs by
  source, forward rounds, cook-offs, tank crate runs) and top-level `s6`.
- **Bridge and HUD.** Intents `build`, `issue_crate`, `place_crate`, `pick_up_crate`;
  supply grid, stock strip, `CRATE×n` tags, rings, `supply_view.gd`. Contract in
  `godot/docs/match_state.md` "S6".

### Fixed during build

- `supply::mortar_bombs` counted a gunner's pistol magazine as bombs (S5a bug).
- T20 summed every squad's feed (3,300 "unsourced" bombs); now mortar squads only.

### Tests and targets (build, 40-match set, `--runs skirmish40`)

- Sim: 13 S6 tests (`crates/sim/src/s6_tests.rs`: T12 hash at 1/2/3/8 threads,
  every S6 field hashed, 21 invalid saves rejected). Godot: `test_match_view`
  291/291, `test_data_reload` 21/21, `test_net_ui` 143/143.
- Build targets (40 matches): 39 pass, 8 miss (7 hard). Germany 65 % (baseline 40 %).

## Tune

Spec `tools/autotune/specs/s6_tune.toml`, 17 batches, 680 matches (report in
`tools/autotune/runs/s6_tune/report.md`). The supply knobs could not move mortar
share (4-5 % at every setting), so they were dropped.

- German price x1.033 gives 50 % [0.970-1.089]; applied x1.03 to the German
  infantry cards plus Zugtrupp and Munitionsträger (not the tank).
- `ai.mix.mortar` 50 (29 % of AI buys); +0.6 points mortar kills per 4 weight.
  `mission_rounds` kept at 6 (no clear effect).

Before the Tune (160 set): Germany 61.9 %, mortar kills 4.9 / 3.6 %, HE 13.8 %.

## Review (1575568..479244f, no repo changes)

Checks: `s6_tests` 13 pass; `check.sh quick` clean. Findings and fixes:

- **F1 high, fixed.** One AP round destroyed an HQ and its stock: the stock box
  stood on the spoil (y -1.5 m, 400 hp) and `shell_box_damage` 400 equalled
  its hp. Now the boxes sit on the pit floor, `shell_box_damage` 150. Test
  `f1_one_ap_round_does_not_destroy_an_hq`.
- **F2 medium, fixed.** AI crates landed 6.8 m from the mortar and were never
  picked up (about 57 % of Germany's shells crates unused). Mortar crates are
  aimed at `structures::mortar_gunner`, carriers set down within 2 x `pickup_m`,
  bearers pick up stranded shells crates. Counter `s6_crate_pickups` (hashed).
- **F3 medium, fixed.** A tank gets a crate only when immobile, or when far
  (`tank_crate_far_m` 300) from every source and under 25 % of its rounds. The
  tank holds (Stop) while the crate comes. Test `f3_...`. Tank crate runs on the
  40 set are still 0 because AI tanks never run low (`ai_s3c.rearms` 0).
- **F4 medium, fixed.** Vehicle and crew squads retreat to base; infantry go
  to the HQ only when it is nearer base than they are.
- **F5 low, fixed.** Load rejects dead carriers, non-own builders, the other
  faction's defs, off-map positions. Each has a bad-save entry.
- **F6 low, fixed.** Staged projectile test: 75 mm HE 0.8 m from a crate cooks
  it off, AP does not. Bullet test renamed `t23_a_bullet_stops_at_a_box`.
- **F7 low, part.** Gauges read "Shells 380/600 kg". Not done: structures are
  not selectable (S6-18); the screenshot is not retaken (it still shows the
  old labels and call-in prices).
- **F8 low, fixed.** AT-ditch doc comment back on `dig_at_ditch_now`.
- **F9, not run.** The `base_bomb_s = 4` A/B on 160 matches stays open.

## Final targets

Last full run (after `d355350`, 40 matches, `--runs skirmish40`): 10 pass,
2 miss (both hard), 40 skipped.

- T4a / T4b refusals: 1 `no_bombs` each (MISS; known race S5b-22).
- T5a / T5b win share 50 / 50 % (band 35-65, PASS).
- T6a base max 55 % (PASS). T7 length 10.5-34.5 min, median 17.6 (PASS).
- T8c HE share 19.4 % (max 26.3, PASS). T11a/b AI observed or direct 84.2 / 83.8 % (PASS).
- T15a 0 matches without a winner (PASS). T20 base and unsourced bombs 0 / 0 and T22 0 kg (PASS).
- T27 HQ built 100 % both sides, crates delivered 4.7-7.5 per match (PASS).
- Reported: mortar share 12.8 / 8.6 % (USA below the T9 band, not on the miss
  list; check on 160); bombs from crates 24.3 / 23.0; crate pickups 5.3 / 3.3;
  cook-offs 0.125 / 0.05; tank crate runs 0 / 0 per match.

160-set run on the Tune binary (`479244f`, before the review fixes), 41 pass,
6 miss (5 hard), 5 skipped: T5c Germany 48.75 % (PASS); T5d out of sample
55.6 % (MISS by 0.6, pooled 52.2 %); T6 base side 50 / 56.3 / 58.1 % (PASS);
T7c/d median 17.0 / 17.5 min (PASS); T9a Germany 10.7 % (PASS); T9b USA 7.9 %
(MISS); T8a/b/d HE 18.3 / 17.6 / 18.3 % (MISS, band 20-26.3); T4a 2 refusals (MISS).

Not run after the fixes: `check.sh full`, 160 and out-of-sample sets, `--jobs 3` timing.

## Open owner questions

Full list S6-1..19 in `docs/TODO.md`. Ones that shape play: S6-1 (A) a shells
crate within 5 m rearms a tank; S6-11 (A) the base refuses a crate while busy;
S6-14 (A) a structure whose stock box breaks is destroyed; S6-17 (A) a far tank
holds for a bearers' run; S6-18 (A) structures stay unselectable until a HUD pass.

## Known gaps

- Orchestrator check on `6d88ea1` (320 matches): Germany 50.6 / 51.9 %, base side 64.4 % oos (T6c MISS).
- T9b USA mortar share 9.2 % and HE share 19.1-19.8 % are below band. The
  allowed levers do not reach them; they need AI work (mortars fire earlier,
  more HE from tanks and mortars) or code change.
- Structures cannot be selected (S6-18). The screenshot is stale. Cook-offs and tank crate runs are rare in AI play (0 tank runs).
- The spec `S6-hq-crates.md` is about 312 lines (limit 300).

## What S7 must know

- Balance is not settled. Run S6 vs S6 with `base_bomb_s = 4` on 160 + 160
  matches before tuning anything else, and re-check T9 after the F2 fix.
- Mortar weight 50 gives about 29 % of AI buys to mortar teams. The baselines
  before S6 Tune are not comparable.
- Versions: `SAVE_VERSION` 15 and `PROTOCOL_VERSION` 9 on this branch (bumped
  once; the relay doc is updated). Any further layout change bumps them again.
- Prices are per tonne in the rules file (`shell_mp_per_t` 1000), not per kg.
- JSON `ammo.spend_mp.<kind>` is a total over the run; only `spend_mp.per_match` is per match.
- Every new `SupplyState` field needs a hash, save/load and per-field test entry.
