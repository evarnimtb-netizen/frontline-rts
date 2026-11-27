# S15: the USSR (the third faction): plan

Status: **plan only** (2026-11-26), nothing built. **Every design question settled on its (A)** (section 13; the owner questions
S15-1..13 are in `docs/TODO.md` marked "(A) assumed 2026-11-26"; any can be reopened before Build A starts). Base: branch
`claude/project-thread-aj1tww` at `96d4b4b` (S14 with its Fix; SAVE 33, PROTOCOL 17; PR 12 waits for the owner's merge); `<BASE>` is
that branch's head when Build A starts. Card: `roster-expansion.md` 3.3 and "S15: USSR"; design: `infantry-structures.md` 2.3, 3.5, 4.2;
`vehicles.md` 4.3, 6.6, 8.3; `guns-artillery.md` 4.4, 5, 6; `artillery-on-map.md` 2.3; `SYSTEM_DESIGN.md` 2.3 (matchups, decided
2026-10-31). Working notes: `S15-research-history.md` (figures, sources and what was read), `S15-research-code-audit.md` (file:line
audit, PR 11 overlap); notes are inputs, this file decides. Pattern: `S14-assault-kit.md`.

**Owner rules that bind this slice.** 2026-11-25 (Eva: "chill with the balance runs, just make the architecture in a way that will allow
to fix everything afterwards"): **no balance run, no Tune, no 480, no win-rate gate.** Every number is a TOML key behind a switch; the
proof is the neutral 40 set and one breakage 40 set (section 11); the faction's win rates are **reported, not judged**. The card's "done
when the faction plays AI-vs-AI against both others inside the win-rate band" is **deferred to a later balance pass**; S15 is done when
the faction plays, every match ends, and each new rule fires. Also: features first; damage only from projectile or fragment contact;
history-faithful data with sources, 1944-06 cards; the owner settles a data point only where sources disagree (S15-6).

## 0. Summary and why this shape
- **Size.** The card says 0.6x S3, "mostly data". Read against the code it is about **1.3x**: the sim needs almost nothing (no faction
  or unit name in non-test sim code), but the data is the largest batch yet (14 infantry cards, 5 guns, 7 vehicles, about 22 weapons,
  20 rounds, 7 supply defs; S13's five vehicles alone were 0.5x), there are three small systems, and the number two is written into the
  bridge, the menus, the lobby and the tools. It splits as every engine slice since S10: **S15a** (layout, matchups, riders, plates,
  captured bombs, bayonets; infantry data), **S15b** (guns and vehicles data), **P** (presenter), **S15-ai** (rider job, card table,
  switches on, proof).
- **Germany against the USA cannot change.** N1 gives every S15 def an id that sorts after all existing ids, so no index moves; no
  German or US def gets a rider slot, plate, foreign feed or bayonet. So the default 40 set equals `<BASE>` with the S15 switches off
  **and with them on** (T210, T211).
- **The third faction enters by a pair list, not by code.** `[matchups]` in the rules names who may be picked and who may fight whom;
  the bridge and the menus read it instead of their `FACTIONS` constants. Germany against the USA stays the default everywhere.
- **The small systems reuse what is there.** Riders are S8a passengers on boxes outside the armour, plus one rule (they get off by
  themselves). A plate is one test before `wounds::deal`. Captured bombs open one faction check. ROKS-3, bayonet, AT rifle: data.

## 1. Scope
**S15a.** Build A: the ONE layout commit; matchups M1-M6; riders R1-R6; plates P1-P5; captured bombs X1-X3; bayonets B1; rows W1-W4.
Build B: faction file, 14 infantry cards, small arms, items, supply defs, the ROKS-3. **S15b.** Build C: 5 guns, 7 vehicles, rounds; W5.
**P.** Faction and opponent pick, colours, names, 12 blockouts, riders on the deck, mocks. **S15-ai.** Rider job; the AI's card table
checked habit by habit; switches on; the two 40 sets; Review; Fix; verify; Docs.
**Out (reason).** Win-rate targets and tuned prices (owner rule). 120 mm PM-38 (no heavy mortar exists for any faction: a towed
gun-entity mortar is its own system). ZiS-3 indirect fire (S15-13). Thrown HEAT grenades (a new item kind, S15-5). Sniper pair (no
sniper rule exists). SU-100, ISU-122S (date); ISU-152, T-34/76, M4A2, BM-13 (Q23 open). Per-faction `fire_direction_s`. Captured
Panzerfausts. A 50 mm mortar (S15-6). An Eastern-front map (S15-12). Fire from a vehicle. Rider slots, plates or bayonets for Germany
and the USA. The AI seeking enemy crates. Voices and final art.

## 2. Switches and keys (Build A's layout commit adds all; the stage in brackets turns a switch on)
| Table | Key | Default | Meaning; source |
|---|---|---|---|
| `[matchups]` | `factions` | `["ww2:germany", "ww2:usa"]`; `ww2:ussr` appended in [S15-ai] | M1: the factions a match may name. An absent or empty list: no check (old rules files) |
| | `pairs` | `[["ww2:germany","ww2:usa"]]`; `["ww2:germany","ww2:ussr"]` appended in [S15-ai] | M2: the hostile pairs allowed, in menu order; the first pair holding a faction gives its default opponent |
| | `free` | 0 | M2: 1 = any two different enabled factions may fight ("free skirmish": USSR against the USA, `SYSTEM_DESIGN` 2.3) |
| | `mirror` | 0 | M4 (S15-10): 1 = the menus and lobby offer the same faction on both sides of a 1v1 |
| `[riders]` | `enabled` | 0 in S15a, 1 in [S15-ai] | R1-R6 master switch; 0: every `[[rider_slots]]` is dropped at setup (the tank is today's tank) |
| | `off_on_suppression` / `suppression_pct` | 1 / 100 | R3a: a rider past this share of `[suppression] crouch_suppression` gets everyone off |
| | `off_on_hit` / `off_on_fire` | 1 / 1 | R3b a shell or rocket strikes the vehicle; R3c its main gun fires (card: "a dismount when the tank is hit or fires") |
| | `exit_ms` / `stun_ms` | 600 / 1000 | R3: per man, all slots at once over the sides; stun after a forced exit (a knock-out keeps `[vehicles.transport] stun_ms`) |
| `[body_armour]` | `enabled` | 0 in S15a, 1 in [S15-ai] | P1-P5 master switch; 0: every member `armour` is dropped at setup |
| | `soft_pen_tenths_mm` | 10 | P2: the strength of a bullet whose weapon has no `penetration` table (pistol calibres): 1.0 mm, so a 2 mm plate stops it at any range (Wikipedia "Stalnoi Nagrudnik") |
| | `stopped_damage_pct` / `stops_melee` / `prone` | 0 / 1 / 0 | P3 damage of a stopped hit; P4 strikes; P1 whether a prone man's plate counts |
| `[supply]` | `foreign_ammo` | 0 in S15a, 1 in [S15-ai] | X1-X3 switch; 0: every `[foreign_feed]` is dropped at setup |
| `[melee]` | `bayonets` | 0 in S15a, 1 in [S15-ai] | B1: 1 = a weapon's `[melee.bayonet]` block replaces its `[melee]` block |
| `[ai.riders]` | `jobs` | 0, 1 in [S15-ai] | A1 switch |
| | `squads_max` / `drop_m` / `job_max_s` / `stall_s` / `recheck_s` | 2 / 150 / 240 / 45 / 60 | A1 |
Def keys (Build A). Vehicle: `[riders] capacity, entry, board_ms` and `[[rider_slots]] box` (tank, assault gun, tank destroyer only).
Member: `armour = { mm = 2.0, zone = "torso", arc_deg = 120 }`. Weapon: `[foreign_feed] cartridge, faction, spread_pct`;
`[melee.bayonet] reach_cm, damage, windup_ms, recover_ms`. Ranges: switches 0..=1; ms 100..=30000; s 1..=3600; pct 0..=400; `mm`
0.5..=10; `arc_deg` 1..=360; `capacity` 1..=`MAX_PASSENGERS`. Ablation files, written and not run (`targets/S15-ablations/`): `s15_off`
(all 0, the lists without the USSR: equals `<BASE>`), `no_riders`, `no_plates`, `no_foreign`, `no_bayonets`, `no_rider_ai`, `free`.

## 3. Data model and neutrality (all saved fields in Build A's ONE layout commit; B, C, P and S15-ai change no layout)
- **`SAVE_VERSION` +1** over `<BASE>` (34 on this branch). **No `PROTOCOL_VERSION` change**: no new command (`Embark`, `Disembark` serve
  riders) and the lobby already sends faction ids as strings. PR 11 holds SAVE 33-35 and PROTOCOL 17-18 for S16-S18 and this branch
  holds 33 / 17 for S14: **whichever merges second renumbers** (risk 1).
- `PlayerState::s15: S15Counters`: `plate_hits`, `plate_stops`, `plate_stops_frag`, `plate_stops_melee`, `rider_boards`,
  `rider_exits[5]` (ordered, suppressed, vehicle hit, gun fired, knocked out), `rider_casualties`, `foreign_drawn`, `foreign_fired`,
  `bayonet_strikes`, `bayonet_hits`. `AiCounters` += `rider_jobs`, `rider_job_ends[5]` (dropped at the goal, forced off, tank lost,
  deadline, stall). `AiState::riders: Vec<RiderJob>` (ascending tank id): `RiderJob { tank: SimId, squad: SquadId, point: u8, phase:
  u8, phase_at: u64 }`. `Kit` += `foreign: u16` (foreign rounds a man carries; left out, and said, if the per-cartridge store already
  holds any cartridge: audit 8).
- Setup only, no state: rider slots in `TransportSetup` (`exposed: true`); a member's `plate`; a weapon's `foreign_feed` and `bayonet`.
  `VehicleState::riders` is sized by the slots, so a tank without them saves as today. No new RNG purpose (P2 is a threshold).
- **N1 Append rule.** Every def S15 adds has an id starting `ww2:ussr_` (units, vehicles, supply, and also weapons, ammo, cartridges,
  items, e.g. `ww2:ussr_mosin_9130`): it sorts after every existing id of its list (audit 2), so no German or US def changes its index.
  Test `s15_defs_sort_last`: for each of the seven lists, the ids without the prefix are the `<BASE>` list, in order.
- **N2 Drop rule.** With a switch at 0 the setup build removes the rider slots, plates, foreign feeds or bayonet blocks before anything
  reads them (the S14 `when` pattern, `content/setup.rs` 400-406). Test `s15_off_setup_has_no_s15_field`.
- **N3 Load validation** `load_rejects_bad_s15_state` (a rider job on a tank without slots, a phase out of range, `Kit::foreign` without
  a foreign feed); `save_fuzz.rs` staging: men riding a tank, a rider job in each phase, a mortar man carrying foreign bombs.

## 4. Matchups: how the third faction enters (Build A; content, CLI, bridge; no sim change)
- **M1 Who may be picked.** `ModDefs::resolve` refuses a player whose faction is not in `[matchups] factions` (`content/setup.rs` 1494).
  With the USSR off, `ww2:ussr` is data that cannot be played; tests and rows pass an override.
- **M2 Who fights whom.** For every two **hostile** players of different factions the unordered pair must be in `pairs`, unless `free`
  is 1. "Hostile" is "another player" today and `teams.hostile` after PR 11 (allies may be any enabled factions: S17-8 (A)). Pairs for
  1944: Germany-USA (West), Germany-USSR (East); USSR-USA only as free skirmish (`SYSTEM_DESIGN` 2.3).
- **M3 Default opponent.** The other member of the first pair that holds the faction: Germany -> USA, USA -> Germany, USSR -> Germany.
  The bridge's `FACTIONS` constant and "the other faction" branch (`bridge/match_setup.rs` 69, 138-142) go: `MenuChoice` gains
  `opponent: Option<String>` (`None`: M3) and the menus get the faction list and pairs from the loaded rules. A human who picks Germany
  sees "Opponent: USA / USSR" (USA first); with the USSR off the picker has one entry and is hidden: **the menu is today's menu.**
- **M4 Mirror (S15-10 (A)).** Menus and the 1v1 lobby keep "factions must differ" (`match_setup.rs` 160, `net/relay.rs` 893) while
  `mirror` is 0; the CLI resolves mirror runs as today. Team modes follow PR 11's S17-8.
- **M5 Slots, bases, maps.** Unchanged: the human is slot 0 at base 0, the AI slot 1 at base 1; no map has a faction-bound base; the
  USSR plays on Kessel Ridge and any two-base map present. AI-vs-AI: `skirmish --faction0 A --faction1 B` exists, defaults stay
  `ww2:germany` / `ww2:usa`; `--both-bases` plays each seed from both bases, which separates the map side from the faction.
- **M6 Online.** No protocol change. Each player picks any enabled faction; the relay still only knows "differ". A pair M2 refuses
  fails in both clients' `resolve` alike, before `Ready`; the lobby shows the reason. `free` and `mirror` are rules values, equal on
  both clients by the content hash.
- Tools: `tools/batch.sh pair OUTDIR F0 F1 [-o FILE]...` (one 40 set: seeds 1-20, both bases; `pair_<f0>_<f1>40.json`); `pool.py`
  prints each faction present; `cli/validate.rs` 135-139 resolves every allowed pair, not "the first two factions".
- Tests: `matchup_refuses_a_faction_not_enabled`, `matchup_refuses_usa_ussr_without_free`, `free_allows_any_two`,
  `default_opponent_is_first_pair`, `menu_choice_without_opponent_is_base`, `validate_resolves_every_pair`.

## 5. Rules (Build A; integers and `Fx`, ascending ids)
### 5.1 Riders
- **R1 Slots.** `[riders]` and `[[rider_slots]]` on a tank, assault gun or tank destroyer def (the check at `content/vehicle.rs`
  2557-2577 allows them there). They lower into the same `TransportSetup` as passenger slots, marked `exposed`.
- **R2 Boarding.** `Embark` as for a half-track (`sim/transport.rs` 162-204): any own infantry squad that fits; entry at the hull's
  rear. `Disembark` as today.
- **R3 Getting off by themselves** (new; Status stage, ascending vehicle id). All riders exit over the sides, `exit_ms`, then `stun_ms`,
  when (a) a rider's suppression passes the threshold, (b) a shell, AT rocket or mine strikes the vehicle (any hit that reaches
  `damage.rs`), (c) its main gun fires. The vehicle keeps its order. (d) Knock-out, abandon, fire: the S8a rule, unchanged.
- **R4 Exposure.** A rider's box sits outside every plate: every bullet, fragment and flame particle that touches it hits him (the S8a
  slot-box test). Validation: no rider box inside a hull or turret box.
- **R5** Riders see as passengers do; they do not fire, capture or contest (S8a). **R6** A turret turns through them (not modelled).
- Tests: `riders_board_a_tank`, `rider_hit_on_the_deck` (control: the slot empty), `riders_off_when_suppressed`,
  `riders_off_when_tank_is_hit`, `riders_off_when_gun_fires`, `riders_off_is_base`, `riders_do_not_capture`, save/load mid-exit.
### 5.2 Plates
- **P1 When.** A bullet, fragment or strike hits a plated man in the plate's zone (`ZONE_TORSO`, `sim/wounds.rs` 23) from his front:
  the angle between the round's reversed direction and his facing is at most `arc_deg / 2`. Prone men: `prone` (0: no plate).
- **P2 Test** (no draw). The round's strength: a bullet's `penetration` table at the flown range; without a table
  `soft_pen_tenths_mm`; a fragment its `frag_pen_mm` with the S3b thin-plate falloff. Strength >= the plate: through, nothing changes.
- **P3 Stopped.** Damage x `stopped_damage_pct` (0) before `deal`; the hit still gives `[combat] hit_suppression`.
- **P4** With `stops_melee` 1 a butt or bayonet strike that meets P1 is stopped. **P5** Blast, flame and burning ignore the plate.
- Tests: `plate_stops_smg_from_the_front`, `plate_does_not_stop_a_rifle`, `plate_does_not_cover_the_back`,
  `plate_does_not_cover_head_or_legs`, `plate_stops_a_light_fragment_only`, `plate_stops_a_strike`, `flame_ignores_the_plate`,
  `plates_off_is_base`.
### 5.3 Captured bombs
- **X1 Draw.** A man whose weapon has `[foreign_feed]`, at a crate **his player holds** of the feed's faction and supply kind
  (`sim/structures.rs` 1085-1150, the one check opened), takes foreign rounds at the resupply rate, at no price (the enemy paid), only
  while his own rounds are below his load. Crates only: structures and trucks are not captured in the game.
- **X2 Fire.** Own rounds first, then foreign ones at `spread_pct` (130) of the spread; the burst is the weapon's own `[he]` (audit 8).
- **X3 One way.** Only `ww2:ussr_82mm_bm37` has a foreign feed (`ww2:8cm_wgr34`, `ww2:germany`); the GrW 34 has none.
- Tests: `soviet_mortar_draws_german_bombs`, `german_mortar_draws_no_soviet_bombs`, `foreign_bombs_fly_wider`, `own_bombs_first`, `foreign_off_is_base`.
### 5.4 Bayonets
- **B1.** `[melee] bayonets` 1: a weapon with `[melee.bayonet]` strikes with that block (S14 M1-M5 unchanged); only the Mosin has one;
  no order, no aim penalty (S15-7 (A)). Tests: `bayonet_reaches_150_cm`, `bayonet_misses_at_190_cm`, `bayonets_off_uses_the_butt`.

## 6. Data (Build B infantry, Build C guns and vehicles; every value with `sources`; figures and marks in `S15-research-history.md`)
Prices, cooldowns and weights are **starting values copied from the nearest German or US card, not tuned**; the German 20 % cut
(S5a-37) is not applied. Each def carries a comment "S15 starting value". Figures marked (verify) in the note are read from a source
in the data stage before they enter a def.
### 6.1 Infantry cards (`defs/units/ussr_*.toml`; faction `ww2:ussr`, colour `#9a5a4a`; start squads: two rifle sections and the sappers)
| Card | Men | Kit | Tags; capabilities | Cost | `ai_weight` |
|---|---|---|---|---|---|
| `ussr_strelkovoe_otdelenie` | 9 | leader PPSh-41; DP-27 gunner (TT-33) and assistant; 1 PPSh; 5 Mosin; F-1 x1 each, 1 smoke | line; dig_basic, recrew | 200 | 100 |
| `ussr_strelkovoe_otdelenie_6` (`extends`) | 6 | leader PPSh; DP pair; 3 Mosin | line, understrength | 140 | 50 |
| `ussr_gvardeyskoe_otdelenie` (`extends`, training `veteran`) | 9 | as the section, 3 PPSh | line | 240 | 0 |
| `ussr_otdelenie_avtomatchikov` | 9 | 7 PPSh-41 (the leader one of them), DP pair kept; RG-42 x2 each | line, riders | 240 | 60 |
| `ussr_pulemyotny_raschyot` | 6 | Maxim M1910/30 (S15-11 (A); `ussr_sg43` is a def without a card), 3 bearers | mg | 160 | 100 |
| `ussr_sapyornoe_otdelenie` | 8 | Mosin, 2 PPSh; satchel, wire and mine kit as the US engineers | engineer; the US engineer list | 220 | 40 |
| `ussr_shturmovaya_gruppa` | 8 | PPSh; **SN-42 on every man**; a ROKS-3 `when` pair as S14 4.2; satchels, smoke | engineer, assault; breach, clear_wire, clear_mines, demolish | 270 | 60 |
| `ussr_minomyotny_raschyot` | 5 | 82-BM-37, 2 bearers (the 5-man rule) | mortar | 250 | 100 |
| `ussr_raschyot_ptr` | 2 | PTRD-41, PPSh; satchel x1 | at | 110 | 100 |
| `ussr_upravlenie_vzvoda` | 3 | PPSh; builds the HQ | hq | 170 | as the US card |
| `ussr_podnoschiki` | 4 | Mosin | bearers | 90 | as the US card |
| `ussr_sanitary` | 2 | TT-33 | medic | 80 | 10 |
| `ussr_razvedchiki` | 2 | PPSh, radio | observer | 110 | 0 (`[ai.arty] fo` buys it) |
| crews: `ussr_tank_crew`, `ussr_gun_crew_*` | | TT-33, PPSh | crew | | |
### 6.2 Small arms, items, supply (reach / damage / rates from the note, section 2)
`ussr_mosin_9130` (5 rounds, `penetration` as the Kar98k class; `[melee]` butt 100 / 50 / 600 / 1000 and `[melee.bayonet]` 180 / 90 /
500 / 1000: `infantry-structures.md` 4.2 gives the reach, damage chosen so a torso thrust downs a man), `ussr_ppsh41` (35-round box),
`ussr_pps43`, `ussr_dp27` (47-round pan, bipod), `ussr_maxim_1910` (250 belt, tripod-class), `ussr_sg43` (no card), `ussr_tt33`,
`ussr_ptrd41` (class rifle, crew weapon, `penetration = [[0, 40], [100, 38], [300, 33], [500, 27]]` (verify): the S8a small-arms
rule decides what it holes), `ussr_roks3` (class flame: 35 m, 96 units = 10 L, bursts of 12, cartridge `ussr_roks3_fuel`),
`ussr_82mm_bm37` (bomb `ussr_82mm_o832` 3.1 kg, charges to 211 m/s, the compressed 600 m range of the other mortars; `[foreign_feed]`).
Cartridges `ussr_762x54r_clip`, `_pan`, `_belt`, `ussr_762x25`, `ussr_145x114`. Items `ussr_f1_grenade`, `ussr_rg42_grenade`,
`ussr_rdg_smoke`, `ussr_satchel`, `ussr_uz_charge` (bangalore kind); mines are the shared defs. Supply: four `ussr_*_crate` defs,
`ussr_komandny_punkt` (HQ), `ussr_sklad_boepripasov` (dump), `ussr_medpunkt` (aid), each a copy of the US def's numbers.
### 6.3 Guns (role; mass; main rounds; card cost; tow `ussr_us6_squad`)
| Def | Role | Mass kg | Rounds | Cost | Notes |
|---|---|---|---|---|---|
| `ussr_45mm_m42` | at_gun | 625 | BR-240 AP, BR-240P APCR (rare), O-240 HE | 200 | the light gun (as the Pak 38 card) |
| `ussr_57mm_zis2` | at_gun | 1,250 | BR-271 AP, BR-271P APCR (rare), O-271 HE | 360 | the 1944 tank killer |
| `ussr_76mm_zis3` | at_gun | 1,116 | BR-350A AP, BR-350P APCR (rare), OF-350 HE | 340 | direct fire only (S15-13 (A)) |
| `ussr_76mm_m1943` | inf_gun | 600 | OF-350 HE (262 m/s), BP-350M HEAT off (no HEAT rule in the game) | 300 | gun missions as the leIG 18 |
| `ussr_122mm_m30` | inf_gun, tag `field_howitzer` | 2,450 | OF-462 HE 21.8 kg | 520 | as the leFH 18 card: cap 1, unlock 8:00 |
### 6.4 Vehicles (`role`; crew; main gun and load; armour boxes as S13: nose, superstructure, turret; card cost)
| Def | Role | Crew | Gun, rounds | Armour (mm at deg from vertical) | Cost | AI |
|---|---|---|---|---|---|---|
| `ussr_t34_85` | tank; **8 rider slots** | 5 | 85 mm ZiS-S-53, 55 (verify; one source 35), 2 DT | hull 45 at 60 front, 45 at 40 upper side, 45 at 0 lower side, 45 at 48 rear; turret 90 / 75 / 52 | 640 | 100 |
| `ussr_su85` | tank_destroyer (casemate); 6 rider slots | 4 | 85 mm D-5S, 48 | 45 at 50 front, 45 sides | 500 | 100 |
| `ussr_su76m` | assault_gun, open top | 4 | 76 mm ZiS-3Sh, 60 | 25-35 front, 10-16 sides, open top and rear | 330 | 50 |
| `ussr_is2` | tank; 8 rider slots; `heavy_wave`, `ai_heavy` | 4 | 122 mm D-25T, 28, 2-3 rounds/min | hull 120 at 60, sides 90, turret 100 | 1,100 | 0 (H1 buys it) |
| `ussr_ba64b` | recon | 2 | DT in an open-top turret | 4-15 sloped | 170 | 5 |
| `ussr_us6` | truck with `[tow]` and `[supply]` | 2 | none | none | 200 | 5 |
| `ussr_m3a1_scout` | transport, open top, 6 passengers (S15-9 (A)) | 2 | .50 M2HB and .30 on the rail (the US weapon defs) | 12.7 front, 6.35 sides | 240 | 100 |
New guns `ussr_zis_s_53`, `ussr_d5s`, `ussr_zis3`, `ussr_d25t`, `ussr_dt`, with rounds BR-365 / BR-365P (rare, `rare_cap` 4) / O-365,
BR-471 / OF-471, and the gun rounds of 6.3. Soviet penetration tables use a 0-degree criterion: converted to the mod's convention in
Build C, said in each ammo def; the seven AP rounds opt into the S13 obliquity rule by `slope_class`. The IS-2's slow loading is its
`reload_ms`, its 28 rounds its rack: no rule.

## 7. S15-ai (`ai/riders.rs`; every key and saved field already in the layout commit, all off)
- **A1 Rider job** (`jobs`). Want: an own vehicle with free rider slots at base that the armour planner sends to attack point `i`, and
  an own squad at base with no job that fits (cards tagged `riders` first, then `line`); at most `squads_max` jobs. Phases: **Board**
  (`Embark`; the tank waits), **Ride**, **Drop** (`Disembark` at `drop_m` short of the point, or R3 did it); the squad then joins the
  point's attack. Ends (counted): dropped, forced off, tank lost, `job_max_s` (L1), no progress for `stall_s` (L2); no retry of a
  failed pairing for `recheck_s` (L3).
- **A2 The card table** (a test, not a rule): each AI habit finds its USSR card by the tag, capability or role it already uses: mix
  slots (6.1, 6.4), builder (sappers), HQ and bearers, medic, observer and guns (`[ai.arty]`: M1943, then the M-30), heavy (IS-2), flame
  (assault group), recon (BA-64B), truck and tow (US6), transport (scout car), AT guns. `ai_habits_find_ussr_cards`;
  `ai_tolerates_an_empty_mix_slot` (a test roster without an `at` card buys on).
- Liveness tests: `rider_job_ends_when_tank_dies`, `..._on_deadline`, `..._on_stall`, `rider_job_drops_short`; save fuzz per phase.
  Then the USSR joins `[matchups]`, the four switches and `[ai.riders] jobs` go to 1; JSON blocks `s15`, `s15_derived`.

## 8. Presenter (P; after Build C; no balance meaning)
Faction buttons and opponent picker from the setup info (M3); USSR colour, name, short name `SOV`; weapon names (`match_view.gd`
98-99); paint and glacis (`vehicles.gd`); 12 blockouts by the S13 recipe; riders seated on the deck boxes; a plate mark on the squad
card; lobby text for a refused pair; mocks and `test_net_ui.gd` with three factions; headless smoke.

## 9. Determinism, cost, guard rails
No new RNG purpose. R3 and rider jobs run serially in ascending id; P2 runs where `hurt` runs and writes only the hit man's chunk. Tick
p99 from `tools/verify.sh` only, at most 25 % over `<BASE>` on the default pair. Hashes equal at 1, 2, 3, 8 threads and through
save/load with men riding, a job in each phase and foreign bombs carried. Every new owner test in `sim/` goes through one helper per
file (`hostile_to`), so the PR 11 merge swaps it for `teams.hostile` in one place and `owner_sites.sh --check` has few rows to add.

## 10. Rows W1-W5 (mechanics, not balance; switches on in the row only; player commands, no AI)
- **W1 Plate (hard).** 100 MP 40 bursts at 50 m at the chest of a standing plated man: `plate_stops` > 0 and he outlives the unplated
  control by at least 2x the rounds; a Kar98k at the same range: no difference; the MP 40 from behind: no difference.
- **W2 Riders (hard).** A 6-man section boards a T-34-85, rides 300 m; a mortar bomb bursts 10 m off: all riders are off within 3 s; a
  second run, an AT hit knocks the tank out: every surviving rider is off and stunned. Control (switch 0): `Embark` refused.
- **W3 Captured bombs (hard).** A Soviet mortar crew, dry, at a taken German shells crate: it draws and fires; the mean miss distance of
  20 foreign bombs is larger than of 20 own bombs; a German crew at a Soviet crate draws nothing.
- **W4 Bayonet (hard).** 100 strikes at a man 1.5 m ahead hit 100; at 1.9 m hit 0; with `bayonets` 0 the 1.5 m strike misses.
- **W5 Armour duels (report; `armour-test` rows as S13's 41-46).** T-34-85 against Pz IV H both ways at 500 m; against the Panther's
  front and side; IS-2 against Panther and Tiger at 800 m; SU-85 against Tiger; ZiS-2 against the Panther's side; 45 mm against Pz IV;
  PTRD against the Sd.Kfz. 251 and the Pz IV's side at 100 m. Each row prints kills and rounds; none has a threshold.

## 11. Measures (the owner rule: two 40 sets, no 480, no gate on a win rate; `docs/slices/targets/S15.toml`, T210 on)
- **T210 (hard) neutral, switches off.** Default pair, 40 set with `s15_off`: `tools/neutral.py` "same 40/40" against `<BASE>`. Run
  after Build A, after Build C and at the end.
- **T211 (hard) neutral, switches on.** The same 40 set at the shipped defaults (USSR enabled, every S15 switch 1): also "same 40/40"
  against `<BASE>` (section 0). A difference here is a bug, not a balance matter.
- **T212 (hard) breakage, the faction in play.** `tools/batch.sh pair DIR ww2:germany ww2:ussr` (seeds 1-20, both bases): 40 of 40 with
  a winner; no panic; every match between 10 and 40 min; hashes equal at 1 and 4 threads on seeds 1 and 2.
- **T213a-f (hard) each new rule fires in that set:** `plate_stops` > 0; `rider_boards` > 0 and `rider_exits` > 0; a ROKS-3 hit
  (`flame_hits` of the USSR) > 0; `bayonet_strikes` > 0; every USSR card with an AI path bought at least once over the 40 (the list in
  the report; T-34-85 and one gun at least are hard); `rider_jobs` > 0 with no job older than `job_max_s` + 5 s at a match's end.
  Captured bombs fire by row W3 only (the AI does not seek crates): `foreign_fired` in the set is a report.
- **Reports, not judged:** T214 the USSR's and Germany's wins of T212, by base; T215 the free pair (`batch.sh pair DIR ww2:usa ww2:ussr
  -o free.toml`): winners, lengths, wins; T216 length quartiles; T217 USSR kills by class and HE share; T218 riders hit while riding; W5.
- Left for the balance pass: the 480 per pair and the 35-65 % band per pair (never one pooled number for three factions,
  `workflows.md` 1), every price and weight of section 6, `soft_pen_tenths_mm`, R3's three switches, the ablation files.

## 12. Stages (`workflows.md` 1-4; models and effort by its table 3, named here by tier; stage briefs `S15a-build-X.md` at each start)
| Part | Stage | Model tier (`workflows.md` 3) | Content | Exit |
|---|---|---|---|---|
| S15a | A engine (2 chunks) | engine Build | ONE layout commit (3, every key of 2, SAVE +1); M1-M6 with the tools; R1-R6; P1-P5; X1-X3; B1; rows W1-W4 on stand-in defs in `mods/ww2/tests/` | `check.sh quick`; T210 |
| | Checker | data Build | diff against this file, named tests, neutral check | list of misses |
| | B infantry data (2 chunks) | data Build | 6.1, 6.2, faction file, supply defs; N1 test; A2's empty-slot test; baselines (`BLESS=1`); a 4-seed Germany-USSR smoke with the override | `check.sh quick`; T210 |
| S15b | C guns and vehicles (2 chunks) | data Build | 6.3, 6.4, rounds; W5; baselines | T210; W5 printed |
| P | presenter (1 chunk) | data Build | section 8; headless mock smoke | |
| S15-ai | D AI (1 chunk) | engine Build | A1, A2, liveness; switches on; JSON `s15`; `S15.toml`; the tools of M | `check.sh tests` |
| | Proof | script | T210-T218 (`tools/batch.sh`, `neutral.py`, `targets`) | T210-T213 |
| | Review, Fix | Review tier, Fix tier | the review gate fires (sim, save layout) | findings fixed |
| | Docs | Docs tier (low effort allowed) | log, STATE (versions, "three factions"), TODO S15 block, `workflows.md` S15 size, `docs/modding/factions.md` (how to add a faction: N1, matchups) | |

## 13. Design questions: all settled on (A) (2026-11-26; the first affected part in brackets; full options in `docs/TODO.md` S15-1..13)
- **S15-1 Who the USSR fights [S15a].** (A) Germany-USA stays the default; Germany-USSR is the second pair; USSR-USA only as "free skirmish", off by default.
- **S15-2 Roster of the first build [S15a].** (A) The full list of section 6 (14 infantry cards, 5 guns, 7 vehicles). (B) A core first.
- **S15-3 Tank riders [S15a].** (A) Riders get off by themselves (suppressed, tank hit, gun fires); only Soviet tanks have rider places.
- **S15-4 The breastplate [S15a].** (A) A real 2 mm plate on the front torso of the assault group: stops pistol calibres, light fragments, strikes.
- **S15-5 Soviet infantry against tanks [S15a].** (A) AT rifle team, satchels, three AT guns; thrown HEAT grenades wait for their item kind.
- **S15-6 The 50 mm company mortar [S15a; sources disagree].** (A) No 50 mm card (design P1). (B) An RM-38 card (battleorder.org).
- **S15-7 Bayonets [S15a].** (A) Soviet riflemen strike with the fixed bayonet (1.8 m), no aim penalty, no order; others stay without.
- **S15-8 Captured bombs [S15a].** (A) The 82 mm fires German 8 cm bombs from a taken crate, less accurately; the AI does not seek crates.
- **S15-9 A transport card [S15b].** (A) The lend-lease M3A1 Scout Car (6 men) beside the tank riders. (B) Riders only.
- **S15-10 The same faction on both sides of a 1v1 [S15a].** (A) Refused in menu and lobby as today; the CLI allows it; teams follow S17-8.
- **S15-11 The MG card [S15a].** (A) Maxim M1910/30; the SG-43 is a def without a card.
- **S15-12 A map for the East [S15-ai].** (A) None in S15: the USSR plays on the existing maps.
- **S15-13 The ZiS-3's second role [S15b].** (A) Direct fire only; indirect fire and its artillery-cap rule later.
Settled in the plan only (owner rule 2026-11-11): prices, cooldowns and weights of section 6 as copied starting values; 9 and 6 men;
ROKS-3 10 L; T-34-85 55 rounds, sloped upper side; the PTRD table; the `ussr_` id prefix (N1); `SOV`, the colour; T210 numbering.

## 14. Risks
1. **PR 11 (S16-S18) overlaps** (audit 9). Numbers: both hold SAVE 33 / PROTOCOL 17 already; S15 takes SAVE 34, PR 11 34-35 and
   PROTOCOL 18: the second merger renumbers, regenerates `resolved_defs.rs` and `ww2_canonical.json` (`BLESS=1`, never by hand) and
   reruns T210. Lines: `bridge/match_setup.rs` (`resolve_with`), `net_session.rs`, `session.gd`, `main_menu.gd`, `multiplayer_panel.gd`,
   `palette.gd`, the `sets` loop of `batch.sh`, the end of `skirmish.toml`. Rules: M2 must read `teams.hostile`; S15's new owner tests
   must be in `docs/slices/S17-sites.md` (`owner_sites.sh --check`). Mitigation: M lives in the content crate (`resolve`), so both
   menus call one function; P starts when the merge order is known.
2. **Neutral with the switches on (T211) is a strong claim.** One German or US def that gains an S15 key, or one USSR id that sorts
   before an old one, breaks it. N1's test and T211 after every data build catch it; the fallback is T210 only, said in the log.
3. **Three small systems, no tuning.** Riders may die in heaps (R3 puts them on the ground under fire), plates may do little (rifles
   and MGs do most killing), the AT slot is weak (S15-5). None is judged here; each has its switch and keys for the balance pass.
4. **The AT rifle on the S8a rule.** Whether a rifle-class weapon with a 40 mm table picks vehicles by itself is not audited: Build B
   tests `ptrd_engages_a_halftrack`; else the PTRD takes the launcher's target rule by a def key.
5. **Data volume and (verify) marks.** About 60 defs, mostly from secondary sources; the Soviet 0-degree tables need converting. Each
   def's `sources` names what was read; W5 is the smoke test for nonsense values.
6. **The AI was written against two rosters** (A2 walks every habit; T212 is the net) **and match length** with cheap infantry and no
   rocket AT (a match over 40 min fails T212; the levers are the USSR cards' `ai_weight`, then S15-5 (B): asked of the owner, not tuned).
7. **`Kit` layout** (audit 8) and **R3 on a moving tank** (whether `rider_out` needs a halted vehicle) are read in Build A's first
   chunk; if a halt is needed the tank brakes for `exit_ms`, and the stage brief says so.
