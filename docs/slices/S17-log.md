# S17 log (team play in the sim; at most 150 lines)
Spec: `docs/slices/S17-team-play.md`. Branch `s17-build` (local, not pushed), base `<BASE>` = `2b19f1c` (S16b docs on
`claude/project-thread-1d8xx3`; SAVE 33, PROTOCOL 17). Commit only your files, never `git add -A`, never `target/`, `.claude/`.

## For the next stage (rewritten by every stage; at most 30 lines)
**State: S17a Build A done** (owner rule 2026-11-25: no balance runs; every new number is a TOML key). Next: Checker after A
(plan 10 step 2: the diff against the plan and `docs/slices/S17-sites.md`), then S17b Build B (A3, A4, A6, Harrow Cross,
`skirmish --teams`, 4-player JSON, the 2v2 set).
**Versions:** SAVE **34** (`PlayerSetup::team`, `[teams]`, `[ai.team]`), PROTOCOL **17** unchanged (no command or message
changes; the setup hash covers `team`).
**Kessel Ridge is neutral:** 40 set at `2b19f1c` and after Build A **same 40/40** (`target/s17a/base`, `target/s17a/new`).
**Read before the checker:** deviations D1-D4 below; the site table's classes; the 2v2 runs on Kessel Ridge with two extra
corner bases made in the test (no four-base map until S17b).

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
