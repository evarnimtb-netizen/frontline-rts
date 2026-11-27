# S17: team play in the sim, AI allies, a four-base map (plan)

Status: **plan**, written 2026-11-25 from `claude/project-thread-1d8xx3` at `cdc404c` (B3 code merged; SAVE 32, PROTOCOL 16 there). Nothing
here is built. S16 (second map, roads) is being built in this thread; S14 (assault kit) and S15 (USSR) are being built in another.
`<BASE>` is the last commit of whichever of S14, S15, S16 has merged when S17a Build A starts; every file:line below was read at `cdc404c`
and Build A re-reads them after its rebase (S16a is editing `content/setup.rs`, `defs.rs` and `skirmish.toml` in the working tree today, so those lines are the committed ones). Pattern: `S16-second-map.md`. Owner rules: features first; **no balance runs** (owner
2026-11-25: "make the architecture so balance can be fixed afterwards"), so every new number is a TOML key or sits behind a switch, and the
checks in section 9 are breakage checks only. The older TODO wire item "S17" (wire path layer) is a different id.

## 0. Why this item, what it costs, and the split into S17 and S18
- **Why.** SYSTEM_DESIGN 2.5 makes PvP 1v1 to 5v5 and co-op vs AI the launch scope; the game is 1v1 only. S16's plan (section 0) named
  2v2 as S17. With S15 there are three factions and with S16 two maps; the next thing a player misses is a second player on his side.
- **What the code says** (read 2026-11-25):
  - **The sim holds N players but has no teams.** `PlayerId(u8)` (`ecs.rs:60`), up to 10 players (`setup.rs:2632`, check `:3547-3551`),
    rules loop over `players.len()` (`rules.rs:92`, `:108`, `:138`, `:202`). But "enemy" is "any other owner" everywhere: 323 raw owner
    comparisons in 42 non-test sim files (most are "mine", perhaps a third are "enemy"), and 55 uses of the per-player sight bit
    `Combat::seen_by` (`ecs.rs:236`, `spotting.rs:3`). There is no `is_enemy` function. No test or tool has ever run a third player
    (no `PlayerId(2)` in the tree), so N-player bugs are latent.
  - **Rules are free-for-all.** Capture counts men per player and freezes when two players are present (`rules.rs:138-164`); ticket
    drain is the gap to the best other player (`rules.rs:90-100`); the last player standing wins (`rules.rs:236-253`);
    `Outcome::Won { winner: PlayerId }` (`game.rs:662`).
  - **The AI knows one enemy.** Four places take "the first player who is not me" (`ai/fort.rs:464`, `:699-701`, `:1031-1033`;
    `ai/guns.rs:1049-1052`); every point not mine is `Rel::Enemy` (`ai.rs:2311-2315`), so an AI would attack its ally's points;
    `known_enemies` reads its own sight bit (`ai.rs:1612-1618`).
  - **The net layer is two slots by type.** `SLOTS = 2` (`protocol.rs:56`), `LobbyState.slots: [_; 2]` (`:293`), `Start.factions` and
    `names: [String; 2]` (`:322-323`), desync `hashes: [u64; 2]` (`:263`, `client.rs:126`, `dump.rs:47`), the relay's slot array
    (`relay.rs:263`, `:294`) and `try_start` on slots 0 and 1 (`relay.rs:884-893`), 20 two-element arrays in the harness
    (`harness.rs:778-821`), and the bridge (`match_setup.rs:33`, `match_net.rs:92`, `net_session.rs:609`, `:622`).
  - **The presenter knows own and enemy.** `owner_color` returns OWN or ENEMY (`palette.gd:41-45`); the frame filter is one sight bit
    (`match_frame.rs:423`, `:541`, `:563`); feed and end texts say "the enemy" (`match_events.rs:250`, `:323`); the menus pick one
    faction and one opponent (`main_menu.gd:153`, `session.gd:57`).
- **Cost.** S16's guess was 1.5x S3 for all of 2v2. Read against the code it is about **1.8x S3**: the owner-comparison sweep and the
  never-run third player are the sim half, the two-slot wire types are the net half. That is too large and too risky for one slice, and
  the two halves share almost no files. So:
  - **S17 (this file)**: teams in the sim, AI allies, a four-base map, 2v2 from the CLI and offline from the menu (you, an AI ally, two
    AI opponents). About **1.0x S3**.
  - **S18** (`S18-online-teams.md`): four-slot relay and protocol, team lobby with AI fill (so co-op vs AI online), hand-over of a dropped
    ally's troops, net presenter. About **0.8x S3**.
- **Split per the owner rule (2026-11-09).** **S17a mechanics**: teams, rules, the sweep, the AI's correctness changes; with every
  player on his own team (the default) the code path is today's, proven on the Kessel Ridge 40 set. **S17b**: AI team habits, the map,
  CLI and tools. **Presenter P** between.
- **Weighed for S18 and not chosen** (owner question S17-9): the late-1944 US wave (mostly data; fits any gap, also a good filler if the
  net work stalls); buildings and a village (M4, the largest item, wants its own thread). Online teams is recommended because S17 alone
  leaves the launch-scope feature playable only against AI.
- **Kept / later.** Kept: teams as a per-player number, shared sight, team capture, team tickets, team victory, AI that picks a foe,
  shares points and helps its ally, a four-base map. Later: shared supply and aid between allies (S17-5), manpower gifts, pings and
  ally chat (S18-5), ordering an AI ally, 3v3 and larger, team sizes that differ, ranked play.

## 1. Scope
**S17a (engine, neutral).** Build A: `team` on a player; `crates/sim/src/teams.rs` with the three questions the code asks (hostile,
allied, sees); the sweep of every enemy test and sight-bit read; rules T1-T9; the `[teams]` keys; the AI's correctness changes A1, A2,
A5; `skirmish` with 3 and 4 players (smoke); SAVE bump; neutral proof.
**S17b (AI, map, tools).** Build B: AI habits A3, A4, A6; `mods/ww2/defs/maps/harrow_cross.toml` (four bases); `skirmish --teams`;
JSON reports for 4 players; `batch.sh` 2v2 set; the 2v2 breakage set.
**Presenter P (Sonnet, after Build A; needs Build B's map for the last step).** Three relations (own, ally, enemy): colours, frame
filter, minimap, top bar, feed and end texts, roster of the ally's squads (read-only); the skirmish menu's mode pick (1v1, 2v2) with
ally and two opponents; the 2v2 mock.
**Not in S17:** anything in `crates/net`, the lobby, hand-over of troops, shared structures, new commands.

## 2. Switches and keys (Build A adds all; stage in brackets turns a habit on)
| Table | Key | Default | Meaning |
|---|---|---|---|
| match request, per player | `team` | none | team number; none = the player's slot (everyone alone: today's game) |
| map `[[bases]]` | `side` | none | which side a base belongs to in a team match (menus and `--teams map` read it) |
| `[teams]` | `shared_sight` | 1 | T2; 0 = each ally sees only his own spots |
| `[teams]` | `team_tickets` | 1 | T6; 0 = drain per player as today |
| `[teams]` | `wiped_waits` | 1 | T7; a wiped player with a standing ally is not out |
| `[teams]` | `pop_pct` | 67 | pop cap of a player on a team of two or more, % of `pop_cap` (`skirmish.toml:9`); S17-2 |
| `[teams]` | `income_pct` | 100 | base and point income of such a player, % |
| `[teams]` | `start_manpower_pct` | 100 | start manpower of such a player, % |
| `[teams]` | `tickets_pct` | 100 | start tickets of such a player, % |
| `[ai.team]` | `share_points` | 0 [B] | A3: ally squads at a point lower my demand there |
| `[ai.team]` | `ally_squad_pct` | 100 | A3: how much one ally squad counts against demand |
| `[ai.team]` | `help_ally` | 0 [B] | A4: a threatened ally point gets a defence priority |
| `[ai.team]` | `help_priority`, `help_demand` | 80, 1 | A4: the values (own threatened point: 80, `ai.rs:2328`) |
| `[ai.team]` | `foe` | `"nearest"` | A1: which hostile player is "the enemy" for works, belts and guns; `"first"` = lowest slot |
The four `_pct` keys apply only to players whose team has two or more members, so a 1v1 never reads them. All validate 10..=300.

## 3. Data model
- `PlayerRequest.team: Option<u8>` (`setup.rs:123-131`, `#[serde(default)]`); `PlayerSetup.team: u8` (`setup.rs:1439-1446`). Resolve
  fills none with the slot index. Validation: at least two teams; team under `MAX_MATCH_PLAYERS`; players at most the map's bases
  (exists, `setup.rs:1466-1481`). "Player i starts at base i" stays (`setup.rs:133-134`).
- `BaseDef.side: Option<u8>`: a hint only; the sim never reads it.
- **Derived, not saved**: `Teams { team: Vec<u8>, allies: Vec<u16> }` in `MatchState`, rebuilt from the setup at start and at load.
  `allies[p]` is the bit mask of p's team, p included; with everyone alone it is `1 << p`, which is exactly the bit the code tests today.
- `teams.rs` API (the only place team logic lives): `hostile(a, b)`, `allied(a, b)`, `sees(p, seen_by) = seen_by & allies[p] != 0`,
  `foes(p)`, `main_foe(p)` (A1), `team_value(p)`, `won_by(p)`.
- **Unchanged**: `seen_by` stays one bit per player (written by each player's own observers), so hashes do not move; a mine's `known`
  mask (`fort.rs:158`, a `u8`: 8 players at most, Build A adds the check); `Outcome::Won { winner }` keeps a `PlayerId`, the lowest
  standing slot of the winning team, and callers ask `won_by`.
- **SAVE +1** over `<BASE>` in one layout commit (the `team` field, `[teams]`, `[ai.team]`). **PROTOCOL stays**: no command and no
  message changes; `build_id` carries SAVE (`crates/net/src/lib.rs:60-67`) and the setup hash (`:71`) covers the new field. A relay
  match still resolves two players, each alone on his team.
- No new entity, no new command, no new RNG stream.

## 4. Rules (integers; every rule reduces to today's when each player is alone on his team)
- **T1 Hostility.** Every "is this an enemy" test asks `hostile(owner_a, owner_b)`: target choice (`combat.rs:312`, `:420`, `:455`,
  `:526`; `gunnery.rs:1443`, `:1524`; `squad.rs:288`; `game.rs:3022`), throws, satchels, mines and wire triggers, capture of vehicles and
  guns (`bail.rs:256`), spotting targets (`spotting.rs:101`, `:191`), demolition, sweep, counter-battery. Every "is this mine" test
  (orders, control, services, pop, cards: `world.rs:2847-2849`, `game.rs:1017-1060`) stays an owner test.
- **T2 Shared sight** (`shared_sight`). Wherever the code reads "player p sees it" (`seen_by & (1 << p)`), it reads `sees(p, ..)`.
  Allies also share mine knowledge (`known`) and counter-battery markers the same way. Each player's observers still run only for
  him, and skip allies as targets, so ray cost grows with men, not with team size.
- **T3 Fire between allies** is fire on own men: direct fire and fragments do not hurt an ally (`combat.rs:2688`, `:2933`), mission
  bursts do under `friendly_fire` (`combat.rs:2617`), and such a death is no kill (`combat.rs:4282`). Nobody aims at an ally.
- **T4 Capture.** Men are counted per team (`rules.rs:138-156`). A point is contested when two hostile teams are present. One team
  present and not holding it: progress grows; at full the point goes to the present team member with the most men there (ties: lowest
  slot). An ally standing on his ally's point changes nothing and wears enemy progress down as the owner does. Points stay owned by
  a player.
- **T5 Income** per player: base income plus his own points (`rules.rs:75-86`), times `income_pct` on a team.
- **T6 Tickets** (`team_tickets`). Drain of player p = (best hostile team's held value - his team's held value) x the rate, applied to
  each member; members start equal, so a team runs out in one tick.
- **T7 Victory.** A team is out when all its members are out; the last team standing wins; none standing is a draw
  (`rules.rs:236-253`). `Wiped` (`rules.rs:227-228`) is not applied while an ally stands (`wiped_waits`): the player keeps his income
  and can buy again. A surrendered player's troops hold where they are until S18 hands them over.
- **T8 Services stay with the owner.** HQs, dumps, crates, aid posts, transports, towing, gun crews, re-crewing and reinforcement serve
  their owner only, as today (S17-5). An ally's abandoned tank or gun cannot be captured. Nobody builds in another player's base
  (`structures.rs:644-647`, `fort.rs:926-936`), ally bases included.
- **T9 Economy keys.** Section 2; read at match start (`start_manpower_pct`, `tickets_pct`) and in the pop checks (`game.rs:1164`; the
  AI's six `pop_cap` reads from `ai.rs:1995` on) through one function `pop_cap_of(player)`.
- **Validation**: a request with one team only is refused; `frontline-cli validate` warns on a map whose `side` values do not give two
  sides of equal size.

## 5. Harrow Cross (`ww2:harrow_cross`, 1536 m, four bases, S17b)
Two roads cross at the centre; a low rise carries the crossroads. Side 0 holds the west edge, side 1 the east. `symmetry = "point"`
(S16a's blended form), every item listed with its mirror, roads from S16a. Seven points.
| Item | Side 0 | Mirror (side 1) |
|---|---|---|
| Bases (r 40) | slot 0 (96, 400); slot 1 (96, 1136) | slot 2 (1440, 1136); slot 3 (1440, 400) |
| Home points | A (360, 400); B (360, 1136) | F (1176, 1136); G (1176, 400) |
| Flank points | C (768, 240) south | E (768, 1296) north |
| Centre | D (768, 768), `value = 2`, on the rise | itself |
| Roads (6 m) | each base to its home point to D; C to D to E | mirrors |
| Woods, valleys | Build B places 4 pairs of woods and one shallow valley pair, every item with its mirror | |
Each base is about 760 m from D (Kessel Ridge 588 m). Build B may change terrain shape; points and bases move only by an owner answer
(S17-7). The map also loads as a 1v1 (slots 0 and 2 would be wanted, but player i takes base i, so 1v1 on it is not offered in S17).

## 6. AI
**S17a (needed so a team match is not wrong):**
- **A1 The foe.** The four "first other player" sites call `main_foe(me)`: the standing hostile player whose base is nearest mine
  (ties: lowest slot). In 1v1 that is the same player as today.
- **A2 Ally points.** `evaluate_points` gets `Rel::Ally` (`ai.rs:2311-2315`): priority 0 and demand 0, so the AI neither attacks nor
  garrisons them; `under_capture` (`ai.rs:2323`) and every `owner != Some(me)` test in `ai/` (13 sites) ask `hostile`.
- **A5 Fire safety.** `ff_safe` and every "own men near" test of the artillery, mortar and throw habits count ally men
  (`ai/arty.rs`, `ai/mortar.rs`, `ai.rs:3224` `throws`); `known_enemies` and `known_vehicles` (`ai.rs:1612`, `:2731`) read `sees`.
**S17b (habits, each behind its switch):**
- **A3 Share points** (`share_points`). At a neutral or enemy point my demand falls by the ally squads within the capture radius times
  `ally_squad_pct`. With the existing distance term (`ai.rs:2324`, each AI prefers points near its own base) this spreads two AIs over
  the map with no shared plan and no saved state.
- **A4 Help the ally** (`help_ally`). An ally point with known enemies near it, or under capture by a hostile team, gets
  `help_priority` and `help_demand`.
- **A6 Works.** Front works, belts and gun corridors stay built against `main_foe` only; the second foe's routes are a report
  (`ai_second_foe_unbelted`), not a rule.
No AI state is added to the save. Buys, cards and pots are per player and unchanged; the pop reads go through `pop_cap_of`.

## 7. Determinism, cost, guard rails
No RNG. `Teams` is a pure function of the setup. Hashes equal at 1, 2, 3, 8 threads and through save/load, in 1v1 and 2v2. Cost: four
players at `pop_pct` 67 put up to 400 men on the map (300 today; SYSTEM_DESIGN's Battalion tier allows 700); spotting, combat and path
budgets (`skirmish.toml:43-44`, shared by all players) scale with men. Tick p99 of the 2v2 set is printed; the one hard line is the
tick period (33.3 ms at 30 Hz) on the `tools/verify.sh` machine, because past it the game cannot run in real time. Tick p99 on Kessel
Ridge 1v1 must not regress (the sweep adds one table read per test).

## 8. Reports (printed, not judged; no targets file rows are hard)
Per 2v2 set: `team0_wins`, draws, capped matches, `length_min`, per-player kills, losses and points held, `ally_help_tasks`,
`shared_point_holds`, `ai_second_foe_unbelted`, own-fire deaths split into own and ally, tick p99, path expansions p99. CLI observer
counters, not saved.

## 9. Done when (breakage checks only; no 480 sets, no ablation measures, no share bands)
**S17a**
- Kessel Ridge 40 set at `<BASE>` and after S17a equal 40/40 (`tools/neutral.py`); the same on Linden Brook if S16b has merged.
- Hashes equal at 1, 2, 3, 8 threads and through save/load; save fuzz (`crates/sim/tests/save_fuzz.rs`) 0 panics with a staged
  four-player, two-team state.
- Named tests: with every player alone, `Teams.allies[p] == 1 << p`; an ally is never a target (rifle, MG, tank gun, AT gun, throw);
  a direct-fire round and a fragment pass an ally, a mission burst hurts him under `friendly_fire`; a man seen by my ally is a
  target for me with `shared_sight` 1 and not with 0; an ally's mine does not go off under me and I know it; two allies on a neutral
  point capture it at the summed rate and the one with more men gets it; an ally on my point does not contest it; team drain equals
  the 1v1 drain for the same point gap; one member wiped with an ally standing is not out, both wiped is a loss; a team wins when
  the other team's members are all out; an ally's abandoned tank cannot be captured; `pop_pct` caps a team player's buys and not a
  lone player's; a one-team request is refused; the AI gives an ally's point no squad and the four foe sites pick the nearest
  hostile base.
- Smoke: a 3-player free-for-all and a 4-player 2v2 on a test map run 45 min of AI without panic, with one outcome, and with equal
  hashes on two thread counts (the first runs ever with a third player).
- `tools/check.sh quick` green; the site table (section 10) has no row left "unclassified".
**S17b**
- The 2v2 set on Harrow Cross: 20 seeds x both sides, AI vs AI, 45-min cap, each of two faction layouts (mixed teams and same-faction
  teams): no panic, every match ends with a team win, a draw or the cap (counts printed), hashes equal across thread counts and
  through one mid-match save/load per seed; save fuzz 0 panics on Harrow Cross.
- In at least one match of the set each AI holds a point its ally does not, and at least one `ally_help_tasks` fires with the
  switches on (the habits run; how well is not judged).
- Tick p99 of the set under 33.3 ms; Kessel Ridge 40 set with the S17b switches on equals S17a's (the habits never fire in 1v1).
- Mirror placement test for Harrow Cross passes; `frontline-cli validate` clean.
**Presenter P**: Godot headless mock-match smoke on a 2v2 mock and a real 2v2 offline match of 2 min: no script error; ally units
drawn in the ally colour and not selectable; the 1v1 smoke unchanged.

## 10. Build stages (a chunk is about 30 self-counted turns)
1. **S17a Build A** (Opus, 2 chunks). Chunk 1: the layout commit (section 3, every key of section 2, SAVE +1), `teams.rs`, and the
   **site table**: `tools/owner_sites.sh` lists every raw owner comparison and sight-bit read in non-test sim code (323 and 55 at
   `cdc404c`), and `docs/slices/S17-sites.md` classes each as Own, Hostile or Sees. Chunk 2: the sweep by that table, T1-T9, A1, A2, A5,
   named tests, the 3- and 4-player smoke, neutral 40.
2. **Checker after A** (Sonnet, fresh agent): the diff against this file and the site table (every Hostile and Sees row changed, no
   Own row changed), named tests, the neutral check; lists misses.
3. **S17b Build B** (Opus, 1-2 chunks): A3, A4, A6; `harrow_cross.toml` and its mirror test; `skirmish --teams 2v2|map`,
   `--faction2`, `--faction3` (`skirmish.rs:77-80`, `:2244-2258`); JSON reports keep today's shape for two players and add players 2
   and 3 (so `neutral.py` sees no change in 1v1); `batch.sh` gets the 2v2 set; the breakage set.
4. **Presenter P** (Sonnet, 1-2 chunks): `palette.gd:41-45` gains ALLY; `match_frame.rs:423`, `:541`, `:563` filter by the team
   mask and mark each unit own, ally or enemy; minimap, top bar (team tickets, both names), feed and end texts
   (`match_events.rs:250`, `:323`), end screen per team; `MenuChoice` (`match_setup.rs:16-37`) gains a mode and a slot list of four;
   the menu's mode pick (`main_menu.gd:153`, `:197`); the 2v2 mock (`mock_match_source.gd`); headless smoke.
5. **Orchestrator**: `tools/verify.sh`; Review (Opus; the gate fires: save layout, rules, the sweep); Fix (Opus); the breakage sets
   again; Docs (Haiku: STATE, TODO with S17-1..9, the modding note for `team` and `side`). Fable consulted where a stage stalls.
Size: about **1.0x S3** (Build A is the large part; the map and the AI habits are small).

## 11. Risks
1. **A missed site.** One enemy test left as an owner test makes allies shoot each other or mines go off under an ally. Answer: the
   site table, the checker's row-by-row pass, and `owner_sites.sh` in `check.sh` so a later slice's new raw comparison shows in review.
2. **A third player has never run.** Expect panics and two-player assumptions outside the grep (per-player arrays, `1 - p` in
   `scenarios.rs`, CLI reports). The smoke test comes in chunk 1's wake, before the sweep is trusted.
3. **Collisions with the slices in flight.** S14 adds enemy tests in `combat.rs`, `items.rs`, `ai/breach.rs` and a new flame file;
   S15 changes `match_setup.rs`, `main_menu.gd`, `multiplayer_panel.gd`, `session.gd`, `cli/targets.rs`, `tools/pool.py` and
   `measure.sh`, and adds rider and plate tests in `combat.rs`; S16 changes `skirmish.rs` (`--map`), map files, `Session.MAPS`, the
   minimap and `neutral.py`. Each takes a SAVE number. Answer: S17a starts from a tree with S14 and S16a merged (S15 if it is ready),
   reruns `owner_sites.sh` after every rebase, and Presenter P starts after S15's front-end stage.
4. **Tick cost at 400 men.** If p99 passes the tick period the lever is `pop_pct` (a key); the owner is asked, nothing is tuned.
5. **AI allies that do not help.** Two AIs may both crowd one point or leave the ally alone. S17 only proves the habits run; how
   well they play is balance work for later, and both habits have keys.
6. **1536 m map.** Longer walks and matches; the cap may end many AI matches. Counted and printed; the lever is map data.
7. **Shared sight raises lethality** (more targets known per shooter). Not measured here by rule; `shared_sight` 0 is the switch.

## 12. Design questions
**Owner questions (open; the work proceeds on (A) unless the owner picks another):**
- S17-1 **The split.** (A) S17 = teams in the sim, AI allies, the map, 2v2 offline; S18 = online teams and co-op. (B) All of 2v2 in
  one slice (about 1.8x S3). (C) Online first with human teams only, AI allies later.
- S17-2 **Troops per player in 2v2.** (A) 100 men each (`pop_pct` 67), 400 on the map. (B) 150 each, 600 on the map. (C) 75 each, 300
  on the map as in 1v1.
- S17-3 **What allies see.** (A) Everything their ally sees, mines and artillery markers included. (B) Only their own spots.
- S17-4 **Tickets.** (A) One count per team: the team with fewer point values bleeds, both members together. (B) Each player his own.
- S17-5 **Sharing between allies.** (A) Not in S17: each player's HQ, dumps, aid posts, trucks and transports serve only him. (B) Allies
  may draw ammunition and use aid posts now (more rules and tests, about one more chunk).
- S17-6 **A player with no troops left.** (A) He stays in while his ally stands, keeps his income and can buy again. (B) He is out, as in
  1v1.
- S17-7 **The team map.** (A) A new 1536 m map, four bases on two edges, seven points, two crossing roads (section 5). (B) Four bases on
  a 1024 m map, tighter and shorter. (C) Two more bases on Linden Brook.
- S17-8 **Who may team up.** (A) Any mix, the same faction twice and the same faction on both sides included (the menu offers it; S15-10
  decides mirrors in 1v1). (B) Historical sides only.
- S17-9 **What S18 is.** (A) Online teams and co-op (four-slot relay, team lobby, AI fill). (B) The late-1944 US wave. (C) Buildings and
  a village.
**Settled on (A)** (minor scope questions, owner rule 2026-11-11):
S17-10 Fire between allies. (A) As on own men: direct fire passes, mission bursts hurt. (B) All fire hurts allies.
S17-11 Who gets a point two allies took. (A) The one with more men on it, ties to the lower slot. (B) The one who arrived first.
S17-12 Name. (A) Harrow Cross. (B) A name tied to a front after S15.
S17-13 Ally colour. (A) A third colour (green) for the ally, own blue, enemy red. (B) Faction colours.
S17-14 The AI's foe. (A) The nearest hostile base. (B) The weakest hostile player.
S17-15 Team sizes. (A) 2v2 only in menus; the sim allows any. (B) 2v1 and 3v3 in menus too.
S17-16 Building in an ally's base. (A) Refused, as in any other player's base. (B) Allowed.
