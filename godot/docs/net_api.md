# FrontlineNet: the API the S2 presenter uses

**Status (integration):** the bridge's `FrontlineNet` (a `RefCounted`,
`crates/godot-bridge/README.md`) meets this contract with the same method
names, so the presenter needed no renames: `Session.ensure_net()` picks it
whenever the GDExtension is loaded. `MockNet` stays for the screenshot
stages and `tools/test_net_ui.gd`. The bridge also has `host_loopback(port)`
(used by `--autoplay_net host:`), `set_ready()`, `build_id()` (shown in the
main menu), and `FrontlineMatch.get_net_hashes()`.

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
| `set_choice(choice: Dictionary)` | Partial updates: any of `faction`, `seed`, `map`, `input_delay`, and (S18) `team` (0–3, -1 none), `players` (2–4) and `ai` (Array of `{slot, faction, team, difficulty}`: the AI fill, replacing the list; difficulty 0 easy, 1 normal, 2 hard); missing keys keep their value. Slot 0 may send all; a joiner sends only `faction` and `team` (the relay ignores the rest from him). The presenter clamps `input_delay` to 2–6. |
| `take_slot(slot: int)` | S18: a joiner moves to an open slot of the match (not slot 0, not an AI slot). |
| `ready()` | Send `Ready` (setup hash). Note: if `FrontlineNet` is a `Node`, a method named `ready` clashes with the `ready` signal in GDScript; prefer a non-Node class (`RefCounted`) or rename it (e.g. `set_ready`) and change the three call sites (`multiplayer_panel.gd` `on_ready`, `session.gd` `_run_autoplay_net`, the test). |
| `leave()` | `Bye`, then `status().state == "idle"`. Safe to call in any state. |
| `status() -> Dictionary` | Snapshot, read every frame by the lobby. |

`status()`:

```text
{
  state: "idle" | "connecting" | "lobby" | "countdown" | "in_match" | "disconnected",
  slot: int,                 # own slot, -1 before Welcome
  slots: [                   # one entry per match slot once in a lobby (2-4)
    { name: String,          # "" = open slot (or an AI slot)
      faction: String,       # "germany" | "usa"
      ping_ms: float,        # -1 unknown
      ready: bool,
      team: int,             # S18: -1 none (alone), else 0-3
      ai: int }, ...         # S18: AI fill difficulty 0-2, -1 a player / open
  ],
  players: int,              # S18: slots in the match (2 = 1v1, 4 = 2v2)
  seed: int, map: String,    # slot 0's choice
  input_delay: int,          # D in ticks (2-6)
  suggested_delay: int,      # clamp(ceil((p99 RTT / 2 + jitter) / tick), 2, 4)
  mismatch: String,          # why Ready was refused (text or reason code), "" if none
  countdown_s: float,        # seconds left while state == "countdown"
  error: String,             # reason code once "disconnected" (see below)
  error_detail: String,      # more about it ("Their build: … Yours: …"), "" none
  host_port: int,            # in-game relay port when hosting, 0 when joined
}
```

The lobby goes `lobby` → `countdown` → `in_match`; on `in_match` the menu
calls `Session.start_net_match()`, which takes the own slot's faction, the
seed and the map from `status()` and loads `match/match.tscn`.

Reason codes the presenter turns into text (`NetUi.REASONS`; an unknown
code is shown as given): `build_mismatch`, `content_mismatch`, `protocol`,
`setup_mismatch`, `full`, `started`, `timed_out`, `no_answer`, `connection_lost`,
`host_left`, `opponent_left`, `left`, `flood`, `relay_closed`,
`bind_failed`, `bad_address`, `content_missing`, `bad_name`. The
reject codes (`NetUi.REJECT_CODES`) title the page "CAN'T JOIN".

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

Intents in net mode: `pause {paused}` means PauseReq / resume for every
player (the pause menu no longer pauses on open); S18 `map_ping {target}`
pings the map for the allies (the `.` key; at most one per `[teams]
ping_gap_s`), and allies' pings come back in `net.pings` (`[{slot, name,
pos}]`, each once); `surrender` as before;
`quit_to_menu` makes the presenter call `Session.goto_menu()`, which calls
`leave()`.

## Command line

```
-- --autoplay_net host:47015 --autoplay 3
-- --autoplay_net join:127.0.0.1:47015 --autoplay 3   (IPv6: join:[::1]:47015)
   [--net_name NAME] [--debug_inject_desync TICK] [--seed N]
   [--net_delay D] [--net_bind_all] [--autoplay_shots DIR]
   [--test_unlock] [--start_manpower N]
```

S3: `--test_unlock` (vehicles unlocked at once) and `--start_manpower N`
must be given to both instances (the setups are hashed at Ready). The S3
net check uses `--test_unlock --start_manpower 4000`: each instance buys
three tanks and writes `vehicles_own` (and `vehicles_enemy_seen`) into
`s2_hashes_<role>.json`; autoplay exits 3 if fewer than 2 own vehicles
were drawn.

The host picks Germany (and `--seed`, and D with `--net_delay`), the guest
USA; both ready, save `s2_autoplay_lobby_<role>.png`, then play with
`match/autoplay.gd` at 1x (`--autoplay_speed` is ignored in net mode). The
autoplay host's relay binds 127.0.0.1 only (`host_loopback`, no macOS
firewall prompt) unless `--net_bind_all`. `match/autoplay_net.gd` adds the
net shots (`s2_<role>_readout|pending|waiting|desync|disconnect|end.png`)
and writes `s2_hashes_<role>.json` (every published hash), which
`tools/compare_hashes.py` compares.

Exit codes: 0 match over, 3 a required shot missing, 4 the lobby did not
start within 90 s (or host/join failed), 6 desync (after the banner shot,
once the dump is written), 7 disconnected (after the disconnect shot).

Two instances on one Mac. The integration ran two 1600x900 windows side
by side on a 3420 px screen; the S2 fix pass used 1280x720 windows at
`--position 0,40` and `--position 1180,500` (partly overlapping, both
visible, so App Nap does not throttle either) on a 2560 px screen. The
shots come out 1600x900 either way:

```
G=~/Applications/Godot.app/Contents/MacOS/Godot
$G --path godot --resolution 1600x900 --position 0,40 -- \
   --autoplay_net host:47015 --autoplay 3 --net_name Erik --autoplay_shots /tmp/s2 &
sleep 4
$G --path godot --resolution 1600x900 --position 1700,40 -- \
   --autoplay_net join:127.0.0.1:47015 --autoplay 3 --net_name Sam --autoplay_shots /tmp/s2
python3 -I tools/compare_hashes.py /tmp/s2/s2_hashes_host.json /tmp/s2/s2_hashes_join.json
```

Add `--debug_inject_desync 1000` to the joiner for the desync banner and
dump; `kill -9` either instance mid-match for the waiting overlay and the
timeout or disconnect result on the other.
