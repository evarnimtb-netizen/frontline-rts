# S17 log (team play in the sim; at most 150 lines)
Spec: `docs/slices/S17-team-play.md`. Branch `s17-build` (local, not pushed), base `<BASE>` = `2b19f1c` (S16b docs on
`claude/project-thread-1d8xx3`; SAVE 33, PROTOCOL 17). Commit only your files, never `git add -A`, never `target/`, `.claude/`.

## For the next stage (rewritten by every stage; at most 30 lines)
**State: S17b Build B and Presenter P done**, checker-after-A findings fixed (owner rule 2026-11-25: no balance runs; every
new number is a TOML key). Next: orchestrator `tools/verify.sh`, Review (gate: save layout, rules, the sweep), Fix, Docs.
**Versions:** SAVE **34** unchanged since Build A (Build B adds no saved field: A3/A4 read state, the CLI observers are
not saved), PROTOCOL **17** unchanged (net matches stay 1v1).
**Kessel Ridge is neutral:** 40 set after Build B (A3, A4 on) **same 40/40** against Build A (`target/s17a/new`, which is
`d85f622`'s sim: the commits after `15b9367` touch tests, tools and docs only) in `target/s17b/kr`.
**Read before Review:** F3-F5 below (path cap on the 1536 m map, A6 is a report proxy, `winner` is the lowest slot of
the winning team so faction win tables read "germany 40/40" in the default 2v2 layout).

## Build A (S17a, plan 10 step 1)
Commits: layout (content keys, `team`, `side`, `teams.rs`), sweep and rules (T1-T9, A1, A2, A5, SAVE 34), tests (sim named
tests and smoke, content, save fuzz), tools and site table, then docs.
- **Data** (plan 2-3): `PlayerRequest.team: Option<u8>` (serde default, skipped when none); `PlayerSetup.team: u8` (none =
  slot). Resolve refuses one team (`players`) and a team at or over `MAX_MATCH_PLAYERS` (`players[i].team`);
  `MatchSetup::validate` repeats both. `BaseDef.side: Option<u8>` (hint only). `[teams]` (`TeamsTuning`: `shared_sight`,
  `team_tickets`, `wiped_waits` 0..=1, default 1; `pop_pct` 67, `income_pct`, `start_manpower_pct`, `tickets_pct` 100, all
  10..=300) and `[ai.team]` (`AiTeamTuning`: `share_points` 0, `ally_squad_pct` 100, `help_ally` 0, `help_priority` 80,
  `help_demand` 1, `foe_first` 0). All keys in `mods/ww2/defs/rules/skirmish.toml`.
- **`crates/sim/src/teams.rs`**: `Teams` (Copy; per player team number, ally mask, sight mask), built by `Teams::of_setup`
  in `MatchState::new` and `rebuild_derived` (load); `#[serde(skip)]`, never hashed (the setup is). `hostile`, `allied`,
  `sees`, `enemy_seen` (hostile and seen), `known_mask` (mine `known` bits, T2), `foes`, `mates`, `in_team`, `won_by`.
  With everyone alone `allies[p] == sight[p] == 1 << p`, so every converted test is the old one in a 1v1.
- **Sweep** (T1-T3, plan 4): targets (rifle, MG, launcher, tank gun, vehicle MG, AT gun self-defence, focus, HE groups),
  spotting targets (allies are neither observer nor target), rounds and fragments pass allied men, vehicles and static
  boxes (`FlightCtx`/`DamageCtx` carry `teams`), own-burst rules count allies as own (mission bursts hurt them under
  `friendly_fire` and the death is own fire: no kill), kill credit between allies none, thrown items pass allied vehicles
  and pieces, mines and AT mines fire only under hostile men and vehicles, wire and pieces hit by hostile fire only, mine
  knowledge and counter-battery markers shared (T2), allies do not mark each other's guns, intel from team sight, the
  friendly-ahead yield counts allies, capture of an ally's abandoned vehicle or gun refused (T8, `NoCapture`). Load checks
  (vehicle and soldier targets must be hostile) use the setup's table. Site table: `docs/slices/S17-sites.md` from
  `tools/owner_sites.sh` (now 244 owner and 51 sight lines; at `2b19f1c` 318 and 49).
- **Rules** (`rules.rs`): T4 men per team, contested by two hostile teams, the point to the present member with most men
  (ties: lowest slot), an ally on his ally's point acts as owner; T5 `income_pct`; T6 `team_tickets` (gap between the best
  standing hostile team's held value and the team's); T7 `wiped_waits` (a wiped player stays in while an ally stands who is
  neither out nor wiped), last team standing wins (`winner` = its lowest standing slot), none a draw; T9 `pop_cap_of` in
  every pop check (sim buys, recrew, reinforcement, AI buys, AI medic, snapshot), start manpower and tickets.
- **AI**: A1 `MatchState::main_foe` (nearest standing hostile base, ties lowest slot; `foe_first` 1: lowest standing
  hostile slot) in the four sites (`ai/fort.rs` front points, AT site, front lane; `ai/guns.rs`); A2 `Rel::Ally` (priority 0,
  demand 0, skipped in `assign`; `Rel::friendly()` where a site meant "not to take": vehicle point, escort release, lanes,
  arty front, assault, breach), `under_capture` and the `ai/` point and wire tests ask `hostile`; A5 `own_near` and the
  mortar own-men list count allies; `known_enemies`, `known_vehicles` and known enemy mines read team sight.

### Results (breakage checks only)
- Tests (release): sim 1009 passed, 0 failed (15 ignored); content, cli and net all pass (`ipv6_loopback_handshake` skipped).
- Clippy `-D warnings` all targets on sim, content, cli, net: clean. `cargo fmt`: clean.
- Neutral: Kessel Ridge 40 set (seeds 1-20, both bases, 45 min), `2b19f1c` vs Build A: **same 40/40**.
- Determinism: the Kessel Ridge 40 rerun with `--threads 4 --jobs 1`: hashes equal 40/40, neutral against the 1-thread set same 40/40.
- Save fuzz: all save_fuzz tests pass, including the new TEAMS save (2v2 at tick 6000) in the fuzz plan; the layout mirror is at version 34.
- Smoke (named tests): a 4-player 2v2 and a 3-player free-for-all of AIs on Kessel Ridge (real terrain, two extra corner
  bases) run 4 min each with no panic, no soldier, gun or vehicle MG ever aimed at an ally (checked every second), equal
  hashes at 1 and 3 threads and through save/load. The 45-min variant is `#[ignore]` (`s17_smoke_45_min`): passes in release (about 120 s).

### Deviations from the plan, with reasons
- **D1** `[ai.team] foe` is `foe_first` 0/1, not a string: every tuning table is integer keys with a range (`tuning_table!`).
- **D2** An ally's abandoned vehicle or gun is refused with the existing `RecrewRefusal::NoCapture` (no new variant: the
  refusal enum is matched in the bridge's `match_events.rs` and the CLI; a new text is Presenter P's).
- **D3** The plan's "Build A adds the check" for the 8-player mine `known` mask: `Teams::known_mask` keeps players 0..8; no
  setup refusal added (a 9th or 10th player simply never knows a mine). Noted for the checker.
- **D4** The 3- and 4-player smoke runs on Kessel Ridge with two bases added in the test setup (the plan's "test map"); the
  plan's 45-min smoke is an ignored test run by hand. `skirmish` with 3 and 4 players from the CLI is S17b (`--teams`).
- Named tests not written as separate tests: "a fragment passes an ally" and "a mission burst hurts him under
  `friendly_fire`" (covered by `direct_fire_passes_an_ally`'s kill/loss balance and the unchanged S11a2 own-burst tests;
  the code path is the same `allied` test); "an ally's mine does not go off under me" (code: `take_mines` and
  `at_exposure` ask `hostile`; a staged mine test is left for the checker).
- Not done: `owner_sites.sh` is not wired into `tools/check.sh` (plan 11.1); `frontline-cli validate` warns on unequal
  `side` values (`MapFileDef::side_warnings`).

### Findings
- **F1** Wire and blasts: own-burst sparing (`own && !b.ff`) means a frag or tank HE never hurts own or allied men at all
  (S11a2 F2); T3 keeps that. Fine for 2v2, noted because "fragments do not hurt an ally" is a consequence, not a new rule.
- **F2** Mine `known` bits stay one per player; with `shared_sight` the AI's path layer and sweepers read the team mask,
  so a mine one ally finds is avoided by both (the mine layer key counts every player's bits, so it refreshes).

## Checker after A (fixes, commit `5fb56d2`)
- (1) `combat.rs` blast overpressure on structure boxes and crates, and the fragment own-box rule, ask `teams` (an ally's
  boxes are spared like own ones, T3/T8); test `an_allys_he_burst_spares_my_crate` (control: a hostile shell breaks it;
  the test fails with the old `sb.owner != b.player`).
- (2) `fire_mission.rs` observers stay an Own test (no shared fire support, S17-5); noted in `S17-sites.md`.
- (3) More than 8 players refused in `resolve` and `MatchSetup::validate` (`MINE_KNOWN_PLAYERS`; `fort.rs` folded
  slots 8-9 with `min(7)`); content test. This is also Build A's "8-player mine check".
- (4) `tools/owner_sites.sh --check` (a site whose file and text the table lacks fails; cut texts match by prefix) runs
  in `tools/check.sh quick`; the table gained the four `teams.rs` API rows and the A3 row.
- (5) `snapshot.rs` fog masks use `Teams::visible_mask` (in the presenter commit `1dd1fe7`).

## Build B (S17b, plan 10 step 3; commit `e825a63`)
- **A3** `share_points`: at a neutral or enemy point my demand falls by ally squads (lead within the capture radius) x
  `ally_squad_pct` / 100. **A4** `help_ally`: an ally's point with known enemies near or under hostile capture gets
  `help_priority` added and demand `help_demand`; `assign` skips an ally point only at demand 0; helpers get `Defend`.
  Both on in `skirmish.toml` (Rust default off). An AI alone never runs either (`teams.in_team`).
- **A6** stays as built in A1 (works face `main_foe`); the report `ai_second_foe_unbelted` is a CLI proxy (F4).
- **Harrow Cross** `mods/ww2/defs/maps/harrow_cross.toml` as plan 5 (bases, points, sides, three roads: two crossing at
  D and the C-D-E flank road; a 6 m rise under D, one valley pair, four woods pairs; seed 2317). `validate` clean, no
  mirror or side warning. Tests `crates/sim/tests/harrow_cross.rs` (mirror incl. base i onto (i+2)%4, terrain symmetry,
  2v2 resolve and main foes [3, 2, 1, 0], every drive type to the farthest point and mirror base under 8000).
- **CLI** `skirmish --teams 2v2|map`, `--faction2`, `--faction3` (`--difficulty1` still sets player 1);
  `--swap-bases` rotates bases by n/2. A team match's report has `teams` (team, winner team, points held s, help tasks,
  shared point holds s, second-foe point-minutes); a set's JSON `s17_derived` and a text line `S17 teams`. Per-match
  arrays cover every player (two in a 1v1: unchanged). `tools/batch.sh teams OUTDIR` plays the mixed and same-faction
  2v2 sets. Save fuzz `TEAMS` save is now a real 2v2 on Harrow Cross; named test `the_ai_helps_an_allys_threatened_point`
  (control: switch off); CLI test of 1/3 threads, mid-match save/load and swap on Harrow Cross.

## Presenter P (commit `1dd1fe7`)
- Bridge: `FrameCtx` team and sight masks; allies' men, squads (read-only), vehicles, works and supplies drawn; mines by
  team bits; frame `allies`, per player `team`/`ally`, ally AI named "Ally AI"; team win = win; feed "Our ally took
  point X". `MenuChoice.mode` (2v2: you + AI ally of your faction vs two AIs; refused on a two-base map).
- Godot: `UiPalette.ALLY` green, `ally_mask`, `owner_color` colours allies everywhere it is used; `foe_slot()` /
  `ally_slots()` replace `1 - local` (top bar, end screen, pause menu); top bar shows "+ ally faction"; menu Mode row
  (1v1 / 2v2, map follows); `Session.MAPS` Harrow Cross, `--mode`. Ally units are not selectable (selection asks
  `owner == local`, unchanged).
- Not built: a 2v2 mock source, minimap ally markers beyond the colour, a per-team end-screen table, a team ticket bar
  (the top bar shows the local player's tickets, equal across a team with `team_tickets`).

### Results (breakage checks only)
- Neutral: Kessel Ridge 40 set (seeds 1-20, both bases, 45 min) **same 40/40** (`tools/neutral.py target/s17a/new
  target/s17b/kr`).
- 2v2 40 set on Harrow Cross (seeds 1-20, both sides, 45 min, mixed teams Germany+USA each side; `target/s17b/hc1` at
  1 thread, `target/s17b/hc4` at 4 threads): exit 0, no panic; all 40 per-match
  records equal at 1 and 4 threads (hashes 40/40; only the per-match tick p99 differs, a timing).
  Printout only: 40/40 won, no draw, no cap; team 0 18, team 1 22; length median 20.6 min (12.2-34.4); help tasks in
  37/40 matches; two allies holding points at once in 40/40; tick p99 max 5.45 ms (3 jobs, busy machine).
- Tests (release, `--no-fail-fast`): sim, content, cli and godot-bridge 1277 passed, 26 ignored, 1 failed: the bridge's
  `handoff::sim_thread_publishes_frames` (a 400 ms wall-clock frame count; passes alone, failed under the parallel
  load). `resolved_defs` canonical snapshot blessed (Harrow Cross, the two A3/A4 switches). Save fuzz: 12,824
  mutations over 6 saves (the TEAMS save a Harrow Cross 2v2), 0 panicked, 0 bad re-saves. Clippy `-D warnings` all
  targets on sim, content, cli, godot-bridge clean; fmt clean; `owner_sites.sh --check` 0 unclassified (after `snapshot.rs` rows).
- Godot 4.7.2 headless: `check_project` OK (menu, mock, real 1v1 and real 2v2 match on the built bridge);
  `test_match_view` 447/448 (the one miss is the pre-existing "16 vehicles under 0.2 ms" timing on a busy machine;
  0.29-0.54 ms here, it failed the same way before any change); `test_net_ui` 143/143.

### Deviations
- **D5** 2v2 set: the user's 40-match set (mixed teams) is run; the plan's second layout (same-faction) is in
  `batch.sh teams` but not run here; plan 9's "one mid-match save/load per seed" is covered by the CLI test (one seed)
  and the save fuzz, not per seed of the set.
- **D6** A6's report is a proxy: own points nearer a non-main hostile base than the main foe's (point-minutes).
- **D7** Not built: own-fire deaths split own/ally, per-player kills/losses in the text summary, the 2v2 mock.

### Findings
- **F3** Path searches on Harrow Cross hit the 8000 cap at p99 (40 set: 129,677 searches, mean 1539, p99 8000; Linden
  Brook p99 5119, Kessel 1087). The map test's base-to-far-point searches stay under the cap, so the capped ones are
  long in-match routes around structures. Lever: map data or `path_max_expansions` (not changed).
- **F4** `ai_second_foe_unbelted` is 8 point-minutes over 40 matches: each AI faces the base across on its own row
  (main foes [3, 2, 1, 0]) and its points sit nearer that base almost always.
- **F5** `winner` (and so `winner_faction` and the faction win table) is the lowest standing slot of the winning
  team; in the default layout that is the German player of either team. Read team wins from `s17_derived`.

