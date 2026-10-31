# Match state and intents (S1 presenter interface)

The S1 match presenter has one entry point: `MatchView`
(`godot/match/match_view.gd`). Each frame the match root gives it one plain
`Dictionary`, the **match state**, and the view reports the player's wishes
as **intents** (signals). The view never reads the sim. Whoever produces the
state is the **source**: `MockMatchSource` today, a bridge-backed source after
integration.

```gdscript
# match.gd (shape of the wiring)
view.camera = $Camera                 # rts_camera.gd
view.height_fn = source.height_at     # (x, z) -> ground y, for picking/overlays
camera.height_fn = source.height_at
view.intent.connect(func(kind, data): source.handle_intent(kind, data))
func _process(_d): view.update_state(source.get_state())
```

A source has three methods:

| Method | Meaning |
|---|---|
| `get_state() -> Dictionary` | One frame of state. `events` and `effects` hold only what is new since the previous call. |
| `handle_intent(kind: StringName, data: Dictionary)` | Turns a player intent into sim `Command`s. |
| `height_at(x: float, z: float) -> float` | Ground height including dug terrain (`FrontlineSim.get_height_at`). |

## Conventions

- **Space.** Godot world space, metres: map centred on the origin, x east,
  z south, y up. Sim (x east, y north, z up) maps to Godot as
  `(x − half, z, −(y − half))` (`crates/godot-bridge/src/present.rs`).
  Positions are `Vector3`.
- **Players.** `0` and `1`. `-1` means nobody (neutral point, no winner).
  Colours in the HUD are relative to `local_player` (own blue, enemy red).
- **Ids.** Squad ids are `int`, stable for the squad's life. Send `squads`
  sorted by ascending id (sim iteration order). Every id list the view emits
  is a sorted `PackedInt32Array`.
- **Fog of war.** Leave out enemy squads the local player has not spotted
  (filter in Rust, §5.12.3). The view draws, selects and lists everything it
  is given. Own squads are always present.
- **Floats are fine.** This is presentation data; convert from Q16.16 in the
  bridge. Nothing here feeds back into the sim except through intents.
- Unknown keys are ignored. Optional keys are marked *opt*.

## State schema

```text
state = {
  tick: int,                     # sim tick of this snapshot
  time_s: float,                 # match clock (s)
  paused: bool,                  # opt; shows "PAUSED" in the top bar
  local_player: int,             # 0 or 1
  map: {
    id: String,                  # "kessel_ridge"
    name: String,                # "Kessel Ridge"
    size_m: float,               # 1024.0 (square map)
    minimap: Texture2D,          # opt; top-down image, north up. Sent whenever
                                 # it changes (the view keeps the last one)
  },
  rules: {                       # constants from MatchSetup
    pop_cap: int,                # 120
    tickets_max: int,            # 500
    dig_max_m: float,            # 60.0  (dig tool length cap)
    mortar_min_m: float,         # 60.0
    mortar_max_m: float,         # 600.0
    capture_radius_m: float,     # opt, 15.0
  },
  players: [                     # index = player id
    {
      id: int,
      faction: String,           # "germany" | "usa"
      name: String,              # opt
      is_ai: bool,               # opt
      tickets: float,
      manpower: float,
      income: float,             # manpower per second, including point bonus
      pop: int,                  # living soldiers
      points_held: int,          # opt
      ticket_drain: float,       # tickets/s being lost now (0 = not draining)
    }, ...
  ],
  points: [                      # capture points, A..E
    {
      id: String,                # "A"
      pos: Vector3,
      radius: float,             # opt, default 15
      owner: int,                # -1 neutral
      capturing: int,            # side making progress, -1 none
      progress: float,           # 0..1 towards `capturing`
      contested: bool,           # both sides inside: frozen
    }, ...
  ],
  squads: [                      # own + spotted enemy, ascending id
    {
      id: int,
      owner: int,
      type: String,              # "rifle" | "mg" | "engineer" | "mortar"
      name: String,              # opt, display name ("Rifle squad")
      men: int,                  # alive
      men_max: int,
      health: float,             # 0..1, summed member HP / (100 * men_max)
      suppression: float,        # 0..1, squad average
      status: String,            # "ok" | "suppressed" | "pinned" | "retreating"
      order: String,             # "idle" | "move" | "attack_move" | "retreat"
                                 # | "dig" | "barrage" | "setup" (MG setting up)
      pos: Vector3,              # selection anchor (leader point)
      screen: Vector2,           # opt; anchor in viewport pixels. If absent the
                                 # view projects pos + 1.8 m with the camera
      members: PackedVector3Array,  # opt; living members (mock renderer only)
      target: Vector3,           # opt; current order target
      dig_line: PackedVector3Array, # opt; own squads with a Dig order
      dig_progress: float,       # opt; 0..1 of dig_line dug
    }, ...
  ],
  callin: {                      # local player only
    catalog: [ { type: String, name: String, cost: int, delay_s: float, men: int }, ... ],
    queue: [ { type: String, remaining_s: float, total_s: float, men: int }, ... ],
    queue_max: int,              # opt, default 5
  },
  events: [                      # new since last frame; the feed shows them
    {
      kind: String,              # "point_captured" | "point_lost" | "squad_lost"
                                 # | "squad_arrived" | "under_attack"
                                 # | "tickets_low" | "info"
      text: String,              # "Point B lost."
      time_s: float,             # opt; match time of the event
      pos: Vector3,              # opt; makes the message clickable / Space jumps
      player: int,               # opt
    }, ...
  ],
  effects: [                     # new since last frame; drive sounds (and later VFX)
    { kind: "shot", weapon: "rifle" | "mg" | "smg", pos: Vector3 },
    { kind: "mortar_launch", pos: Vector3 },
    { kind: "explosion", pos: Vector3 },
    { kind: "dig", pos: Vector3 },        # a spade of earth, about once a second per digging squad
  ],
  result: {
    status: String,              # "running" | "won" | "lost" (local view)
    winner: int,                 # -1 while running
    reason: String,              # "tickets" | "annihilation" | "surrender"
  },
  stats: [                       # index = player id; read for the end screen
    { kills: int, losses: int, points_held_s: float, squads_called: int, trench_m: float },
    ...
  ],
}
```

Where the bridge gets it (S1 spec, "Data flow to the presenter"): squads and
soldiers from the `RenderSnapshot` per-soldier/squad data; `points`, `players`,
`result` from the match state; `events` from point owner changes, squad deaths
and call-in arrivals between two snapshots; `effects` from "shots fired and
explosions this tick" (aggregate small-arms shots per squad per frame so a
firefight does not flood the audio pool; the AudioManager also caps voices).

## Intents

Every intent is emitted as `intent(kind: StringName, data: Dictionary)` and
also as the typed signal listed. `squads` is always a sorted
`PackedInt32Array` of own squad ids.

| `kind` | `data` | Typed signal | Sim command |
|---|---|---|---|
| `select` | `{squads}` | `selection_changed(squad_ids)` | none (presenter state) |
| `move` | `{squads, target: Vector3}` | `move_ordered(squad_ids, target)` | Move |
| `attack_move` | `{squads, target: Vector3}` | `attack_move_ordered(squad_ids, target)` | AttackMove |
| `stop` | `{squads}` | `stop_ordered(squad_ids)` | Stop |
| `retreat` | `{squads}` | `retreat_ordered(squad_ids)` | Retreat |
| `dig` | `{squads, polyline: PackedVector3Array}` | `dig_ordered(squad_ids, polyline)` | Dig (≤ `rules.dig_max_m`; only rifle/engineer ids are sent) |
| `barrage` | `{squads, target: Vector3}` | `barrage_ordered(squad_ids, target)` | Barrage (only mortar ids within range are sent) |
| `call_in` | `{type: String}` | `call_in_requested(squad_type)` | CallIn |
| `surrender` | `{}` | `surrender_requested()` | Surrender |
| `pause` | `{paused: bool}` | `pause_requested(paused)` | none in lockstep; vs AI the source may stop stepping |
| `quit_to_menu` | `{}` | `quit_to_menu_requested()` | none; match.gd changes scene |

Notes for the integrator:
- Right-click with mortars selected splits the order: mortars whose range
  covers the point get `barrage`, the rest of the selection gets `move`
  (two intents in one click). To move mortars, use M (move tool).
- Right-click on a spotted enemy squad sends `attack_move` to its position.
- The minimap's right-click uses the same rules as the world.
- Dig polylines are ground points (`height_at`) clicked by the player and
  already capped to `dig_max_m`; the sim should still validate.
- The view does not throttle intents; the source should drop orders for ids
  that are no longer the local player's.

## Input map (defaults)

Registered at runtime by `MatchInput.ensure_actions()` (physical keys,
rebindable through InputMap):

| Input | Action |
|---|---|
| Left click / drag | select / box-select; Shift adds or toggles |
| Double-click | select all own squads of that type on screen |
| Right-click | contextual order (move, attack enemy, mortar barrage) |
| M / A / B | move / attack-move / barrage tool (left click places, Shift keeps the tool, right-click or Esc cancels) |
| S / R | stop / retreat (immediate) |
| D | dig tool: click points, Backspace removes the last, right-click digs |
| Ctrl+1–9 (or Cmd) | store group; 1–9 recall, twice quickly centres the camera |
| Space | jump to the latest event with a position |
| Esc / F10 | cancel tool, else match menu |
| Arrows, screen edges | pan; Q/E rotate; wheel zoom (10–150 m); middle-drag rotate/tilt |

## Running the mock

- `Godot --path godot res://match/match.tscn` runs the HUD on the mock source.
- `godot/tools/capture_screens.sh <Godot>` saves the menu and HUD screenshots
  to `docs/slices/screens/`.
- `Godot --headless --path godot -s res://tools/check_project.gd` loads every
  script and scene and runs the menu and the match for a few frames.
