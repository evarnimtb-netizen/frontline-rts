# Slice S2: two-player multiplayer (relay-closed lockstep)

**Owner context (2026-10-31):** playable-first. S1 is done, S1b is
finishing. S2 puts a 1v1 match into a friend's hands: on a LAN, by direct
IP, over Tailscale, or through a `frontline-relay` on a cheap VPS (§15 M1).
*(proposed)* marks Claude's defaults, **not yet confirmed by the owner**.
Work proceeds on the defaults until the owner answers the questions at the
end. Read `SYSTEM_DESIGN.md` §2.2 "Lockstep with mixed hardware", §5.11,
§6, §10. S1/S1b rules apply; this file wins where it differs.

Revision 2 folds in the netcode and scope reviews.

## Hard rules

- **No sim changes.** A match is a pure function of `MatchSetup` and the
  bundle stream fed to `World::step`. The AI runs in the sim on both
  clients, so it costs no bandwidth.
- **Sans-IO.** `Relay` and `NetClient` expose `on_datagram(from, bytes,
  now)` and `poll(now)`, so tests are deterministic on a virtual clock.
- **The relay is the clock.** No client waits on another's CPU, and no
  network I/O runs on a thread that can stall: the sim thread and the
  Godot main thread never touch a socket (§2.2, §6).
- **Input is never lost.** Once a client has stamped a command, the relay
  eventually puts it in a bundle; it may restamp it later, never drop it.
- **All network input is untrusted.**
- **Documented protocol:** `sdk/docs/net/relay-protocol.md` (§17 Q15).
  Every wire change bumps `PROTOCOL_VERSION`.

## Scope

**In:**
- 1v1 human vs human. Host or join by IP:port. The relay runs on its own
  thread inside the host's game, or as the `frontline-relay` binary.
- Lobby, countdown, relay-closed turns, pause, hash checks, desync dumps,
  `desync-diff` (field diff), resync after a short drop.
- **A build the friend can run:** an unsigned macOS export zip.
- **Cross-network play without new code:** a `playing-online.md` guide for
  port forwarding, Tailscale, and a VPS-hosted `frontline-relay`.

**Out:**
- Steam lobbies, NAT punch-through, UPnP, server browser, encryption.
- A Windows build of the game (and the cross-platform determinism CI it
  needs, §5.11). `frontline-relay` still builds on Linux, macOS and Windows.
- More than 2 players, the governor (`SetSpeed`, `SetController`), speeds
  other than 1×, mid-match changes to D, mod download, join-in-progress,
  rejoin after a crash.
- `desync-diff --replay`. The dump keeps everything it needs, so it is a
  cheap follow-up.
- `tick_salt` in the RNG (the wire carries it, set to 0).
- **2 humans + 2 AI is not cheap:** no teams in `MatchState`, victory is
  last-player-standing, Kessel Ridge has 2 bases. With a teams slice, AI
  slots will cost no bandwidth.

## Transport *(proposed)*: plain UDP

S2 uses a non-blocking UDP socket (`socket2`, bound dual-stack `[::]` with
`only_v6(false)`, so IPv4 and IPv6 both work). Reliability comes from
acks and resends, below. **Not TCP:** one loss stalls every later bundle
for an RTO (≥ 200 ms), longer than D, which is the stall relay-closed
turns exist to avoid. **Not `quinn` yet:** it pulls tokio and TLS into the
GDExtension for no gain at 2 players. A `Transport` trait keeps a later
move to QUIC datagrams (M7) local to one layer.

Control messages (lobby, pause, `Bye`) are re-sent every 100 ms until
acked. **Default port 47015** *(proposed)*, configurable everywhere.
27015 is rejected: it is the Source dedicated-server port, inside Steam's
27000–27100 range.

## Threads

- **Client:** a dedicated `frontline-net` thread owns the socket, the
  clock sync, input sampling and acks. It talks to the sim thread over
  channels: `NetEvent` in (`Bundle`, `Paused`, `Resume`, `Desync`, `Bye`),
  and hashes out. Godot's `send()` puts `CommandKind`s on the net
  thread's queue directly.
- **Relay:** its own thread, both in the host's game and in the binary.
  `FrontlineNet` on the Godot main thread only reads status snapshots, so
  a host frame hitch stalls nobody.
- **Deadlines:** the relay sleeps until 1 ms before a turn deadline, then
  spins. On Windows the binary also calls `timeBeginPeriod(1)` (the
  default timer is 15.6 ms).
- **End of match:** the net thread keeps running until `Bye`, even after
  `run_loop` sees victory and stops reading input.

## Protocol

The header is `b"FLNT"`, `PROTOCOL_VERSION: u16`, `kind: u8`, then
`token: u64` (random, assigned by the relay). The body is postcard, and a
datagram is at most 1,200 bytes.

- `Hello` (build_id, content hash, name; padded to 512 B) → `Welcome`
  (token, slot) or `Reject` (reason). No pre-auth reply is larger than
  the request, so the relay cannot be used for amplification.
- `LobbySet` (faction; slot 0 also seed, map, D), `Ready` (setup hash).
- `Start`: start time on the relay clock, `tick_hz`, D, seed, slots.
- `Ping`/`Pong`: timestamps, every 1 s for the whole match.
- `Input`: bundle ack (`complete: u32` = highest tick with every bundle
  held, `mask: u64` for the 64 ticks after it), then **every unacked turn
  of own input**, oldest first, as `(target_tick, seq0, Vec<CommandKind>)`,
  up to the datagram limit. Own input counts as acked once its `seq`
  appears in a bundle.
- `Bundle`: unacked `(tick, tick_salt, Vec<Command + target_tick>)`,
  **oldest first**. Each command keeps the `target_tick` it was stamped
  with, so restamps are visible to everyone (§10).
- `Hash (tick, hash)`, `PauseReq`/`Paused{P}`/`Resume{P, close_at}`,
  `Desync`, `Bye(reason)`.

## Turns, input delay, pause and speed

- **One turn is one tick** (33.3 ms). Bundle T holds the commands for
  `world.step` while `world.tick() == T`.
- **Clock sync:** each `Pong` gives an offset sample. The client uses the
  median of the last 8 and slews toward it by at most 1 ms per second
  (it steps only if more than 50 ms off), so 20–50 ppm crystal drift never
  adds up over a match.
- **Stamping:** once per period the net thread takes queued
  `CommandKind`s, with `target_tick = current relay-clock tick + D` (from
  the synced clock, not from the newest bundle) and the next per-player
  `seq`.
- **D *(proposed; revised in the fix pass, owner call)***: the lobby
  computes `clamp(ceil((p99_rtt/2 + jitter) / period), 2, 4)` from lobby
  pings, and each client steps a tick `P = clamp(ceil((p99_rtt/2 +
  jitter) / period), 1, 6)` periods after its turn closes (playout). D
  only has to cover the trip to the relay; the trip back is the playout.
  The first formula (`ceil((p99_rtt + jitter) / period) + 1` with a
  fixed one-tick playout) put both trips into D and still stalled
  whenever the trip back took over a period. Click to reaction stays
  about the same (D + P ≈ the old D + 1); stalls on jittery links went
  from 16–67 % of ticks to under 0.2 %. See the slice log. Slot 0 may override it (2–6) before Ready. D is fixed for the
  match. On a LAN, D = 2; on a 60 ms internet path, D = 4.
- **Relay-closed turns:** the relay closes turn T at `start + (T+1)·period`
  regardless of what arrived. Input whose `target_tick` is already closed
  goes, in order, to the next open turn (a restamp). The relay overwrites
  `Command.player` with the connection's slot, accepts only the next `seq`
  per player (later ones wait for the client's resend), and sorts by
  `(player, seq)`.
- **Bundle delivery:** the relay sends the oldest unacked bundle first.
  After a gap it sends a paced catch-up burst (at most 8 datagrams per
  period), so a 10 s drop (≈ 300 bundles) refills in order.
- **Client pacing:** the sim thread steps T at its scheduled slot
  `start + (T+1)·period + one tick of playout`, mapped to local time, once
  bundle T is in. It never steps past the newest bundle, and catches up at
  ≤ 3 ticks per period. Frames carry the scheduled slot, not the arrival
  time, as their timestamp, so presenter interpolation does not jitter.
- **Order feedback:** the presenter draws a pending-order marker on click.
  At D = 4 the unit reacts about 130–200 ms later.
- **Pause:** on either player's `PauseReq` the relay broadcasts
  `Paused{P = last closed + 1}` and stops closing turns. Either player can
  resume. `Resume{P, close_at}` gives the relay time at which turn P
  closes (≥ 300 ms ahead), and both clients rebase their slot schedule on
  it. No 3-2-1 countdown.
- **Speed** is locked at 1×. The relay's `--speed` flag is for tests only,
  and net mode ignores `--autoplay_speed`.

## Session setup

1. **`Hello`** carries the `build_id` (git commit, a `-dirty` flag,
   `PROTOCOL_VERSION` and `SAVE_VERSION`, from `crates/net/build.rs`) and
   a **mod content hash**: xxh3 of postcard(`ModDefs`), via the new
   `ModDefs::content_hash()` in `sdk/crates/content/src/modset.rs`.
   `ModDefs` (line 48) needs `Serialize` added to its derive; every def
   type already has it. Mismatch: `Reject` (dev: `--allow-build-mismatch`).
2. **Lobby.** Slot 0 is whoever connects first. With the in-game relay
   that is the host; with a standalone relay nothing assumes the relay
   machine plays. Slot 0 sets the seed, map and D. Each player picks a
   faction, and the two must differ. In `match_setup.rs`, `MenuChoice`
   gains `slots: [SlotChoice; 2]`, and `resolve_with` makes both slots
   human.
3. **`Ready`** sends `xxh3(postcard(MatchSetup))` from each client's own
   resolve; the relay starts only if they match. `MatchSetup` is the sim's
   only content, so this is the determinism guard.
4. **Countdown.** Clients map `Start`'s relay time through the synced
   clock, then show 3-2-1.

## Hash check and desync dump

- **Exchange:** the client sends each `Some(hash)` that `World::step`
  returns (every `hash_interval` = 30 ticks). The sim thread also keeps a
  ring of `state_hash()` for the last 300 ticks; it reads the incremental
  cache, so it is cheap, and needs no sim change.
- **Detection:** on a mismatch the relay sends `Desync` and both clients
  pause at the same P, at most 30 ticks plus one RTT after divergence.
- **Checkpoint:** every 300 ticks each client takes a `world.save()`. It
  keeps the newest checkpoint whose tick the relay has **confirmed** (both
  hashes agreed), plus every bundle since. An unconfirmed newer one may
  already be past the divergence, so it never replaces the confirmed one.
- **Dump** to `user://desync/<match_id>/slot<N>/` (macOS:
  `~/Library/Application Support/Godot/app_userdata/Frontline/desync/`):
  - `manifest.json`: build_id, mod content hash, setup hash, seed, D,
    desync tick, both hashes at the desync tick, `state_hash` and
    `full_state_hash` at P, the per-tick hash ring;
  - `checkpoint.sav` (confirmed), `paused.sav` (tick P on both clients);
  - `bundles.jsonl`: every bundle since the checkpoint, with `tick_salt`.
  - The relay writes `relay.jsonl` (its bundles and both hash streams).
  - No RNG state is needed: `rng.rs` keeps none, and per-entity counters
    are already in `SimState`.
- **`frontline-cli desync-diff A.sav B.sav`**
  (`sdk/crates/cli/src/desync_diff.rs`; it uses only `World::load`, which
  the CLI already links at M0):
  - compares globals, players, squads, points and entity fields by id, and
    prints the first 50 differences, e.g. `entity 412 pos.x: 1033.512 vs
    1033.508`. Exits 1 on any;
  - if the saves match but the hashes differ, says so: `World::load` runs
    `rebuild_derived`, so a divergence only in derived data or the hash
    cache cannot show in a save diff. The per-tick ring narrows the tick,
    and the later `--replay` finds the field.

## Disconnect, timeout and reconnect

- **Identity is the token, not the address.** A datagram with a valid
  token from a new address moves the connection there (NAT rebinding,
  Wi-Fi to Ethernet). Bad tokens are dropped.
- **2 s of silence:** "unstable"; turns keep closing without that
  player's orders. **10 s:** the relay pauses ("waiting for <name>", 60 s
  timer).
- **Resync** (client still running): the client keeps sending unacked
  input and its ack; the relay re-sends missed bundles oldest first; the
  client fast-forwards at ≤ 3 ticks per period. Then `Resume{P, close_at}`.
- **Timeout:** the relay closes turn P carrying a `Surrender` for that slot,
  using the slot's next `seq`, then sends `Resume`. No sim change. A late
  reconnect from that player gets `Bye(TimedOut)`.

## Code changes

- **New `crates/net`** (closed, like the sim): `protocol.rs`, `relay.rs`,
  `client.rs`, `clock.rs`, `transport.rs` (`UdpTransport`, and
  `SimTransport` with seeded delay, jitter, drop, reorder, duplicate and
  blackout), `bin/frontline-relay.rs` (`--port`, `--max-delay`, `--speed`,
  `--log`), and `bin/frontline-net-test.rs` (headless 2-client run).
- **`match_thread.rs`:**
  - `run_loop` pulls from a `TickSource`: `Local` is today's path (`seq`,
    `cmd_rx`, pause, speed); `Net(NetLink)` is new, a pair of channels to
    the net thread plus the shared slot schedule.
  - Add `MatchHandle::spawn_net(world, slot, link)`. In net mode `send()`
    feeds the net thread and `set_paused()` sends `PauseReq`.
  - `counters.log` holds all players' commands, keyed by pre-step tick
    (today `tick() + 1`). Safe: only a test reads it.
  - `MatchCounters` gains `rtt_ms`, `input_delay`, `restamps`,
    `stall_ticks`, `net_state`, `desync_tick`.
- **`match_node.rs`:** `try_start` takes `local` from the session (not
  `0u8`, line 419); `get_state` adds `net`; a new `FrontlineNet` node
  starts the lobby (`host`, `join`, `set_choice`, `ready`, `status`) and
  hands its link to `start_match`.
- **`session.gd`:** `--autoplay_net host:PORT` and `--autoplay_net
  join:IP:PORT` (underscores, like `--autoplay_speed`). Both instances
  also pass `--autoplay N`. Net mode forces speed 1×.
- **Debug hook** (dev builds): `--debug_inject_desync TICK` makes that
  client step tick TICK with one extra local-only `Stop` for one of its
  own units. It diverges without any sim change, and works from Godot as
  well as from `cargo test`.
- **Export:** `godot/export_presets.cfg` (macOS) and
  `tools/export_macos.sh`, which builds the bridge in release (arm64 and
  x86_64, merged with `lipo`), stamps `build_id`, exports and zips
  `Frontline-<commit>-macos.zip`.

## Presenter

- **Main menu → Multiplayer:** Host (port, default 47015) or Join
  (address:port, IPv4 or IPv6).
- **Lobby:** slots with faction and ping, slot 0's seed, map and D (with
  the computed suggestion), Ready, mismatch reason, countdown.
- **In match:** pending-order markers; net readout (ping, D in ms and
  ticks, restamps, stalls); status dot; red desync banner with tick and
  dump path; "waiting for player" overlay; who paused.
- **Disconnect screen:** the reason, then back to the menu.

## Documentation

- `sdk/docs/net/relay-protocol.md`: every message, limit, ack rule and
  version rule.
- `docs/net/playing-online.md`:
  - **LAN:** host, then join by the shown address.
  - **Direct IP:** the host forwards UDP 47015 on the router. Hosts behind
    carrier-grade NAT (common on mobile and some ISPs) cannot host.
  - **Tailscale (recommended for remote play):** both install it; the
    joiner uses the host's 100.x address. Works through almost any NAT.
  - **VPS relay:** run `frontline-relay --port 47015` on a ~$5/month VPS;
    both players join it, so any NAT works.
  - macOS shows a one-time "accept incoming connections" prompt.
- Friend instructions in the zip: unzip, run `xattr -dr
  com.apple.quarantine Frontline.app`, open. Both players need the same
  zip, because `build_id` must match.

## Security

- Length-check before decoding (≤ 1,200 B). Bounded decode:
  `MAX_DIG_POINTS` points and `MAX_UNITS` ids per command; ≤ 16 commands
  and 1,024 B per player per turn (the client rolls excess over).
- Each player's pending queue at the relay is capped at 256 commands;
  beyond that the relay sends `Bye(Flood)`. This is the only way input is
  refused, and an honest client cannot reach it.
- Random 64-bit tokens; pre-auth replies no larger than the request;
  per-address rate limit before auth; no relay-only messages from clients;
  nothing received is executed. Without encryption a token can be sniffed
  on-path; accepted for S2 (Tailscale encrypts anyway).
- The relay overwrites `Command.player`, and the sim already rejects
  orders for foreign units (`MatchState::apply`, `game.rs:421`).

## Performance (Battalion tier, estimates)

Uplink ≈ 2 KB/s per client (30 inputs/s, ~3 unacked turns each). Downlink
≈ 3 KB/s per client with 2 players, ≈ 7 KB/s with 10 (M7), when a host
relay uploads ≈ 70 KB/s. Independent of unit count. Relay CPU under 0.1 ms
per turn. Catch-up bursts briefly reach ≈ 300 KB/s.

## Testing

- **`crates/net/tests/loopback.rs`:** two `World`s and an in-process
  `Relay` over `SimTransport` on a virtual clock, with scripted commands,
  0–80 ms jitter, 5 % drop, reordering and duplicates. Asserts identical
  hashes, no lost commands (every stamped `seq` executes exactly once),
  and counted restamps. The default run is 3 game minutes per seed.
- **Long soak** (`#[ignore]`, run with `cargo test -- --ignored` or
  nightly): 20 game minutes (36,000 ticks), seeds 1–3, with a setup whose
  tickets are high enough that the match lasts the full 20 minutes.
- **Burst-loss tests:** a 1 s total drop and a 10 s blackout both resync
  with no lost command; a 70 s blackout ends in `Surrender`; a source
  address change mid-match keeps the connection.
- **Desync test:** `--debug_inject_desync 1000` on client B; detected
  within 30 ticks, dumped, named by `desync-diff`.
- **Headless:** `frontline-net-test --seconds 1200 --jitter 40 --drop 0.05`
  and `--jitter 2` (LAN-like).
- **Two Godot instances on one Mac:** `--autoplay_net host:47015 --autoplay
  3` and `--autoplay_net join:127.0.0.1:47015 --autoplay 3`. App Nap is
  disabled first (`defaults write org.godotengine.godot NSAppSleepDisabled
  -bool YES`, or the exported app's bundle id), or the background
  instance gets throttled.

## Done when

Agent-checkable:

1. `cargo test --workspace --release`, clippy and fmt pass, including the
   3-minute loopback for seeds 1–3, the desync test and the burst-loss
   tests. The ignored 36,000-tick soak passes once and the result is
   noted in the slice log.
2. `frontline-relay`, `frontline-net-test` and `desync-diff` build and
   have `--help`. `frontline-relay` cross-checks on Linux.
3. `relay-protocol.md` and `playing-online.md` exist and cover every
   message, limit, version rule and connection option.
4. Two instances on one Mac run 3 minutes of autoplay with hashes agreeing
   at every check; screenshots (`docs/slices/screens/s2-*`) of lobby, net
   readout, pending-order marker, desync banner (via
   `--debug_inject_desync`) and disconnect screen.
5. `frontline-net-test --jitter 2` picks D = 2; `--jitter 40` keeps
   restamps under 0.1 % of commands; sim tick P99 is unchanged from S1.
6. `tools/export_macos.sh` produces a zip that runs on a second user
   account of the same Mac after the `xattr` step.

Owner checklist (needs people or a second network; not blocking the
agent):

7. One manual match, two humans with mouse and keyboard, played to
   victory.
8. One match over Tailscale or the VPS relay, with RTT, D and restamp
   count recorded. Wired if possible; on Wi-Fi a few restamps are expected.
9. The friend runs the exported zip on their own Mac.

## Design-doc changes to propose (not applied here)

- §2.2 and §10: size the **total** click-to-step delay from P99 RTT plus
  jitter, split in two: D (stamp to turn close) covers half the P99 RTT
  plus jitter, and a client-side playout delay of the same size covers
  the trip back. (Revision 2 put the whole RTT into D with a one-tick
  playout; the fix pass measured that it stalls on jittery links.)
- §10: each bundled command carries its original `target_tick`.
- §4: transport row reads "UDP (plain in S2; QUIC datagrams from M7)" if
  the owner picks question 1(A).

## Alternatives rejected

- **Repeat only the last 4 turns of input, drop input over 3 s late**
  (draft 1): one lost burst left a permanent `seq` gap. Replaced by resend
  until acked.
- **Newest-first bundles** (draft 1): could starve the oldest missing
  bundle after a long drop. Replaced by oldest-first plus a paced burst.
- **Net I/O inside the sim loop and relay polled from Godot's main
  thread** (draft 1): a slow step or frame hitch stalled everyone.
- **Fixed D = 3 with a lobby override** (scope review): kept the
  override, but not a fixed default. D = 3 restamps nearly every command on
  a 60–80 ms Tailscale path, and the lobby formula is a few lines. D still
  never changes mid-match.
- **Dropping all 300-tick checkpoints** (scope review): one confirmed
  checkpoint costs one save every 10 s and is what makes `--replay`
  possible later. Only `--replay` itself is deferred.
- **Relay-run 3 s resume countdown** (draft 1): cut. `Resume` keeps only
  the time base it needs.
- **`frontline-cli net-test` in `sdk/crates/cli`**: would link the closed
  relay into the open SDK (§17 Q15, the CLI's `Cargo.toml` note). Moved to
  `crates/net`. `desync-diff` stays in the CLI because it needs only the
  sim, which the CLI already links at M0.
- **`#[cfg(test)]` HP-change hook** (draft 1): unreachable from a Godot
  build, and it would touch the sim. Replaced by the extra-command hook.
- **Port 27015:** clashes with Source dedicated servers and Steam's range.
- **"No restamps in 10 minutes on a LAN"** (draft 1): fails on Wi-Fi and
  untestable on one Mac. Replaced by a restamp rate and an owner check.
- **Dropping datagrams from an unexpected address:** blocks NAT
  rebinding. Token identifies the client.
- **UPnP port mapping now:** about a day of work and unreliable;
  Tailscale and the VPS relay cover the same need with no code.
- **TCP:** head-of-line blocking stalls every later bundle for an RTO.

## Owner questions

Work proceeds on the recommended defaults (marked **A**) until you answer.

1. **Transport.** **(A) Plain UDP now, QUIC datagrams at M7**
   (recommended) / (B) `quinn` now, as §4 says.
2. **Cross-network play.** **(A) Document Tailscale and a VPS relay, no
   code** (recommended) / (B) also add UPnP / (C) LAN only in S2.
3. **Friend's platform.** **(A) Mac only, unsigned export zip**
   (recommended) / (B) also Windows, which adds a Windows export and
   cross-platform determinism CI / (C) the friend builds from source.
4. **Player timeout.** **(A) Relay-injected `Surrender` after 60 s**
   (recommended) / (B) AI takeover, which needs `SetController` and sim
   state.

## Slice log

**Fix pass (2026-11-01)**, after the netcode-security and
spec-playability reviews. All runs on the M3 MacBook Air.

- `cargo test --workspace --release`: pass. Clippy `-D warnings` and
  `cargo fmt --check`: clean. New `crates/net/tests/relay_rules.rs` (6
  tests, one per review finding; all fail on the pre-fix relay/client).
- **Soak** (`cargo test -p frontline-net --release -- --ignored`, rough
  path 15 ms + 0–80 ms jitter, 5 % loss): seeds 1–3, about 36,000 ticks
  each, 1,199 hash checks per seed, 0 mismatches, 0 lost orders,
  restamps 0.029 %, 0 %, 0 %; stalls 63/1, 6/2, 0/1 ticks (before the fix
  pass: about 24,000 of 36,000 on every seed). Passed in 35 s.
- `frontline-net-test`:

  | Run | D / playout | Restamps | Stalls (ticks late) |
  |---|---|---|---|
  | `--jitter 2` | 2 / 1 | 0 of 556 | 0 / 0 |
  | `--jitter 40` | 2 / 2 | 0 of 556 | 0 / 0 |
  | `--seconds 1200 --jitter 40 --drop 0.05` (match won at about tick 31,500) | 2 / 2 | 10 of 2,994 (0.33 %) | 27 / 35 (was 3,918 / 3,566 of 24,142 at D = 4) |
  | same, `--tickets 100000` (full 36,000 ticks) | 2 / 2 | 15 of 3,408 (0.44 %) | 36 / 41 |
  | 300 s, 30 ms + 0–120 ms, 15 % drop | 4 / 6 and 4 | 44 of 878 (5.0 %) | 4 / 151 of 9,000 (was 99.6 %) |
  | 300 s, 0–40 ms, 30 % drop | 2 / 2 | 86 of 878 (9.8 %) | 484 / 484 (was 81 %) |
  | 60 s real UDP, 20 ms + 0–60 ms, 10 % drop (two runs) | 2 / 2 | 4 and 28 of 273 (1.5 %, 10 %) | 52–70 of 1,800 (was 62–67 %) |
  | 300 s, 30 ms + 0–10 ms | 2 / 2 | 0 of 878 | 0 / 0 |

  Every run: all hashes agree, every order ran exactly once, in order.
  The lower D trades restamps on lossy links (an order one or more ticks
  late; 5–10 % of orders at 10–30 % loss) for almost no stalls and 67 ms
  less lag on every other order; slot 0 can still raise D in the lobby.
- **Two Godot instances**, 3 minutes, 127.0.0.1: 181 of 181 hashes agree
  (ticks 30–5,430), both exit 0, D = 2, playout 1, 0 late, 10 stalls each
  (windowed start-up, as before).
- **Desync** (`--debug_inject_desync 1000` on the joiner): both stop at
  tick 1,021 after the mismatch at 1,020, exit 6, dumps complete;
  `desync-diff` names `stats.commands_applied 22 vs 23` and entity 21's
  waypoint and nav fields, and prints the dump's desync tick.
- **Joiner killed**: host waits 60 s, then wins ("Sam lost the connection
  and timed out."), exit 0. **Host killed**: guest shows "Reconnecting",
  then the disconnect screen, exit 7.
- **Export**: no Godot export templates on this Mac, so
  `tools/export_macos.sh --verify` built the editor-runtime fallback:
  `build/Frontline-312b20f0cd06-macos.zip`, 176 MB, universal bridge.
  The verify run shows the menu with the build id; two copies of the
  unzipped app (quarantine cleared) played a 90 s net match, hashes
  agree. Not done: a second macOS account (agents may not create
  accounts), an Intel Mac, a release-template export.
- `frontline-relay` type-checks for `aarch64-linux-android` (Linux
  kernel) and Windows (msvc, gnu); a glibc Linux build was not possible
  (no `x86_64-unknown-linux-gnu` std installed).
- Sim untouched (`git diff s1b-soldier-control -- crates/sim` is empty).
  Skirmish seed 1, AI vs AI, 3 threads, 3 runs: final hash
  `0x86f6991286fe64ed`, P99 0.274–0.276 ms, peak P99 0.329–0.335 ms
  (S1b: 0.275–0.276 and 0.335–0.337).
