# godot-bridge: S2 net API

The GDExtension's multiplayer side (S2, `docs/slices/S2-multiplayer.md`).
The presenter contract it meets is `godot/docs/net_api.md` and
`godot/docs/match_state.md` "Net mode (S2)" (written against `MockNet`);
this page lists what the real classes do, including the differences.

## Threads

- **Main thread:** `FrontlineNet` and `FrontlineMatch`. They only send
  small messages to channels and read snapshots; nothing waits on the
  network. Host names are resolved on a short-lived thread.
- **`frontline-net`** (one per `FrontlineNet` connection): owns the socket
  and the `NetClient` (`crates/net/src/client_io.rs`).
- **`frontline-relay`** (host only): the in-game relay
  (`crates/net/src/server.rs`).
- **`frontline-match`**: the sim thread. In a net match it reads bundles,
  pauses, desyncs and goodbyes from its own channel and steps at the
  relay's schedule (`src/match_net.rs`, `NetStepper`).

## FrontlineNet (RefCounted)

Make it with `ClassDB.instantiate(&"FrontlineNet")`. It is a
`RefCounted`, so `ready()` does not clash with `Node.ready`, and
`Session.add_child` is skipped. Keep a reference across the
menu → match scene change.

| Call | What it does |
|---|---|
| `set_name(name: String)` | `Hello` name. Trimmed, control characters dropped, at most 24 bytes; empty means "Player". Call it before `host` or `join`. |
| `host(port: int) -> bool` | Starts the in-game relay on `[::]:port` (dual-stack, so IPv4 and IPv6 both work) and joins it over `127.0.0.1` as slot 0. On `false`, `status().error` is `bind_failed`, or `content_missing` if `mods/ww2` could not be loaded. |
| `host_loopback(port: int) -> bool` | Same, but the relay binds `127.0.0.1:port` only. Use it for two instances on one Mac: no macOS "accept incoming connections" prompt. |
| `join(addr: String) -> bool` | Takes `1.2.3.4:47015`, `[::1]:47015` or `name:47015`. A name is resolved off the main thread. `false` means the address is malformed (`bad_address`). If a lookup fails later, the state becomes `disconnected` with `bad_address`. |
| `set_choice(d: Dictionary)` | A partial update of any of `faction` (`germany`/`usa`), `seed`, `map` (short id) and `input_delay` (clamped to 2–6). The full choice is resent every time, because the relay overwrites the faction and clears both Ready flags on any change. Slot 1's seed, map and D are ignored by the relay. |
| `ready()` / `set_ready()` | Resolves the setup this client would play (both slots human, the lobby's factions, seed and map) and sends `Ready(xxh3(postcard(setup)))`. If the second slot is empty or the factions are equal, nothing is sent and `mismatch` says why. |
| `leave()` | Sends `Bye` and goes back to `idle`; safe in any state. Threads stop in the background: the client's `Bye(Quit)` goes first (a player who quits mid-match surrenders), then a hosted relay sends `Bye(Shutdown)` to whoever is left. A later `host()` waits for that to finish, so the port is free again. |
| `set_debug_inject_desync(tick: int)` | Dev hook for the next match. The same effect as `debug_inject_desync` in the `start_match` dictionary. |
| `status() -> Dictionary` | Snapshot (below). Calling it also processes the net thread's events, so call it every frame while in the lobby. |

`status()`:

```text
{ state: "idle" | "connecting" | "lobby" | "countdown" | "in_match" | "disconnected",
  slot: int (-1 before Welcome),
  slots: [ {name, faction (short id, "" none), ping_ms (relay's P99 RTT, -1 open slot), ready}, x2 ]
         (empty before Welcome; both ready once the match started),
  seed: int, map: String (short id),
  input_delay: int,       # slot 0's D, else the relay's suggestion; Start's D once started
  suggested_delay: int,   # relay's clamp(ceil((p99 RTT + jitter)/period) + 1, 2, 4), worst slot
  mismatch: String,       # "" | "setup_mismatch" | relay note text | local reason text
  countdown_s: float,     # until turn 0 opens, from Start mapped through the synced clock
  error: String,          # reason code once "disconnected"
  host_port: int }        # the relay's port when hosting (0 otherwise)
```

**Reason codes:** `bad_address`, `bind_failed`, `content_missing`,
`no_answer` (no `Welcome` within 10 s), `connection_lost` (relay silent
for 10 s in the lobby), `protocol`, `build_mismatch`, `content_mismatch`,
`full` (also when the match already started), `bad_name`, `timed_out`,
`flood`, `relay_closed` (`Bye(Shutdown)`: the host left or the relay
stopped).

There is no `host_left` or `opponent_left`. The client cannot tell a
host's in-game relay from a standalone one, so both report `relay_closed`.
An opponent who leaves in the lobby just frees the slot. An opponent who
leaves mid-match surrenders, and the end screen shows it.

**Slot 0** goes to whichever `Hello` reaches the relay first. The host
joins its own relay over loopback right away, so in practice it gets slot
0. A test that hosts and joins in the same instant must wait for the host
to reach the `lobby` state first.

## FrontlineMatch in net mode

1. `attach_net(net)` with the `FrontlineNet` (anything else detaches).
2. `start_match({net: true, local_player: N, debug_inject_desync: T?})`.
   The setup comes from the relay's `Start`, not from the dictionary: seed,
   map and both factions, with both slots human. Its hash is checked
   against the one sent with `Ready`. `local_player` is only cross-checked
   (the relay's slot wins). Call it once `status().state` is `in_match`, or
   during `countdown`. Each match can be taken once.
3. `set_speed()` does nothing in net mode: the match runs at 1x. A
   `pause` intent with `{paused}` sends `PauseReq` or `ResumeReq` for both
   players. The pause shows once the relay confirms it (`net.state ==
   "paused"`, `paused == true`).

`get_state()` adds:

```text
net: { ping_ms, input_delay, delay_ms, restamps (own), stalls,
       state: "ok" | "unstable" | "waiting" | "paused" | "desync",
       paused_by (name), paused_slot (only while paused),
       waiting_for (name), waiting_slot (own slot = "reconnecting"), waiting_s_left,
       desync_tick (-1), desync_dump (absolute folder, "" until written),
       desync_dump_error (only if writing failed),
       disconnect_reason ("" | timed_out | flood | relay_closed | connection_lost) },
pending_orders: [ {pos: Vector3 (the clicked target, or the polyline's middle point), kind} ],
players[i].name   # lobby names; is_ai is false for both
```

- **`state`:** `unstable` means a peer has been silent for 2 s, or this
  client has heard nothing from the relay for 2 s. If the relay is silent
  for 10 s, the state is `waiting` for the own slot ("reconnecting"),
  counting down from 70 s. At 75 s, `disconnect_reason` becomes
  `connection_lost`. If the relay pauses for a silent player, the state is
  `waiting` with the relay's 60 s timer.
- **`disconnect_reason`** is set only after every bundle received before
  the goodbye has run and the match is still undecided. A relay that
  closes after a surrender therefore shows the end screen, not a
  disconnect.
- **`pending_orders`:** each positional intent (`move`, `attack_move`,
  `barrage`, `throw`, `dig` and their `*_units`/`throw_each` forms) adds a
  marker. A marker stays until the last command of that intent has run in
  the sim. Orders sent while the match is paused stay pending until it
  resumes, and after a desync they stay forever. At most 64 markers are
  kept.
- **Desync dump:** `user://desync/<seed>-<8 hex>/slot<N>/`, with
  `manifest.json`, `checkpoint.sav` (the newest checkpoint the relay
  confirmed), `paused.sav` and `bundles.jsonl`. The match id is the same on
  both clients. The host also copies its relay log there as `relay.jsonl`;
  the hosted relay writes it to `user://net/relay_last.jsonl`, which each
  hosted session overwrites. Compare the two `paused.sav` files with
  `frontline-cli desync-diff`.

## Rust API (the next stage)

- `match_thread::MatchHandle::spawn_net(world, slot, NetLink, NetMatchInfo)`.
  - `send(kind) -> Option<u64>` returns the send ordinal, which equals the
    command's `seq` in net mode. Commands over the wire limits are dropped
    (`None`).
  - `set_paused(bool)`, `is_net()`.
- `MatchCounters` gains:
  - `rtt_ms` (-1 means unknown), `input_delay`, `restamps`, `stall_ticks`;
  - `net_state` (`match_net::NetState as u8`), `desync_tick` (-1 means
    none);
  - `own_executed`;
  - `hashes`: published `(tick, hash)` pairs, the last 4096;
  - `net: Mutex<NetDetail>`: paused_by, waiting_for and seconds left, dump
    path or error, disconnect code.
- `counters.log` holds every player's network commands, keyed by the tick
  before the step (`world.tick()`, no longer `+ 1`). A skirmish still logs
  only the local player.
- `match_net::NetStepper` and `LinkIo` are sans-IO: tests drive them on a
  virtual clock with a fake link.
- `net_session::NetSession` is the Godot-free lobby:
  - `host_on(addr)`, `join(addr)`, `set_choice(ChoiceUpdate)`, `ready()`,
    `view()`, `take_match() -> NetMatch { setup, slot, link, info }`,
    `leave()`;
  - `inject_desync` and `user_dir` are public fields.
- `match_setup::MenuChoice.slots: Option<[SlotChoice; 2]>` names both
  slots' factions and whether each is human.

## Tests

- `cargo test -p godot-bridge` runs:
  - `match_net`: pacing, catch-up and the never-past-newest rule on a fake
    link; pause, waiting and resume; the readout; the confirmed checkpoint
    and the dump files; the desync hook; `Bye(MatchOver)`.
  - `net_session`:
    - two `MatchHandle`s in net mode on loopback UDP for 60 game seconds
      (`FRONTLINE_NET_TEST_SECONDS` overrides), comparing hashes, logs and
      orders run;
    - an injected desync that dumps both clients;
    - lobby reason codes.
