# S18: online teams and co-op (four-slot relay, team lobby, hand-over) (short plan)

Status: **short plan**, written 2026-11-25 from `claude/project-thread-1d8xx3` at `cdc404c`. Nothing here is built. It is the second
half of 2v2 (`S17-team-play.md` section 0) and is planned in full after S17a is built, because S17 fixes the team model it stands on.
`<BASE>` is the last commit of S17. File:line from `cdc404c`. Owner rules as S17: features first, no balance runs, every number a key,
breakage checks only.

## 0. Why, and what it costs
- After S17 a player can have an ally only against AI and only offline. SYSTEM_DESIGN 2.5's launch scope is online PvP in teams and
  co-op vs AI "in the same lobbies and relays". S18 is that: up to four humans, AI in the empty slots.
- **The net layer is two slots by type**, not by a constant: `SLOTS = 2` (`protocol.rs:56`), `LobbyState.slots: [Option<SlotInfo>; 2]`
  (`:293`), `Start.factions` and `names: [String; 2]` (`:322-323`), desync `hashes: [u64; 2]` (`:263`, `client.rs:126`, `dump.rs:47`),
  `PeerStat` pairs (`client.rs:154`, `protocol.rs:392`), the relay's slots (`relay.rs:263`, `:294`), `try_start` on slots 0 and 1 with
  "the two factions must differ" (`relay.rs:884-893`), the harness's 20 pair arrays (`harness.rs:778-821`), and the bridge
  (`match_setup.rs:33` `slots: Option<[SlotChoice; 2]>`, `match_net.rs:92`, `net_session.rs:609`, `:622`, `:804`). A net match has no AI
  player today (`Start` carries no AI flag).
- Size: about **0.8x S3**; most of it is the relay and harness, and the lobby screen.
- Split per the owner rule: **S18a** wire and relay for N slots, proven neutral on 1v1 net tests; **S18b** lobby, AI fill, hand-over;
  **Presenter P** the lobby screen and net HUD.

## 1. Scope
**S18a (net engine).** Slot arrays become lists of `Start.players` length (2 to 4; cap key below); `Start` carries per slot: name,
faction, team, base, AI flag and difficulty; turn closing, pause, stall and surrender timers per slot for any count; desync report
with one hash per human; reconnect for any slot; `frontline-net-test` and the harness with 2, 3 and 4 clients. PROTOCOL +1.
**S18b (lobby and rules).** Lobby: host picks map and mode (1v1, 2v2, co-op 2 vs AI); players pick slot, team and faction; empty slots
may be AI (the sim runs every AI on every client, as offline); start when every human is ready and the setup hashes agree. Hand-over
(H1). Co-op vs AI: two humans on one team, two AI on the other.
**Presenter P (Sonnet).** `multiplayer_panel.gd` lobby with four slot rows and team columns; `mock_net.gd` for four; net overlay and
disconnect screen per player (`net_overlay.gd`, `net_disconnect_screen.gd`); "waiting for" texts for any slot; ally ping (S18-5).
**Not in S18:** more than four slots, a server browser, join in progress, spectators, ranked play.

## 2. Switches and keys
| Where | Key | Default | Meaning |
|---|---|---|---|
| relay config | `max_slots` | 4 | slots a lobby may open (2 keeps today's relay) |
| relay config | `allow_ai` | 1 | AI in empty slots (0: humans only) |
| relay config | `allow_same_faction` | 1 | lifts the "must differ" rule in team modes (1v1 follows S15-10) |
| `[teams]` | `handover` | 1 | H1; 0 = a dropped player's troops hold in place |
| `[teams]` | `handover_pop` | 0 | 1 = handed-over troops count against the ally's pop cap for new buys only; 0 = never |
| `[teams]` | `ping_gap_s` | 2 | least time between two pings of one player |

## 3. Rules
- **H1 Hand-over** (sim rule, so it also works offline). When a player surrenders or the relay surrenders him after
  the pause timeout (`relay.rs:72-73` `timeout_us`), his living squads, vehicles, guns, structures, crates, mines and works change owner to his
  lowest-slot standing ally in one tick, in id order; his manpower is added to the ally's; his cards and cooldowns are dropped. With no
  standing ally the team is out (S17 T7). Owner change already exists for captured tanks and guns (`bail.rs`, `guns.rs`).
- **H2 AI fill.** An AI slot is resolved as `is_ai` in the same setup on every client, so its commands are sim output and never
  cross the wire; the relay only counts human slots for turns, pauses and readiness.
- **H3 Start check.** At least two teams; each team's slots filled (human or AI); every human ready with the same setup hash.
- **H4 Pause and stall** are per human slot as today; any human may pause within the existing rate limit (`relay.rs:777-779` `pause_req_ok`).
- **H5 Ping** (`Ping { at }`, a relay message shown only to allies; not a sim command, so no hash effect).

## 4. Determinism and checks (breakage only)
**S18a done when:** the 1v1 net tests and `frontline-net-test` long run pass unchanged in behaviour (same turn counts, no desync);
harness runs with 3 and 4 clients under the existing loss and jitter scripts end with equal hashes on every client; a forced desync
writes a dump naming every slot; reconnect of slot 2 and of slot 3 mid-match; a v16 client is rejected with the version text; Kessel
Ridge 40 set unchanged (no sim change in S18a).
**S18b done when:** a 2v2 with four harness clients, a co-op match (2 clients, 2 AI) and a 1 human + 3 AI match each run 20 min on
Harrow Cross with equal hashes; H1 named tests (every entity kind changes owner, a save in the same tick loads, save fuzz 0 panics,
hashes equal at 1, 2, 3, 8 threads); a client dropped mid-match hands over and the match goes on; Kessel Ridge 40 set equals S17's
(H1 never fires without a surrender); SAVE +1 if H1 adds saved fields.
**Presenter P done when:** headless lobby smoke with the four-slot mock; a real two-client co-op match of 2 min through a local relay
(`autoplay_net.gd`); no script error.

## 5. Build stages
1. **S18a Build A** (Opus, 2 chunks): protocol and relay lists, client, dump, harness for N; PROTOCOL +1; net tests.
2. **Checker** (Sonnet): diff against this file, `sdk/docs/net/relay-protocol.md` updated, the 1v1 net tests.
3. **S18b Build B** (Opus, 1-2 chunks): lobby state and start check, AI fill, H1, H5, bridge `net_session.rs` and `match_setup.rs`.
4. **Presenter P** (Sonnet, 1-2 chunks): section 1 list.
5. Orchestrator: `tools/verify.sh`, the net long run; Review (Opus; gate fires: protocol, relay, save); Fix (Opus); Docs (Haiku:
   STATE, TODO, `docs/net/playing-online.md`, relay protocol doc).

## 6. Risks
1. **Lockstep with four humans**: one slow client stalls three. The existing D and stall rules apply per slot; only counted here.
2. **Hand-over touches every owned thing**; a kind left behind keeps a dead owner. One table of owned kinds, one test per kind.
3. **Turn size**: `MAX_CLOSED_TURN_BYTES` 1100 (`relay.rs:38`) was sized for two players' quotas (`protocol.rs:76`); four may need a
   lower per-player quota or a second datagram. Decided in Build A from measured turn sizes.
4. **Collision with S15** (lobby opponent picker, mirror rule at `relay.rs:890-893`) and with S17 Presenter P (`match_setup.rs`).

## 7. Owner questions (the work proceeds on (A))
- S18-1 **A dropped or surrendered ally's troops.** (A) They go to his ally, with his manpower. (B) They hold in place and fight on
  without orders. (C) They are removed.
- S18-2 **AI in online lobbies.** (A) Yes, any empty slot may be AI (this is co-op vs AI). (B) Humans only.
- S18-3 **Lobby size.** (A) Up to four slots (2v2, co-op 2 vs 2 AI, 1v1). (B) Up to six (3v3), with a larger map later.
- S18-4 **Who may pause.** (A) Any player, with today's limit. (B) The host only.
- S18-5 **Talking to the ally.** (A) A map ping only. (B) Ping and text chat. (C) Nothing in S18.
- S18-6 (settled (A)) Handed-over troops and the pop cap. (A) They never block the ally's cap. (B) They count for new buys.
