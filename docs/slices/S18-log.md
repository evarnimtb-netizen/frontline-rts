# S18 log (online teams and co-op; at most 150 lines)
Spec: `docs/slices/S18-online-teams.md`. Branch `s18-build` (local, not pushed), base `f58bcbd` (S17 log; SAVE 34,
PROTOCOL 17). One build stage covering S18a, S18b and Presenter P. Commit only your files, never `git add -A`, never
`target/`, `.claude/`.

## For the next stage (rewritten by every stage; at most 30 lines)
**State: S18 built (net, sim, bridge, presenter), self-reviewed.** Next: Review (gate fires: protocol, relay, save), then
Docs (`docs/net/playing-online.md`), then the owner's merge.
**Versions:** SAVE **35**, PROTOCOL **18**.
**Kessel Ridge is neutral:** 40 set **same 40/40** against `f58bcbd` (`target/s18/base` vs `target/s18/new`, world
`954ec4217cdfa0383a0fb96d` vs `ca909935f62dac05bbc7079a`; H1 never fires without a surrender).
**Owner questions** S18-1..5 run on (A) ("(A) assumed" in `docs/TODO.md`); S18-6 was settled (A).

## What was built
- **Protocol 18** (`crates/net/src/protocol.rs`): `MAX_SLOTS` 4 replaces `SLOTS`; `LobbySet` gains `team`, and from the
  host `players` and `ai: Vec<AiPick { slot, faction, team, difficulty }>`; `Ready { setup_hash, rev }`; `Up::TakeSlot`,
  `Up::MapPing { x, y }`, `Down::Slot`, `Down::MapPing { slot, x, y }`; `LobbyState { slots: Vec, players, rev, .. }`,
  `SlotInfo.team`, `.ai`; `Start.players: Vec<StartPlayer { name, faction, team, ai }>` replaces `factions`/`names`;
  `Desync.hashes: Vec<Option<u64>>`; `Bundle.peers: Vec`. Limits checked on decode (`s18_limits_hold`).
  `MAX_TURN_CMDS` = 4 x 17. `sdk/docs/net/relay-protocol.md` updated.
- **Relay** (`relay.rs`): `RelayConfig` `max_slots` 4, `allow_ai` 1, `allow_same_faction` 1, `ping_gap_us` 500 ms
  (`frontline-relay --max-slots --no-ai --distinct-factions --ping-gap-ms`). The lobby holds `players` slots (2 until the
  host opens more, so a 1v1 relay still refuses a third: `Full`). A join takes the lowest open non-AI slot. H3 start check:
  every slot filled (player or AI), every player ready with the same hash for the current revision, factions picked (in a
  two-slot match, or with `--distinct-factions`, hostile factions differ), at least two teams. Turns close round-robin
  over the match's slots (`tick mod players`: a 1v1 alternates exactly as before). Hashes: any two connected players
  differing is a desync at once; all in and equal confirms; one player left compares with nobody. Pings go to connected
  same-team players, one per gap. Name clashes get " (2)", " (3)", ...
- **Client** (`client.rs`, `client_io.rs`, `dump.rs`): `ready()` sends the last lobby revision heard; `take_slot`,
  `map_ping`; events `Slot`, `MapPing`; `DumpManifest.hashes: Vec<Option<String>>`.
- **H1 hand-over** (`crates/sim/src/handover.rs`, `game.rs` Surrender): with `[teams] handover` 1 a surrendering player's
  living soldiers, vehicles, guns, abandoned vehicles, downed men, squads, HQs, dumps, aid posts, crates, works, pieces,
  wire, charges, mines (the ally learns them) and unit dig jobs go to his lowest-slot standing ally in one tick; manpower
  moves too. Corpses, wrecks, items and rounds in flight, cook-offs, captured points keep their owner. Once only (a second
  Surrender, e.g. his own then the relay's as he quits, moves nothing). `PlayerState.handed` (squads received) and
  `handed_to` (saved, hashed; SAVE 35). `[teams] handover_pop` 0: handed squads never count against the ally's pop cap
  (S18-6). Load accepts a structure of the giver's faction owned along the `handed_to` chain. `[teams] ping_gap_s` 2 is
  the presenter's ping gate. A 1v1 never hands over.
- **Harness** (`harness.rs`): `run_virtual` plays N clients (`VirtualSpec.team: Some(ai slots)`): the host joins first,
  sets size, teams and AI fill, then the others join; per-client report fields are Vecs in slot order; hashes compared
  across every pair of clients; `map_pings` schedule; `net_setup_teams`.
- **Bridge**: `MenuChoice.slots: Option<Vec<SlotChoice { faction, human, team, difficulty }>>` (2 to 4; the S17 O4
  refusal of a 2v2 with two net slots kept); `NetSession` builds the lobby's slots for `Ready` and `Start`'s for the match;
  `FrontlineNet.set_choice` takes `team`, `players`, `ai`; `take_slot`; `status()` slots carry `team`, `ai`, plus
  `players`. `FrontlineMatch`: `map_ping` intent (gated at `ping_gap_s`), `net.pings` in the state; names per slot.
- **Presenter**: lobby mode pick (1v1, 2v2 on Harrow Cross), four rows with team, AI rows, Add AI / Remove AI (host),
  Move here (joiner), own team buttons, Ready blockers (open slots, one team, map). "." pings the ground under the cursor;
  pings show in the event feed (Space jumps there). MockNet plays four slots. `godot/docs/net_api.md`, `match_state.md`.

## Checks (all breakage-only; no balance runs)
- `cargo test --release` net (skip `ipv6_loopback_handshake`), sim, content, cli, godot-bridge: all pass. Content: the
  setup BASELINE and the canonical snapshot re-blessed (three new `[teams]` keys); `TeamsTuning::KEYS` 10.
- Team loopback (`crates/net/tests/teams.rs`, virtual clock, Harrow Cross, tickets 5000): 2 people + 2 AI fill, 300 s:
  7190 ticks, 149 hash checks, 0 mismatches; slot 2 blacked out 75 s, timed out, its troops to the AI in slot 3. Four
  people, 240 s: 5379 ticks on three clients, 714 pairwise checks, 0 mismatches; slot 3 back after a 10 s blackout
  (waiting pause, resume), slot 1 dropped for good and handed to slot 0; slot 0's ping reached slot 1 only. A forced
  desync in a four-player match: all four stop and dump, each manifest lists four hashes.
- Relay rules (`relay_rules.rs`): lobby size, AI fill and slot moves; a stale Ready dropped; pings to allies only, spaced.
- Sim (`s18_tests.rs`): every kind handed over, save/load same hash, control `handover` 0; handed squads free the cap;
  no standing ally; a repeated Surrender; a 2v2 of AIs with two surrenders equal at 1, 2, 3, 8 threads and across a
  reload. Save fuzz (`--include-ignored`, mirror version 35, a team world with a hand-over): 17 passed, 0 panics.
- Kessel Ridge 1v1 neutral 40 set: same 40/40 vs `f58bcbd`.
- Clippy `-D warnings`, fmt, `tools/owner_sites.sh --check` (15 new sites classed in `S17-sites.md`).
- Godot 4.7.2 headless: `test_net_ui` 164/164, `test_match_view` 449/449, `test_mock_maps` 25/25, `test_data_reload`
  21/21, `check_project` OK.

## Deviations from the plan
- One stage instead of S18a/checker/S18b/Presenter P.
- `handover_pop` 1 counts handed squads fully (the plan says "for new buys only"); the default 0 is S18-6 (A).
- The plan's 20-minute team runs and a 1 human + 3 AI run are not in the suite; the team tests run 4-5 game minutes
  (thousands of ticks). `frontline-net-test` (UDP binary) and `run_udp` stay two-client.
- Ready carries the lobby revision: a Ready made before the last lobby change is dropped (the player presses Ready again).
  `lobby_flood_does_not_jam_the_other_stream` now readies once the cut client caught up with the lobby.
- The host is still slot 0 and the host leaving still closes the in-game relay for everyone (as in 1v1).
- Pings are a control message (`Up/Down::MapPing`), not `Ping { at }`, to keep the clock `Ping` packet.
- Turn size: `MAX_CLOSED_TURN_BYTES` stays 1100; with four busy players the excess rolls to the next turn (restamps).

## Unfinished
- No per-player net overlay redesign: the existing overlay and disconnect screen name any slot through the per-slot names.
- No real two-client co-op run through `autoplay_net.gd` (headless Godot here has no second instance driver).
- `docs/net/playing-online.md` (Docs stage).
