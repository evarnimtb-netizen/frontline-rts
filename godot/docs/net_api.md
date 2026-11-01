# FrontlineNet: the API the S2 presenter assumes

The S2 presenter (menu → Multiplayer, lobby, match HUD in net mode) was
built against `godot/net/mock_net.gd` (`MockNet`), a GDScript stand-in with
no sockets. This page is the contract the bridge's real class has to meet,
or that integration adapts the GDScript to. Spec:
`docs/slices/S2-multiplayer.md` ("Presenter", "Session setup").

## Where it lives

`Session.ensure_net()` makes the object once and keeps it in
`Session.net`, so it survives the menu → match scene change:

- `ClassDB.class_exists("FrontlineNet")`: `ClassDB.instantiate`, and if it
  is a `Node`, `Session.add_child(net)`;
- otherwise `MockNet.new()` (the menu then works, but nothing is sent).

The presenter only calls it from the Godot main thread and only reads
snapshots from it: it never waits on the network.

## Lobby calls

| Call | Meaning |
|---|---|
| `set_name(name: String)` | Before `host`/`join`; `Hello` carries it. Optional: called only `if net.has_method("set_name")`. |
| `host(port: int) -> bool` | Start the in-game relay on `port` and join it as slot 0. `false` = could not start; `status().error` says why (`bind_failed`). |
| `join(addr: String) -> bool` | `addr` is already validated and normalised by `NetUi.parse_join`: `"1.2.3.4:47015"`, `"[::1]:47015"` (IPv6 always bracketed), or `"host.name:47015"` (the bridge resolves names, e.g. Tailscale MagicDNS). `false` = refused at once (`bad_address`). |
| `set_choice(choice: Dictionary)` | Partial updates: any of `faction`, `seed`, `map`, `input_delay`; missing keys keep their value. Slot 0 may send all four; slot 1 sends only `faction` (the relay ignores the rest from slot 1). The presenter clamps `input_delay` to 2–6. |
| `ready()` | Send `Ready` (setup hash). Note: if `FrontlineNet` is a `Node`, a method named `ready` clashes with the `ready` signal in GDScript; prefer a non-Node class (`RefCounted`) or rename it (e.g. `set_ready`) and change the three call sites (`multiplayer_panel.gd` `on_ready`, `session.gd` `_run_autoplay_net`, the test). |
| `leave()` | `Bye`, then `status().state == "idle"`. Safe to call in any state. |
| `status() -> Dictionary` | Snapshot, read every frame by the lobby. |

`status()`:

```text
{
  state: "idle" | "connecting" | "lobby" | "countdown" | "in_match" | "disconnected",
  slot: int,                 # own slot, -1 before Welcome
  slots: [                   # always 2 entries once in a lobby
    { name: String,          # "" = open slot
      faction: String,       # "germany" | "usa"
      ping_ms: float,        # -1 unknown
      ready: bool }, ...
  ],
  seed: int, map: String,    # slot 0's choice
  input_delay: int,          # D in ticks (2-6)
  suggested_delay: int,      # clamp(ceil((p99 RTT + jitter) / tick) + 1, 2, 4)
  mismatch: String,          # why Ready was refused (text or reason code), "" if none
  countdown_s: float,        # seconds left while state == "countdown"
  error: String,             # reason code once "disconnected" (see below)
}
```

The lobby goes `lobby` → `countdown` → `in_match`; on `in_match` the menu
calls `Session.start_net_match()`, which takes the own slot's faction, the
seed and the map from `status()` and loads `match/match.tscn`.

Reason codes the presenter turns into text (`NetUi.REASONS`; an unknown
code is shown as given): `build_mismatch`, `content_mismatch`, `protocol`,
`setup_mismatch`, `full`, `timed_out`, `no_answer`, `connection_lost`,
`host_left`, `opponent_left`, `left`, `flood`, `relay_closed`,
`bind_failed`, `bad_address`.

## Match calls

`match.gd` calls `source.attach_net(Session.net)` (only if the bridge has
it) before `source.start_match(Session.match_setup())`. In net mode the
setup also holds `net: true`, `local_player` (own slot), `input_delay`,
`speed: 1.0` and, when given, `debug_inject_desync: TICK`. Then
`set_speed(1.0)`.

The bridge's `FrontlineMatch.get_state()` then adds, in net mode:

- `net` (Dictionary; its presence switches the HUD to net mode),
- `pending_orders` (Array),
- `players[i].name` from the lobby (used for "paused by", "waiting for",
  the end screen).

Schema: `godot/docs/match_state.md`, "Net mode (S2)".

Intents in net mode: `pause {paused}` means PauseReq / resume for both
players (the pause menu no longer pauses on open); `surrender` as before;
`quit_to_menu` makes the presenter call `Session.goto_menu()`, which calls
`leave()`.

## Command line

```
-- --autoplay_net host:47015 --autoplay 3
-- --autoplay_net join:127.0.0.1:47015 --autoplay 3   (IPv6: join:[::1]:47015)
   [--net_name NAME] [--debug_inject_desync TICK] [--seed N]
```

The host picks Germany (and `--seed`), the guest USA; both ready, save
`s2_autoplay_lobby_<role>.png`, then play with `match/autoplay.gd` at 1x
(`--autoplay_speed` is ignored in net mode). The run exits with code 4 if
the lobby does not start within 90 s.
