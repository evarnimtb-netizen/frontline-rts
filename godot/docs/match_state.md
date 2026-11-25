# Match state and intents (S1/S1b presenter interface)

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
- **Ids.** Squad and soldier ids are `int`, stable for the entity's life.
  Vehicle ids (S3) share the soldier id space; see "Vehicles (S3)".
  Send `squads` and `soldiers` sorted by ascending id (sim iteration order).
  Every id list the view emits is a sorted, deduplicated `PackedInt32Array`.
  Soldier and squad ids are separate id spaces.
- **Selection is soldiers (S1b).** The view's selection is a set of soldier
  ids. A state without `soldiers` (today's S1 bridge) still works: the view
  makes one stand-in soldier per squad with the squad's id, so every
  selection is whole squads and only squad-level intents go out.
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
    suppress_line_max_m: float,  # 40.0  (S3b suppress tool line cap)
    mortar_min_m: float,         # 60.0
    mortar_max_m: float,         # 600.0
    capture_radius_m: float,     # opt, 15.0
    throw_range_m: {             # opt (S1b); (min, max) m per item
      frag: Vector2, smoke: Vector2, satchel: Vector2,   # (5,30) (5,30) (3,8)
    },
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
      status: String,            # "ok" | "suppressed" | "heads_down" (S3b: half ducked below a trench lip) | "pinned" | "retreating"
      order: String,             # "idle" | "move" | "attack_move" | "retreat"
                                 # | "dig" | "fire_mission" (S5b; was "barrage") | "setup" (MG setting up)
                                 # | "follow" (S3 escort; `follow`: the vehicle)
                                 # | "suppress" (S3b area fire; `suppress_line`)
      pos: Vector3,              # selection anchor (leader point)
      screen: Vector2,           # opt; anchor in viewport pixels. If absent the
                                 # view projects pos + 1.8 m with the camera
      units: PackedInt32Array,   # S1b; living soldier ids (as sent in
                                 # `soldiers`), ascending
      items: { frag: int, smoke: int, satchel: int },  # S1b, own squads;
                                 # totals over the squad's men
      target: Vector3,           # opt; current order target
      dig_line: PackedVector3Array, # opt; own squads with a Dig order
      dig_progress: float,       # opt; 0..1 of dig_line dug
      suppress_line: PackedVector3Array, # opt, S3b; the 2 ends of the line a
                                 # squad on Suppress fires at (equal for a point)
    }, ...
  ],
  soldiers: [                    # S1b; own + spotted enemy, living, ascending id
    {
      id: int,
      squad: int,                # squad id
      owner: int,
      slot: int,                 # 0..men_max-1, stable formation slot; pips
                                 # and detail rows follow it (dead slots = X)
      pos: Vector3,              # feet, the interpolated position the
                                 # renderer draws (picking projects it)
      hp: float,                 # 0..100; under 60 shows as wounded
      stance: String,            # "stand" | "crouch" | "prone" (effective)
      pinned: bool,              # suppressed past the pinned threshold
      ducked: bool,              # S3b: ducked below a trench lip (not firing)
      suppression: float,        # opt, 0..1
      in_trench: bool,           # opt; standing in a dug trench (bridge)
      # Own soldiers only (leave these out for enemies):
      stance_override: String,   # "auto" | "stand" | "crouch" | "prone"
      detached: bool,            # follows his own unit order
      order: String,             # opt, detached men: "move" | "attack_move"
                                 # | "retreat" | "dig" | "hold" | "throw"
                                 # | "follow" (S3; `follow`: the vehicle)
                                 # | "suppress" (S3b; target: the line's middle)
      target: Vector3,           # opt, detached men with a target: drawn as
                                 # an order marker
      items: { frag: int, smoke: int, satchel: int },
      name: String,              # opt; else derived from type + slot
      weapon: String,            # opt; else derived from faction + type
    }, ...
  ],
  smokes: [                      # S1b; active smoke clouds
    { pos: Vector3, radius: float, density: float,     # radius m, density 0..1
      id: int },                 # opt; stable per cloud (puff placement)
  ],
  thrown: [                      # S1b; items in flight (and fused on the ground)
    { kind: String, pos: Vector3, landed: bool,        # "frag" | "smoke" | "satchel"
      id: int, owner: int },     # opt
  ],
  callin: {                      # local player only. S3b (owner 2026-11-01):
                                 # no queue, no delivery time; a buy spawns
                                 # the squad in the base at once
    catalog: [ { type: String, name: String, cost: int, men: int,
                 short: String,           # opt; tile name ("Engineers")
                 recall_s: float,         # S3b; the type's re-call cooldown
                                          # after a buy (rifle 24.0)
                 cooldown_s: float,       # opt (S3b: sent while > 0); re-call
                                          # cooldown left
                 cooldown_total_s: float, # opt, with cooldown_s; its full
                                          # length (the tile's fill)
               }, ... ],
                                 # S3b: `delay_s`, `queue` and `queue_max`
                                 # are gone
  },
  events: [                      # new since last frame; the feed shows them
    {
      kind: String,              # "point_captured" | "point_lost" | "squad_lost"
                                 # | "squad_arrived" | "under_attack"
                                 # | "call_in_refused" (S3b)
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
    { kind: "explosion", pos: Vector3, size: String },  # size (S1b, opt):
                                          # "mortar" | "frag" | "satchel"
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

### The bridge source (`FrontlineMatch`, crates/godot-bridge/src/match_node.rs)

`match/match.tscn` uses the GDExtension node `FrontlineMatch` as its source.
`match.gd` calls `start_match(Session.match_setup())` (faction, difficulty,
map, seed; optional `tickets`, `start_manpower`, `ai_vs_ai`); the bridge reads
`mods/ww2` (next to the project, the executable or the source tree, or the
node's `mods_dir`), resolves a `MatchSetup` and runs the sim on its own
thread with the AI inside it. The local player is slot 0.

What it fills, beyond the schema above:
- `squads[].pos` is the leader's interpolated position (the drawn one);
  `screen` is not sent, the view projects `pos` (the camera runs first in the
  frame, so the projection matches the picture).
- `squads[].rounds` (barrage rounds left), `points[].value`.
- `players[].pop` counts living soldiers and (S3) each living vehicle's
  `pop` (S3b: there is no queue to add); `income` and
  `ticket_drain` are per second.
- `stats[].squads_called` and `trench_m` are counted presenter-side (squads
  that arrived after the start; dig orders' line length × progress).
- `effects`: one `shot` per firing squad per frame (fog-filtered),
  `mortar_launch` per shell, `explosion` per burst (with `size`: `mortar`,
  `frag` or `satchel`), `dig` about once a second per digging squad in view.
- S1b: `soldiers` (own, and enemies the local player sees) with `pos` the
  interpolated position the bridge draws this frame; `slot` is kept for a
  man's life (newcomers take the lowest free slot); `in_trench`; own men
  also `stance_override`, `detached`, `items`, and for detached men
  `order` and `target` (move, attack-move, retreat: own base, dig: middle
  of the line; `throw` while winding up). `squads[].units` lists the ids
  sent in `soldiers`. `smokes` are the clouds within sight range of one of
  the local player's men; `thrown` are his own items, items whose thrower
  he sees, and items within 40 m of his men. `rules.throw_range_m` comes
  from the local faction's items.
- S1b intents: `move_units`, `attack_move_units`, `stop_units`,
  `retreat_units`, `dig_units`, `set_stance`, `throw`, `throw_each` keep
  only the local player's living men (as of the newest frame), sorted,
  deduplicated, at most 120, and become one `MoveUnits` /
  `AttackMoveUnits` / `StopUnits` / `RetreatUnits` / `DigUnits` /
  `SetStance` / `Throw` / `ThrowEach` each; positions are quantised to
  millimetres once (`crates/godot-bridge/src/intents.rs`). `regroup`
  sends the sim's `Regroup { squad }` per own squad.
- `events` are derived from frame diffs (point owner changes, own squads
  arriving, lost, losing men or getting suppressed, tickets below 100, the
  match result).

Other methods: `get_map_info()` (woods, points, bases in Godot space,
factions, own base), `squad_ids(owner, type)`, `squad_pos(id)`,
`set_speed(x)` (0.25–8, scripted runs), `get_stats()`, `get_last_error()`,
and (S1b) `dug_depth_at(x, z)`: how far the ground lies below the undug
surface there (trenches and craters as drawn; scripted runs use it to find
trenches).
Soldiers (stance poses follow stance orders: the sim's stance already
includes them), corpses (death fall, 30 s), tracers, mortar shells, thrown
items (dark body and a pale trail), muzzle flashes and the chunked terrain
are drawn by the bridge; `match/world_fx.gd` draws woods, flags,
explosions scaled by `size`, and smoke clouds (pooled billboard puffs in
one MultiMesh) from the state.

Where the bridge gets it (S1 spec, "Data flow to the presenter"): squads and
soldiers from the `RenderSnapshot` per-soldier/squad data; `points`, `players`,
`result` from the match state; `events` from point owner changes, squad deaths
and call-in arrivals between two snapshots; `effects` from "shots fired and
explosions this tick" (aggregate small-arms shots per squad per frame so a
firefight does not flood the audio pool; the AudioManager also caps voices).

## Intents

Every intent is emitted as `intent(kind: StringName, data: Dictionary)` and
also as the typed signal listed. `squads` is always a sorted
`PackedInt32Array` of own squad ids; `units` a sorted `PackedInt32Array` of
own soldier ids.

| `kind` | `data` | Typed signal | Sim command |
|---|---|---|---|
| `select` | `{squads, units}` | `selection_changed(squad_ids)`, `soldier_selection_changed(unit_ids)` | none (presenter state); `squads` = squads with a selected man |
| `move` | `{squads, target: Vector3}` | `move_ordered(squad_ids, target)` | Move |
| `attack_move` | `{squads, target: Vector3}` | `attack_move_ordered(squad_ids, target)` | AttackMove |
| `stop` | `{squads}` | `stop_ordered(squad_ids)` | Stop |
| `retreat` | `{squads}` | `retreat_ordered(squad_ids)` | Retreat |
| `dig` | `{squads, polyline: PackedVector3Array}` | `dig_ordered(squad_ids, polyline)` | Dig (≤ `rules.dig_max_m`; only rifle/engineer squads) |
| `fire_mission` (S5b; `barrage` an alias for one slice) | `{squads, target: Vector3, rounds: int}` | `barrage_ordered(squad_ids, target)` | Fire mission (mortar squads; a selected mortar man sends his squad; see "S5b") |
| `regroup` | `{squads}` | `regroup_ordered(squad_ids)` | Regroup (S1b sim): clears `detached`, the men rejoin the formation; the squad keeps its order |
| `move_units` | `{units, target: Vector3}` | `move_units_ordered(unit_ids, target)` | MoveUnits |
| `attack_move_units` | `{units, target: Vector3}` | `attack_move_units_ordered(unit_ids, target)` | AttackMoveUnits |
| `stop_units` | `{units}` | `stop_units_ordered(unit_ids)` | StopUnits |
| `retreat_units` | `{units}` | `retreat_units_ordered(unit_ids)` | RetreatUnits |
| `dig_units` | `{units, polyline: PackedVector3Array}` | `dig_units_ordered(unit_ids, polyline)` | DigUnits (`line` in the sim spec; only men of rifle/engineer squads) |
| `set_stance` | `{units, stance: String}` | `stance_ordered(unit_ids, stance)` | SetStance; `"auto"`, `"stand"`, `"crouch"` or `"prone"` |
| `throw` | `{units, item: String, target: Vector3}` | `throw_ordered(unit_ids, item, target, false)` | Throw (the sim picks one eligible thrower); item `"frag"`, `"smoke"` or `"satchel"` |
| `throw_each` | `{units, item: String, target: Vector3}` | `throw_ordered(unit_ids, item, target, true)` | ThrowEach (every eligible man throws) |
| `call_in` | `{type: String}` | `call_in_requested(squad_type)` | CallIn |
| `surrender` | `{}` | `surrender_requested()` | Surrender |
| `pause` | `{paused: bool}` | `pause_requested(paused)` | none in lockstep; vs AI the source may stop stepping |
| `quit_to_menu` | `{}` | `quit_to_menu_requested()` | none; match.gd changes scene |

**Order mapping (S1b-soldier-control.md).** For move, attack-move, stop,
retreat and dig the view splits the selection: a squad whose every living
man is selected gets the squad-level intent (formations, small command
log); the selected men of partly selected squads go into one `*_units`
intent. One click can therefore send both `move {squads}` and
`move_units {units}`. `set_stance`, `throw` and `throw_each` always carry
the selected soldier ids (a whole-squad selection sends its member ids);
`barrage` and `regroup` always carry squads.

Notes for the integrator:
- Plain right-click with mortars selected splits the order: mortar squads
  whose range covers the point get `barrage`, the rest of the selection gets
  `move` / `move_units`. To move mortars, use the Move button or M.
- Ctrl+right-click sends `attack_move` / `attack_move_units` for the whole
  selection (mortars included).
- Right-click on a spotted enemy soldier sends an attack-move to his
  position.
- The minimap's right-click uses the same rules as the world (through the
  inverse rotation when the minimap turns with the camera).
- Dig polylines are ground points (`height_at`) clicked by the player and
  already capped to `dig_max_m`; the sim should still validate.
- Throws are only sent when at least one selected, unpinned man carrying
  the item has the target within range; the sim re-checks everything.
- Stance: the view remembers a stance it sent for about 1.5 s (or until the
  state shows it), so pressing a stance key twice quickly still means
  "then Auto" under lockstep delay.
- The view does not throttle intents; the source should drop ids that are
  no longer the local player's (and, per the sim spec, sort, dedupe and cap
  `units` at 120).

## Input map (defaults)

Registered at runtime by `MatchInput.ensure_actions()` (physical keys,
rebindable through InputMap). These are the owner's S1b hotkeys
(`docs/slices/S1b-hud.md`):

| Input | Action |
|---|---|
| Left click | select one soldier (nearest within 14 px; near-ties go to the one closer to the camera); Shift toggles him |
| Double-click | select his squad; Shift adds it |
| Ctrl+double-click (or Cmd) | select every own squad of that type with a man on screen |
| Left drag | box-select own soldiers; Shift adds |
| Right-click | move (split as above); on a spotted enemy: attack-move; mortars in range: barrage |
| Ctrl+right-click | attack-move |
| H / R / L | stop / retreat / regroup (immediate) |
| Y | dig tool: click points, Backspace removes the last, right-click digs |
| B | fire-mission tool (mortars; S5b) |
| M | move tool (left click places, Shift keeps it) |
| G / T / F | frag / smoke / satchel throw tool: range ring, target ring and cursor red out of range; click throws (one man), Shift+click: every eligible man |
| X / C / Z | stand / crouch / prone for the selected men; the active stance again returns to Auto |
| Ctrl+1–9 (or Cmd) | store the exact soldier set; 1–9 recall it (dead men drop out), twice quickly centres the camera |
| Tab | focus the call-in column (Up/Down, Enter calls in, Tab/Esc leaves) |
| Space | jump to the latest event with a position |
| Esc / F10 | cancel the tool, else match menu |
| WASD, arrows, screen edges | pan; Q/E rotate; wheel zoom (10–150 m); middle-drag rotate/tilt |

## HUD v2 (S1b)

Built by `match/hud/` on `MatchView`; layout, colours and fonts follow
`docs/slices/S1b-hud.md` and its mocks. Constants live in
`match/hud/hud_style.gd` (font sizes 13/14, translucency, the accent, the
normal and compact metric sets). `hud_layout.gd` places the pieces and keeps
at least one window pixel per HUD pixel, so 1280x800 lays out natively with
the compact metrics instead of shrinking 14 px text to 11 px. Pieces:
`top_bar.gd` (with the A–E chips), `minimap.gd` (setting `minimap_rotate`,
on by default, turns it with the camera: only the turned map square is
drawn, with no backdrop, and clicks in the box's empty corners reach the
world; off, a north-up rounded square; `minimap_dock` puts it bottom-left), `event_feed.gd` (3 lines,
click-through), `roster.gd` (group tabs, squad cards with per-man pips),
`detail_panel.gd`, `orders_grid.gd` (abilities, orders, stance strip) and
`call_in_panel.gd`. Rings under selected and hovered soldiers are one
MultiMesh batch (`match/selection_rings.gd`, one buffer upload per frame);
detached men's targets, throw range rings and the throw target are drawn by
`ground_overlay.gd`; the hover tooltip and tool hints by `unit_overlay.gd`.

## Net mode (S2)

A state with a `net` key puts the view in net mode (no key: the skirmish
presenter, unchanged). The source keeps sending everything above, plus:

```text
net: {
  ping_ms: float,          # round trip to the relay, -1 unknown
  input_delay: int,        # D in ticks
  delay_ms: int,           # D in ms
  playout: int,            # ticks this client shows a tick after its turn closes
  restamps: int,           # own commands moved to a later turn ("late")
  stalls: int,             # ticks this client waited for a bundle
  state: String,           # "ok" | "unstable" | "waiting" | "paused" | "desync"
                           # (reads "ok" once the match is over)
  paused_by: String,       # player name while "paused"
  paused_slot: int,        # opt; slot that paused (else matched by name)
  waiting_for: String,     # player name while "waiting"
  waiting_slot: int,       # opt; slot waited for (own slot = "Reconnecting")
  waiting_s_left: float,   # seconds until the relay's Surrender
  desync_tick: int,        # -1 none
  desync_dump: String,     # absolute folder of the dump, "" none
  disconnect_reason: String, # reason code; non-empty = link gone for good
},
pending_orders: [          # own orders sent but not executed yet
  { pos: Vector3, kind: String },  # kind: move | attack_move | barrage |
],                         # throw | dig (or the intent name; *_units and
                           # throw_each map onto these)
players[i].name            # lobby names
```

What the view does with it:

- **Readout** (`hud/net_readout.gd`): status dot (green ok, amber
  unstable / waiting / paused, red desync), ping, D in ticks and ms,
  restamps, stalls, in the top bar between the enemy tickets and the clock
  (`top_bar.free_span()`); words at 1600, letters at 1280. Tooltip explains.
- **Pending orders**: a turning dashed ring at the target from click time
  (positional intents) until the order leaves `pending_orders`; a click
  marker the state never confirms lasts 0.5 s.
- **Panels** (`hud/net_overlay.gd`): red desync banner (tick, dump path,
  Copy path, Leave); "Waiting for <name> (Ns)" with a draining bar, or
  "Reconnecting" for the own slot; "Paused by <name>" with Resume.
- **Disconnect screen** (`hud/net_disconnect_screen.gd`): the reason, Back
  to menu.
- While `paused`, `waiting` or `desync`, and under the disconnect screen,
  `input_blocked()` is true: no selection or orders (the camera still
  moves). `unstable` blocks nothing.
- The pause menu does not pause on open; it offers "Pause match (both
  players)" / "Resume match". There are no sim speed controls in the HUD;
  net mode runs at 1x.

The FrontlineNet lobby API and the command-line flags are in
`godot/docs/net_api.md`.

## Vehicles (S3)

Frozen contract for the S3 presenter (`docs/slices/S3-armour.md`
"Snapshot contract", "Presenter", "Commands"). Build on
`mock_match_source.gd` against this; `FrontlineMatch` fills the same keys
at integration (S3 step 8). The Rust side is frozen too:
`VehicleSnapshot`, `ShellSnapshot`, `ArmourEvent` and the `PlayerSnapshot`
fields `vehicles_counted`, `vehicle_limit`, `unlock_tick` in
`crates/sim/src/snapshot.rs` (empty lists until the sim spawns vehicles).
Everything in the conventions above applies: Godot space, metres,
radians, sorted id lists, unknown keys ignored, *opt* keys may be absent.

**Model.** A vehicle is an entity, like a soldier, and a squad of one.
Vehicle ids share the soldier id space (a vehicle id is never a soldier
id). Vehicles are sent only in `vehicles`, never in `squads` or
`soldiers`; a vehicle's squad id is only for the roster
order. A knocked-out vehicle keeps its id as a wreck (`owner` −1).
Enemy vehicles are sent only while the local player sees them; wrecks
always. Own vehicles always.

### State keys

```text
rules: {
  ...                              # S1/S1b keys unchanged
  vehicles: {
    unlock_s: float,               # match time vehicles can be called in from
                                   # (240.0; 0.0 with the test override)
    limit: int,                    # most vehicles alive per player (4)
  },
  dig_profiles: {                  # replaces nothing: dig_max_m stays the trench's
    trench:   { max_m: float, depth_m: float, width_m: float },   # 60, 1.6, 1.6
    at_ditch: { max_m: float, depth_m: float, width_m: float,     # 30, 2.3, 3.4
                engineers_only: bool },                           # true
  },
  vehicle_types: {                 # every vehicle type of the match, by type key
    "<type>": {                    # type key = def id without the mod prefix,
                                   # e.g. "ger_pz4_h", "usa_m4a3_75"
      id: String,                  # "ww2:ger_pz4_h"
      name: String,                # "Panzer IV Ausf. H"
      short: String,               # "Panzer IV" (cards, feed, markers)
      faction: String,             # "germany" | "usa"
      role: String,                # "tank"
      cost: int, pop: int, recall_s: float,    # 640, 12, 120.0 (S3b: the
                                               # re-call cooldown; was call_in_s)
      speed: float,                # top speed, m/s (6.0)
      sight_m: float,              # 175.0
      # Box sizes for meshes, footprints, picking, rings and the card's
      # silhouette (metres; the hull origin is on the ground under the
      # hull's centre; "forward" is the hull's front):
      hull_length: float, hull_width: float, hull_height: float,   # 5.8 2.8 1.36
      hull_ground: float,          # belly height above the ground (0.40)
      turret_x: float,             # pivot forward of the hull centre (0.20)
      turret_base: float,          # pivot height above the ground (1.76)
      turret_length: float, turret_width: float, turret_height: float, # 2.3 1.9 0.9
      turret_offset_x: float,      # turret box centre forward of the pivot (0.0)
      gun_length: float,           # muzzle forward of the pivot (4.5)
      gun_height: float,           # muzzle height above the pivot (0.45)
      seats: PackedStringArray,    # roles it has, pip order D G C L R:
                                   # "driver" "gunner" "commander" "loader" "radio_mg"
      ammo: [                      # main-gun rounds, penetrator first
        { kind: String,            # "ap" (AP/APHE) | "he"
          name: String,            # "7.5 cm PzGr 39"
          max: int },              # 30
      ],
    },
  },
},
players: [
  {
    ...                            # S1 keys unchanged; `pop` now counts living
                                   # soldiers plus each living vehicle's `pop`
    vehicles: int,                 # alive (the call-in "2/4" badge; S3b: no queue)
    vehicle_limit: int,            # 4
  }, ...
],
vehicles: [                        # own + seen enemy + every wreck, ascending id
  {
    id: int,                       # entity id
    squad: int,                    # its squad id; -1 for a wreck
    owner: int,                    # player; -1 = wreck
    type: String,                  # key into rules.vehicle_types
    name: String,                  # display name ("Panzer IV Ausf. H")
    pos: Vector3,                  # hull origin on the ground, interpolated as
                                   # drawn (picking and markers project it)
    yaw: float,                    # hull: rotation about +Y of a model facing −Z
                                   # (the soldiers' convention, present.rs
                                   # model_yaw)
    pitch: float,                  # hull, + = nose up
    roll: float,                   # hull, + = left side up
    turret_yaw: float,             # relative to the hull, + = counter-clockwise
                                   # seen from above
    gun_pitch: float,              # + = muzzle up
    speed: float,                  # m/s along the hull, negative reversing
    flags: int,                    # bitmask, see below; enemies get only
                                   # BURNING, WRECK, KNOCKED_OUT (S3c: and
                                   # ABANDONED, BOARDING)
    state: String,                 # the badge: first that applies of "wreck",
                                   # S3c "abandoned",
                                   # "burning", "bogged", "immobilised",
                                   # "repairing", "crossing", "reversing",
                                   # "moving", "idle". Enemies: "wreck",
                                   # "abandoned", "burning", "moving"
                                   # (|speed| > 1) or "idle"
    abandoned: bool,               # S3c, every vehicle: the crew bailed out.
                                   # An abandoned vehicle has owner -1 and
                                   # squad -1 (nobody's unit, anyone may
                                   # inspect it, like a wreck) and also:
    last_owner: int,               #   S3c, abandoned only: its last owner
    abandon_cause: String,         #   why: "ammo_fire" | "engine_fire" |
                                   #   "crew" | "useless" (gun and mobility
                                   #   out) | "under_fire" | "order"
    crew_squad: int,               #   its crew squad's id, -1 none (gone)
    inside: int,                   #   crewmen still climbing out
    boarding: bool,                #   a squad is climbing in (re-crew)
    parts_out: PackedStringArray,  #   components at 0 HP ("gun", "track_l"
                                   #   ...): the hover text names them
    # Own vehicles only (absent for enemies and wrecks):
    crew: PackedStringArray,       # 5 entries, D G C L R: "ok" | "wounded"
                                   # | "dead" (or left the seat) | "swapping"
                                   # (a crewman moving in, 4 s) | "none" (the
                                   # vehicle has no such seat)
    components: {                  # fraction of full HP, 0..1
      engine: float, track_l: float, track_r: float,
      gun: float, ring: float, ammo_rack: float,
      transmission: float,         # S3b: gearbox and final drives
    },
    repairing: String,             # "" | "engine" | "track_l" | "track_r" | "ring"
                                   # | "transmission"
    ammo: { ap: int, he: int,      # main-gun rounds left (S8a Fix: absent with no
                                   # main gun, half-tracks); S3b: apcr only on a
            apcr: int,             # gun that carries a special round (Panzer IV);
            mg: int,               # S3b Fix: MG rounds left in stowage (if limited)
            mg_full: int },        # S3c: the full stowage (with mg); the vehicle
                                   #   card shows "MG 2,950", amber under a
                                   #   quarter, red "MG dry" at 0
    loaded: String,                # round in the breech: "ap" | "he" | "apcr" | ""
    ammo_choice: String,           # "auto" | "ap" | "he" | "apcr" (SetAmmo)
    reload: float,                 # 0..1, 1 = ready to fire
    target: int,                   # enemy vehicle the gun is on, -1 none
    followers: int,                # soldiers escorting it (FollowUnits)
    suppress_line: PackedVector3Array, # opt, own, S3b: the line its MGs (and HE
                                   # every combat.suppress.tank_he_s) fire at
  }, ...
],
shells: [                          # main-gun shells and AT rockets in flight that
                                   # the local player may see: fired by his units,
                                   # by a unit he sees, or within 40 m of his men
  {
    id: int,                       # projectile slot; stable while it flies
    kind: String,                  # "ap" | "he" | "heat" (S3 rockets are heat)
    owner: int,                    # firing player
    shooter: int,                  # vehicle or soldier id that fired it
    pos: Vector3,
    vel: Vector3,                  # m/s, Godot axes
  }, ...
],
armour_events: [                   # new since last frame, in sim order. Own
                                   # vehicles' events, and impacts the local
                                   # player could see (the target vehicle is seen,
                                   # or within 40 m of his men)
  {
    kind: String,                  # "fired" | "ricochet" | "no_pen" | "pen"
                                   # | "component_hit" | "crew_hit" | "disabled"
                                   # | "knocked_out" | "bogged"; S3c
                                   # "abandoned" | "crew_out" | "recrewed"
    vehicle: int,                  # the vehicle it happened to (or that fired)
    type: String,                  # its vehicle type key
    owner: int,                    # its owner at the time (a knocked-out
                                   # vehicle's last owner, not -1)
    attacker: int,                 # firing player, -1 none (fire, bogging)
    component: String,             # per kind:
                                   #   fired: the round, "ap" | "he" | "apcr"
                                   #   component_hit (damaged, still works) and
                                   #   disabled (reached 0 HP): "engine" |
                                   #   "track_l" | "track_r" | "gun" | "ring" |
                                   #   "ammo_rack" | "transmission" (S3b)
                                   #   crew_hit: the seat role, "driver" ...
                                   #   knocked_out: the cause, "crew" |
                                   #   "ammo_rack" | "fire" | "ammo_fire" (S3b:
                                   #   a rack fire burned out; "crew": every
                                   #   seat empty in a vehicle that cannot bail
                                   #   out; S3c: an abandoned vehicle that
                                   #   burns out or blows up is knocked out too)
                                   #   abandoned (S3c): the trigger, as
                                   #   vehicles[].abandon_cause
                                   #   crew_out (S3c): the seat of the man who
                                   #   got out ("driver" ...); pos: where he stands
                                   #   recrewed (S3c): "own" | "captured";
                                   #   owner: the new owner, attacker: the last
                                   #   others: ""
    zone: String,                  # ricochet / no_pen / pen: the plate's zone
                                   # ("hull_front" ... "turret_top"), else ""
    ammo: String,                  # the round's kind: "ap" | "he" | "heat", ""
    pos: Vector3,                  # impact point; muzzle for "fired"
    seq: int,                      # running count of armour events in the
                                   # match (later direct-control prediction)
    time_s: float,
  }, ...
],
callin: {
  catalog: [ {                     # S1 keys, plus:
    id: String,                    # squad type id ("ww2:ger_pz4_h_squad");
                                   # send it back in call_in
    type: String,                  # S1 roles, plus "at" (AT team) and "vehicle"
    pop: int,                      # men, or the vehicle's pop (12)
    vehicle: String,               # opt; vehicles: the vehicle type key
    locked_s: float,               # opt; vehicles: seconds until unlock (the
                                   # tile's lock and countdown); 0 or absent =
                                   # unlocked
  }, ... ],
  vehicles: int,                   # local player's vehicles alive (S3b: no queue)
  vehicle_limit: int,              # tiles of vehicles at the limit grey out
},
```

`squads[].type` can now be `"at"` (an AT team: a launcher gunner and a
rifleman). Squads never have type `"vehicle"`. S3c: `"crew"` is a tank
crew that bailed out (pistols and SMGs; never bought). A crew squad whose
tank still stands abandoned has `crew_of: int` (the vehicle id) and
`crew_of_name: String` ("Panzer IV"); a squad walking to re-crew a vehicle
has `order` `"recrew"` and `recrew: int` (the vehicle).

**Flag bits** (`vehicles[].flags`; same values as `vehicle_flags` in
`snapshot.rs`):

| Bit | Name | Meaning |
|---|---|---|
| 1 | MOVING | faster than 1 m/s either way |
| 2 | REVERSING | driving backwards |
| 4 | BOGGED | stuck after a trench crossing (20–40 s) or on blocked ground |
| 8 | IMMOBILISED | engine or a track destroyed, or no driver |
| 16 | BURNING | engine fire (presenter: flames, at most 8 drawn) |
| 32 | REPAIRING | crew repairing in the field |
| 64 | CROSSING | a track over a trench (speed capped at 1 m/s) |
| 128 | WRECK | knocked out: owner −1, blocks rounds and vehicles |
| 256 | KNOCKED_OUT | a wreck less than 60 s old: smoke it (nearest 16) |
| 512 | ABANDONED | S3c: the crew bailed out; owner −1, `last_owner` set; can be re-crewed |
| 1024 | BOARDING | S3c: a squad is climbing into it |

**Feed** (`events`), new kinds the bridge derives (the feed shows only
main-gun and rocket matters):

| `kind` | When | Example `text` |
|---|---|---|
| `vehicle_arrived` | an own vehicle call-in (S3b: spawns at once) | "Panzer IV deployed at base." |
| `vehicle_knocked_out` | any seen vehicle knocked out (`player` = its owner) | "Panzer IV knocked out." / "Enemy Sherman knocked out." |
| `vehicle_damaged` | an own component reaches 0 HP | "Panzer IV: track destroyed." |
| `vehicle_bogged` | an own vehicle bogs | "Sherman bogged in trench." |
| `vehicle_abandoned` | S3c: any seen vehicle abandoned (`player` = its owner) | "Panzer IV abandoned (ammo fire): 4 of 5 bailing out." / "Enemy Sherman abandoned." |
| `vehicle_recrewed` | S3c: an abandoned vehicle manned by its own side | "Panzer IV re-crewed." / "Enemy Sherman re-crewed." |
| `vehicle_captured` | S3c: a capture involving the local player | "Captured an enemy Sherman." / "Our Panzer IV was captured by the enemy." |
| `recrew_refused` | S3c: the sim refused a `recrew` | "Not re-crewed: vehicle limit reached (4)." |

**Effects** (`effects`), new kinds:

```text
{ kind: "cannon", pos: Vector3, dir: Vector3, round: "ap" | "he", type: String },
                                   # main gun fired: muzzle flash, dust, sound
{ kind: "rocket_launch", pos: Vector3, dir: Vector3 },  # AT launcher fired
{ kind: "armour_hit", result: "ricochet" | "no_pen" | "pen", pos: Vector3 },
                                   # sparks (ricochet, no_pen) or flash (pen)
{ kind: "cook_off", pos: Vector3 }, # ammo rack detonation
{ kind: "explosion", pos: Vector3, size: "tank_he" | "rocket" },
                                   # new sizes for tank HE and HEAT bursts
```

Engine sounds come from `vehicles` (the 4 nearest moving ones), not from
effects.

### Intents (S3)

Own vehicle ids go in `units`, next to soldier ids, for the S1b `*_units`
intents and the new ones below. The sim sorts, deduplicates, drops ids
that are not the sender's living units, and caps the list at 120 (as for
soldiers). Squad-level intents (`move {squads}` ...) never carry vehicles.

| `kind` | `data` | Sim command | Notes |
|---|---|---|---|
| `select` | `{squads, units}` | none | `units` may hold own vehicle ids |
| `move_units`, `attack_move_units`, `stop_units`, `retreat_units` | as S1b | MoveUnits, AttackMoveUnits, StopUnits, RetreatUnits | accept vehicle ids; attack-moving vehicles halt to fire the main gun. A Move whose target is behind a vehicle and within 40 m reverses |
| `reverse_units` | `{units, target: Vector3}` | ReverseUnits `{units, target}` | vehicles back up to `target`, front kept toward the enemy; soldiers in the set get a MoveUnits |
| `face_units` | `{units, target: Vector3}` | FaceUnits `{units, toward}` | vehicles turn the hull front to the point and hold; soldiers dropped |
| `attack_unit` | `{units, target: int}` | AttackUnit `{units, target}` | `target`: a seen enemy vehicle's id. Own vehicles and launcher gunners in `units` focus it while it is seen; vehicles without range or line of fire close in. Other men ignore it |
| `set_ammo` | `{units, ammo: String}` | SetAmmo `{units, ammo}` | `"auto"`, `"ap"`, `"he"` or (S3b) `"apcr"` (a gun without it fires its standard round); vehicles only |
| `repair_units` | `{units}` | RepairUnits `{units}` | crews repair the worst repairable component in place (no move or main gun meanwhile) |
| `follow_units` | `{units, vehicle: int}` | FollowUnits `{units, vehicle}` | soldiers escort a friendly vehicle (single file behind it); vehicle ids in `units` are dropped |
| `suppress_units` | `{units, polyline: PackedVector3Array}` or `{units, target: Vector3}` | SuppressUnits `{units, line}` (S3b) | soldiers and vehicles hold and fire at the point or line (its first and last points; the sim pulls a line longer than `rules.suppress_line_max_m` in) without a spotted target; mortar men are dropped |
| `dig`, `dig_units` | as S1b, plus `profile: String` *opt* | Dig `{squad, line, profile}`, DigUnits `{units, line, profile}` | `"trench"` (default) or `"at_ditch"` (engineers only, at most `rules.dig_profiles.at_ditch.max_m`) |
| `recrew` | `{squads, vehicle: int}` | Recrew `{squad, vehicle}` per squad (S3c) | the squads walk to the abandoned vehicle and man it; refused (feed `recrew_refused`) for AT teams, too few men, at the vehicle limit or pop cap, or an enemy's vehicle with capture off |
| `abandon` | `{units}` | Abandon `{squad}` per own vehicle (S3c) | the crews bail out (vehicles with a crew squad) |
| `call_in` | `{type, id}` | CallIn `{squad_type}` | `id` (catalog `id`) wins over `type`. S3b: bought and spawned at once, or refused (before the unlock, at the vehicle limit, on the type's cooldown, short of manpower, over the pop cap); the result comes back as a feed event (below) |

Sim `CommandKind` shapes (added by the S3 sim stages; `PROTOCOL_VERSION`
becomes 3 with them): `ReverseUnits { units: Vec<SimId>, target: FxVec2 }`,
`FaceUnits { units: Vec<SimId>, toward: FxVec2 }`, `AttackUnit { units:
Vec<SimId>, target: SimId }`, `SetAmmo { units: Vec<SimId>, ammo:
AmmoChoice }` (`Auto`, `Ap`, `He`), `RepairUnits { units: Vec<SimId> }`,
`FollowUnits { units: Vec<SimId>, vehicle: SimId }`; `Dig` and `DigUnits`
gain `profile: DigProfile` (`Trench`, `AtDitch`;
`frontline_content::DigProfile`). `Dig`, `Barrage`, `Regroup` and
`SetStance` are rejected for vehicle squads.

### Input (S3 additions)

S1b keys unchanged. Mixed selections split: soldiers get their `*_units`
orders, vehicles theirs; vehicle-only keys affect only vehicles.

| Input | Action |
|---|---|
| Right-click on a seen enemy vehicle | `attack_unit` (vehicles and AT gunners); other men attack-move to it |
| Right-click on an own vehicle with infantry selected | `follow_units` (the soldiers) |
| Alt+right-click | `reverse_units` |
| O | face tool: click a point, `face_units` |
| P | S3b suppress tool (soldiers and vehicles): click a point or drag a line, `suppress_units`; Shift keeps the tool |
| N | next ammo for the selected vehicles: Auto → AP → (APCR, S3b, on a gun that carries it) → HE (`set_ammo`, from the first selected vehicle's `ammo_choice`) |
| J | `repair_units` |
| K | S3c Crew tool (infantry selected): click an abandoned vehicle, `recrew` |
| Right-click on an abandoned vehicle with a tank crew selected | S3c `recrew`; other selections treat it as the ground |
| Bail out button (vehicle grid, no key) | S3c: arms for 2 s ("Again?"); the second press sends `abandon` |

Picking: a vehicle is hit inside its projected footprint (`hull_length` ×
`hull_width` at `pos` and `yaw`) or within 14 px of its centre; selection
rings are sized to the footprint. `1`–`4` stay control groups.

### Presenter pieces (for orientation; the spec has the details)

World marker per vehicle (type glyph, burning or bogged pip); a vehicle
card in the roster (name, silhouette from `vehicle_types` box sizes, crew
pips from `crew`, component strip, `AP 24 · HE 18` with `loaded` and
`ammo_choice` marked, state badge); detail panel for a focused vehicle (no
penetration hints about enemies); orders grid for vehicles (Move,
Attack-move, Stop, Retreat, Reverse, Face, Repair, ammo toggle); call-in
tiles with cost, pop, a lock and countdown (`locked_s`) and the `2/4`
badge; feed lines; hit markers ("Ricochet", "No pen", "Penetrated", "Crew
hit") above observed `armour_events` for 1.5 s, at most 6; minimap
rectangles for vehicles and dim crosses for wrecks.

### S3 presenter as built (on the mock)

Built against `mock_match_source.gd` + `mock_vehicles.gd` (MockVehicles:
tanks in every state, shells, armour events, AT teams, escorts, vehicle
call-ins with the unlock and the limit). The bridge only has to fill the
keys above; nothing below reads the sim.

- **Meshes** (`crates/godot-bridge/src/meshgen.rs`): `vehicle_hull_mesh`
  (tracks with sloped runs, lower hull, fenders, superstructure with a
  glacis, engine deck, exhausts: 184 triangles), `vehicle_turret_mesh`
  (sloped-sided turret, mantlet, cupola: 72), `vehicle_gun_mesh` (barrel
  and muzzle brake: 64), all built from the `vehicle_types` box sizes.
  `compose_vehicle` composes hull (yaw, then pitch, then roll) × turret
  yaw × gun pitch; `vehicle_buffers` packs MultiMesh buffers. GDScript
  reaches them through the static class **`FrontlineMeshes`**:
  `vehicle_part(dims: Dictionary, part: "hull"|"turret"|"gun") -> ArrayMesh`
  and `vehicle_buffers(dims, poses: PackedFloat32Array, cap) -> [hull,
  turret, gun]` (12 floats per pose: x y z, yaw, pitch, roll, turret yaw,
  gun pitch, custom r g b a; 16 floats per instance out, padded to `cap`).
  The trunnion (gun pitch axis) sits 0.42 × turret_length ahead of the
  pivot at `gun_height`. `dims` may carry an optional `glacis_deg`; the
  presenter uses 10° for Germany and 47° for the USA when absent.
- **`match/vehicle_view.gd` (VehicleView)**, made by `match.gd` in both
  scenes: one MultiMesh per (type, part), CAP 48 instances each, one
  buffer upload per batch per frame. Turret yaw follows the state by the
  shortest arc (`lerp_angle`), pitch, roll and gun pitch are smoothed.
  Custom data: the faction paint mixed 42 % toward the side colour; wrecks
  charred at 55 % brightness (`shaders/vehicle_parts.gdshader`). Without
  the GDExtension it falls back to boxes.
- **`match/vehicle_fx.gd` (VehicleFx)**: fixed pools, nothing created
  during play. Muzzle flash + light + ground dust ring (`cannon`; 8 rigs),
  backblast (`rocket_launch`), sparks + puff (`armour_hit` ricochet,
  no_pen; 8), flash (`pen`; 6), cook-off fireball, flame jet, debris and a
  smoke column (3), tracers (AP pale, HE orange) and rocket trails (a pale
  trail and a bright head) in ONE MultiMesh, fires on the nearest 8
  BURNING vehicles, smoke over the nearest 16 KNOCKED_OUT wrecks, dust
  behind the nearest 8 vehicles faster than 2 m/s. Engine loops: the 4
  nearest moving vehicles keep a voice slot in
  `AudioManager.set_engine_loops` (pitch by speed).
- **Sounds** (`audio/sfx/`, synthesised by `tools/gen_s3_sfx.py`, CC0,
  each with a `.license.toml`): `cannon` (10 KB), `engine_loop` (15 KB,
  seamless 2 s), `ap_impact` (9 KB), `rocket_launch` (24 KB). MatchView
  plays them for `cannon`, `rocket_launch`, `armour_hit` (ricochets 4 dB
  quieter) and `cook_off` (the explosion, louder). All audio: 328 KB.
- **Selection** holds own vehicle ids next to soldier ids (`select` sends
  them in `units`). `MatchView.veh_pick_*` index the live vehicles each
  frame; `selection.pick()` takes a soldier under the cursor first, else a
  vehicle inside its projected footprint (`vehicle_screen_footprint`) or
  within 14 px of its centre. Box-select takes own vehicles by centre;
  double-click keeps a vehicle, Ctrl+double-click takes that type on
  screen. Rings: `GroundOverlay/VehicleRings`, a second SelectionRings
  batch of thin ellipses stretched to the hull + 0.9 m and turned with it.
- **Orders**: move / attack-move / stop / retreat add the selected
  vehicles to the `*_units` intent; a squad-level intent never carries
  vehicles. RMB on a seen enemy vehicle: `attack_unit` for own vehicles
  and the men of selected AT squads, `attack_move` to it for the rest.
  RMB on an own vehicle with soldiers selected: `follow_units` (soldier
  ids only). Alt+RMB: `reverse_units` for the vehicles, a move for the
  soldiers. O arms the face tool (`face_units`), N cycles the first
  selected vehicle's choice (Auto → AP → HE; pressing twice before the
  state catches up still advances), J sends `repair_units`.
- **HUD**: the roster gets a vehicle card per own vehicle in squad-id
  order (silhouette from the box sizes, name, badge; crew pips, the
  5-part component strip, `AP n · HE n` with the loaded round in the
  accent and a forced choice boxed; about 250 px wide). The detail panel's
  `vehicle` mode shows crew letters, six component bars (the one being
  repaired outlined), ammo, the choice chip and the reload. With only
  vehicles selected the orders grid switches to Reverse / Face / Repair,
  Move / Attack / Stop / Retreat and an Auto | AP | HE strip. Call-in
  vehicle tiles: cost and pop, a lock and m:ss countdown from `locked_s`,
  an `n/limit` badge (red at the limit), reasons `locked` and `limit`; the
  intent carries the catalog `id`. **Layout change:** when one column of
  tiles would rise above 57.5 % of the HUD height (the S1b 1280 rule, "no
  higher than y460"), the tiles go two per row (a 244 px column); with six
  catalog entries that is always the case. World markers (tank glyph,
  flame / mud / spanner / immobilised pips, `+n` escorts), hit markers
  (stacked, at most 6, 1.5 s), feed stripes for the four new kinds (an
  enemy knocked out is own-coloured), minimap rectangles and wreck
  crosses, escort links (dashed, own colour) and the face tool's arrows.

**Contract notes found while building**, as settled at integration (the
bridge, `crates/godot-bridge/README.md` "S3: vehicles in the bridge"):

- (1) The bridge sends `squads[].follow` (squad-level escorts, `order`
  `"follow"`) and `soldiers[].follow` with `order` `"follow"` (detached
  men), so escort links are drawn in the real game too.
- (2) Catalog entries carry `pop` (a vehicle's pop for vehicle entries)
  and `id`; vehicle catalog entries also `vehicle` and `short`. (S3b: the
  queue is gone.)
- (3) Unchanged: `immobilised` still outranks `repairing` in `state`.
- (4) `vehicle_types` carries `glacis_deg` (the hull front plate's slope
  from the def: Panzer IV 10, Sherman 47).
- Enemy vehicles' own-only keys are absent, as above; in the Rust frame
  they are zeroed, with `repairing` and `target` at their "none" values.
- `cannon` effects carry `round` and `type`, `rocket_launch` `dir`,
  `armour_hit` `result`.
- Staged scenes (S3 "Done when" 7): with `start_match({scene: NAME})` the
  state adds `scene: {name, ready, ready_tick, max_ticks}` and
  `get_map_info()` adds `scene` and `scene_focus` (match/autoplay_scene.gd).

The notes as written while building on the mock:

1. Escort links need to know which vehicle a squad or man follows; the
   contract only has `vehicles[].followers` (a count). The mock sends an
   *opt* `squads[].follow` (vehicle id, own squads with a squad-level
   follow, `order` "follow") and `soldiers[].follow` (detached men,
   `order` "follow"); the presenter draws links only when present.
   Proposed for the bridge.
2. (Obsolete since S3b: there is no queue.) Call-in queue entries: the panel counts a queued entry's pop as
   `pop` if present, else `men`. The bridge should send `pop` for vehicle
   entries (the mock does), or a queued tank counts 0 toward the pop cap
   check in the HUD (the sim still rejects it).
3. Badge precedence puts `immobilised` before `repairing`, so a tank
   fixing a broken track reads "Immobilised"; the spanner pip, the
   outlined strip segment and the detail bar show the repair.
4. `vehicle_types` has no armour slope for the look; the presenter takes
   `glacis_deg` from the dims when present, else by faction.

## Autoplay

S3: `--autoplay` also buys tanks once they unlock (and an AT team) and
drives them with every vehicle intent (`match/autoplay_s3.gd`), and fails
(exit 3) unless one of our vehicles and one enemy vehicle were drawn.
`--autoplay_scene armour_<name>` runs a staged S3 scene and saves
`s3_<name>.png` (`match/autoplay_scene.gd`; exit 1 if its condition never
held). `--test_unlock` and `--start_manpower N` are for scripted runs (in
a net match on both instances).

`godot/tools/autoplay.sh <Godot> [minutes] [speed]` boots to the menu,
starts a skirmish and lets `match/autoplay.gd` play the local side through
the same intents (call-ins, moves, a trench in front of B, a mortar
barrage), saving `s1_autoplay_*.png` to `docs/slices/screens/`, then
surrenders at the time limit and saves the end screen. The S1b steps
(`match/autoplay_s1b.gd`) push mouse and key events into the viewport:
click one man, double-click his squad, box two squads (Ctrl+1), send one
man forward, Z prone, recall group 1, T smoke, the HUD at 1600x900 and
1280x800, and G frag into a trench holding a spotted enemy, saving
`s1b_*.png`.

## Running the mock

- `Godot --path godot res://match/match_mock.tscn` runs the HUD on the mock source; `res://match/match.tscn` is the real match (FrontlineMatch).
- `godot/tools/capture_screens.sh <Godot>` saves the menu and HUD screenshots
  to `docs/slices/screens/` (S1b: `s1b_hud_mock_1600.png`,
  `s1b_hud_mock_1280.png`, `s1b_throw_tool_mock.png`).
- `Godot --headless --path godot -s res://tools/test_match_view.gd` runs the
  presenter input and layout tests on the mock (S3: vehicle flags,
  buffers, picking, rings, orders, keys, card, ammo strip, lock and limit,
  hit markers, the 16-vehicle CPU cost and the 1280x800 roster fit).
- `godot/tools/capture_s3_screens.sh <Godot>` saves `s3_mock_*.png`
  (vehicles, card, fit, locked call-in, escort; 1600x900 and 1280x800).
  The staging (`match.gd` `_stage_s3`, `--capture_setup s3_vehicles` ...)
  fires the tanks shortly before the shot (`Session.capture_left_s()`).
- `python3 -I godot/tools/gen_s3_sfx.py` regenerates the four S3 sounds
  and their licence manifests (needs ffmpeg).
- `Godot --headless --path godot -s res://tools/test_net_ui.gd` runs the S2
  presenter tests on MockNet (menu, lobby, net HUD); and
  `godot/tools/capture_net_screens.sh <Godot>` saves `s2_mock_*.png`.
- `Godot --headless --path godot -s res://tools/check_project.gd` loads every
  script and scene and runs the menu and the match for a few frames.

## Call-in without a queue (S3b, owner 2026-11-01)

"Remove the queue, but keep the timer of the specific unit after it was
called. Like in GOH." A call-in now buys at once: the sim spends the
manpower and spawns the squad in the base in the same tick (spread over
seven base spots, the least crowded first), and that squad type's re-call
cooldown starts (`recall_cooldown_s` in its unit or vehicle def). There is
no queue and no delivery time anywhere in the state.

- `callin.catalog[]` drops `delay_s` and gains `recall_s`; `cooldown_s` and
  `cooldown_total_s` are sent while the cooldown runs. `callin.queue` and
  `callin.queue_max` are gone.
- `players[].pop` is the sim's pop (soldiers alive and each vehicle's
  pop); nothing is queued to add.
- `vehicle_types[].call_in_s` is now `recall_s`.
- Feed (`events`), from the sim's per-tick call-in results (bridge
  `EventTracker::call_ins`, local player only):

  | `kind` | When | Example `text` |
  |---|---|---|
  | `squad_arrived` | own squad bought | "Rifle Squad deployed at base." (the def's name) |
  | `vehicle_arrived` | own vehicle bought | "Panzer IV deployed at base." |
  | `call_in_refused` | own call-in refused by the sim | "Rifle Squad not called in: ready again in 12 s." |

  Refusal reasons, in the sim's check order: "not in your roster",
  "vehicles unlock in m:ss", "vehicle limit reached (4)", "ready again in
  N s", "need N more manpower", "pop cap reached (120)". The call-in panel
  says the same for a tile it will not send (`refusal_text`), so a click
  on a blocked tile also gets a feed line.
- Keys: Tab focuses the call-in column, then 1–9 buy that tile.

## S4: cards, members and capabilities

S4 (`docs/slices/S4-variants.md`) gives every unit card its men by
`[[members]]` slot, tags and capabilities. The presenter keeps the S1-S3c
`type` word (derived from the card's tags: crew, vehicle, at, mortar,
engineer, mg, line -> `rifle`), so older code paths keep working; new code
reads the keys below. Two cards may share a `type` (a full and an
understrength rifle squad): **the call-in catalog and the `call_in` intent
are keyed by the card `id`**.

```text
callin.catalog[]: {               # S3b keys, plus (S4):
  id: String,                     # card id ("ww2:ger_grenadiergruppe"); the
                                  # call_in intent sends it (it wins over type)
  short: String,                  # tile name, every card ("Grenadiere")
  men: int,                       # 10 (0 for vehicles)
  tags: PackedStringArray,        # ["line", "understrength"]
  understrength: bool,            # the tile's half chevron
  gloss: String,                  # tooltip: "Grenadiergruppe — rifle squad,
                                  # 10 men: 1 MP 40, 1 MG 42, 1 P 38, 7 Kar98k;
                                  # 2 Panzerfaust 60" (S4-13 (A))
}
squads[]: {                       # S1-S3c keys, plus (S4):
  tags: PackedStringArray,
  capabilities: PackedStringArray,# "dig_basic" | "dig_full" | "barrage" |
                                  # "recrew" | "vehicle_crew" | "at_hunt";
                                  # the orders grid follows them (below)
  understrength: bool,
  slot_names: PackedStringArray,  # own soldier squads: the role word of each
                                  # formation slot in spawn order ("Leader",
                                  # "MG gunner", ...), for fallen men's rows
}
soldiers[] (own): {               # S1b keys, plus (S4):
  name: String,                   # his member slot as a role word ("MG gunner")
  weapon: String,                 # the weapon he carries now ("MG 42"); a man
                                  # who picked up a crew weapon shows it
  crew_weapon: bool,              # he carries a crew weapon (MG, mortar,
                                  # launcher): the squad card's accent diamond
  items: { frag, smoke, satchel, panzerfaust: int },
}
```

- **Orders by capability.** Dig needs `dig_basic` (the AT-ditch profile
  `dig_full`), Barrage `barrage`, Crew K `recrew` or `vehicle_crew`. A
  state without `capabilities` (the S1-S3c mock) falls back to the type
  lists (`MatchView.DIGGERS`, `MORTARS`).
- **Panzerfaust.** With Panzerfausts and no satchel in the selection, the
  ability row's third slot is "Pzfaust" (count badge): it arms a tool;
  clicking a seen enemy vehicle sends `attack_unit` (the men of the squads
  that carry one) and `attack_move` to it. Right-clicking an enemy vehicle
  does the same for those squads. The men also fire on their own at
  vehicles within the weapon's reach (60 m still, 40 m moving).
- **Call-in tiles.** One tile per card; a soldier tile shows its men in a
  disc on the title line, its re-call cooldown as a ring round the disc
  (vehicle tiles keep the fill), and an understrength card a half-filled
  chevron. Refusal words gain "N alive (max)" (`max_alive`) and "<group>
  limit" (a limit group); "vehicles unlock in" is now "unlocks in m:ss"
  (cards may have their own `unlock_s`).
- **Feed.** `crew_weapon_picked_up`: "MG 42 picked up by MG assistant."
  (own squads; `pos` where he stands).
- **Mock.** `MockMatchSource` has two cards for rifle and MG (an
  understrength rifle squad, a heavy MG squad), keys its cooldowns by card
  id, and sends `capabilities` per squad.

## S5a: counted rounds, fire modes, ammunition spending

S5a (`docs/slices/S5a-ammo.md`) counts every round a man carries and
prices the base refill. The presenter shows rounds per man and per crew
weapon, a LOW / DRY marker, held fire and the manpower spent on
ammunition. Keys are own-only unless marked; a state without them (older
builds, legacy content) shows nothing new.

```text
soldiers[] (own): {               # S4 keys, plus (S5a):
  active_weapon: String,          # the weapon in hand ("Kar98k"; his slot's
                                  # sidearm while his main weapon is dry: "P 38")
  rounds: int,                    # rounds he holds for it (pouches and
                                  # magazine; -1: uncounted, e.g. launchers)
  sidearm: bool,                  # on his sidearm
  fire: String,                   # "discipline" | "at_will" | "hold"
}
squads[] (own): {                 # plus (S5a):
  ammo_pct: float,                # 0..1, the squad's ammunition share (its
                                  # crew weapon's feed, else its men's main
                                  # weapon; AT teams' rockets count too);
                                  # absent for uncounted squads
  ammo_state: String,             # "ok" | "low" (under rules supply.low_pct)
                                  # | "dry" (nothing left of that feed)
  crew_weapon_name: String,       # "MG 42" when a man carries a crew weapon
  crew_rounds: int,               # the team's rounds for it (gun, gunner,
                                  # assistant and bearers; launchers: rockets)
  fire_mode: String,              # "discipline" | "hold" (every man) | "mixed"
  refilling: bool,                # in its own base below a full load: the
                                  # base refill is serving it
}
vehicles[] (own): {               # plus (S5a):
  ammo: { ..., rare_cap: int },   # the most rare rounds it may hold (guns
                                  # with a special round: "APCR 2/3")
  ammo_state: String,             # "ok" | "low" | "dry" from main-gun rounds
}
players[]: {                      # plus (S5a):
  ammo_mp: { small_arms, explosives, shells, total: float },  # MP spent on
                                  # ammunition this match, by supply kind
  refill_paused: bool,            # a refill waited for manpower within the
                                  # last second (top-bar flag)
}
stats[]: { ..., ammo_mp: {...} }  # end screen: the same by player
```

**Intent.** `set_fire { units: PackedInt32Array, mode: "discipline" |
"at_will" | "hold" }` -> `CommandKind::SetFire` (own men only; vehicles
are dropped, their MGs keep discipline). `discipline` (the default) opens
fire within `combat.fire.hold_beyond_pct` (70 %) of the weapon's range (a
weapon's own `hold_beyond_pct` wins); `at_will` to full range; `hold`
only at enemies within `hold_open_m` (30 m), Panzerfausts and launchers by
their own reach, and the squad springs to discipline once one of its men
fires or is suppressed to `combat.fire.hold_spring_points` (ww2: 70,
pinned). A Suppress order fires to full range except in
`hold`.

**HUD (HUD v2, military direction).**

- **Squad card rows** (`detail_panel.gd`): each man's rounds in a small
  column left of his health (red at 0, amber while on his sidearm); the
  crew weapon's carrier shows the team's rounds for it. The label keeps
  "role · weapon in hand". The single-soldier card's Weapon line reads
  "Kar98k · 45 rds".
- **Card header:** tags left of the status: amber LOW, red DRY, a blue
  HOLD when the whole squad holds fire, "Refilling" in base; a 2 px
  ammunition bar under the header.
- **Strip cards** (`roster.gd`): LOW / DRY left of the men count (room is
  always reserved); vehicle cards: the same tag left of the badge, from
  main-gun rounds; the ammo text reads "APCR 2/3" with `rare_cap`.
- **Orders grid:** a thin segmented fire strip under the stance row
  (Discipline | At will | Hold; the selection's shared mode outlined, a
  new mode shows at once as pending). The console is `fire_h` + a gap
  taller (NORMAL 154 px, COMPACT 146 px) so the buttons keep their size.
- **Top bar:** the MP block's tooltip gives the manpower spent on
  ammunition by kind; a red "No manpower for ammo" tag while a refill
  waits. **End screen:** an "Ammunition" row, "small arms 34, explosives
  21, shells 188 MP".
- **Mock** (`MockMatchSource`): rounds per man by type (the MG team's
  belts with its gunner), shots spend them (an MG burst 7), the base
  refills 30 a second at 0.2 MP per kg (28 g), `set_fire` per man, the
  keys above; `set_rounds(sid, frac)` stages LOW / DRY. Screenshot
  staging `--capture_setup s5a_ammo`: the "hud" firefight with Rifle 1
  LOW and selected, MG team 1 DRY, Rifle 2 holding fire.

## S5b: fire missions, observers, ranging, map fire

S5b (`docs/slices/S5b-mortars.md`) replaces the Barrage with the fire
mission. A mission is Direct (the mortar's own leader sees the point),
Observed (a leader of the player inside the voice link of the gunner,
`fire_mission.observer_link_m` 150 m, sees it; the mortar's own leader
sent forward counts) or Map fire (a point the player knows, wider aim
error, no ranging); unknown ground is refused. Keys are own-only; a state
without them (older builds, legacy content) shows the old behaviour.

```text
squads[] (own): {                 # plus (S5b):
  order: String,                  # "fire_mission" (was "barrage")
  rounds: int,                    # rounds left of the mission
  mission: {                      # while on a mission (S5b rules only)
    mode: String,                 # "direct" | "observed" | "map": the last
                                  # round's (the order's before the first)
    phase: String,                # "laying" | "adjusting" | "effect"
    error_m: float,               # aim error now (m) ...
    first_error_m: float,         # ... and at the first round (ranging bar)
    seen: int,                    # rounds an observer saw land
    fired: int, rounds_left: int, adjust: int,
    charge: int,                  # charge of the last round (1 = charge I)
    flight_s: float,              # flight time of the last round (s)
    ranged: bool,                 # started from a remembered ranging cell
    target: Vector3,              # the mission's point (ground)
    gunner_pos: Vector3,          # the man at the tube
    observer_id: int,             # the observer of the last round, and
    observer_pos: Vector3,        # where he stands (absent: no observer
                                  # now, or he fell): the link line
  },
}
fire_preview: {                   # while the tool asks (set_fire_preview)
  squad: int, point: Vector3, rounds: int,
  ok: bool,                       # false: refused now
  reason: String,                 # when refused: "not_mortar" | "rounds" |
                                  # "off_map" | "inside_minimum" |
                                  # "cannot_reach" | "unknown_ground" |
                                  # "no_bombs"
  text: String,                   # the HUD's reason line (refused)
  mode: String,                   # "direct" | "observed" | "map"
  error_m: float,                 # start aim error (the remembered one
                                  # when `ranged`)
  ring_m: float,                  # expected miss: error_m plus half the
                                  # precision spread's extent at that range
  ranged: bool, walk: bool,       # walk: beyond range, the team walks up
  range_m: float,                 # from the gunner
  gunner_pos: Vector3,
  observer_id: int, observer_pos: Vector3,  # observed / direct
}
intel: [ { pos: Vector3, age_s: float, trench: bool } ]  # the own knowledge
                                  # map (25 m cells: enemies last seen there,
                                  # trench cells for the match), with the
                                  # preview only
events[]: { kind: "fire_mission_refused", text, squad: int,
            reason: String, ... } # "Fire mission refused: Unknown ground: ..."
rules: { ..., fire_mission_rounds: int }  # the tool's default rounds
                                  # (fire_mission.player_rounds)
squads[].capabilities: ["fire_mission", "barrage", ...]  # "barrage" kept as
                                  # an alias for one slice
```

**Preview.** `FrontlineMatch.set_fire_preview(squad: int, point: Vector3,
rounds: int)` asks; `clear_fire_preview()` stops. The sim thread answers
when it publishes the next frame (also while paused, once per new
request) with the sim's read-only `MatchState::fire_preview`: the same
refusal check and mode check as the order, never inside a tick, so it
cannot change the match. The mock (`MockMatchSource`) has the same two
methods and a simple model (Direct within 400 m of the mortar, Observed
when another own squad inside 150 m of it is within 400 m of the point,
known = a capture point within 40 m or a seen enemy within 30 m).
`MatchView.preview_source` is the source when it has `set_fire_preview`
(`match.gd`).

**Intent.** `fire_mission { squads: PackedInt32Array, target: Vector3,
rounds: int }` -> `CommandKind::FireMission` per mortar squad (`rounds`
1..=50; absent: `fire_mission.player_rounds`). `barrage` is kept as an
alias for one slice; both emit `barrage_ordered`.

**HUD (HUD v2, military direction).**

- **Fire-mission tool** (key B, orders-grid button "Mission", tooltip
  "Fire mission: ..."): while armed, the selection controller asks for
  the preview of the focused (else first) selected mortar squad at the
  cursor; the cursor label reads `FIRE MISSION · Observed · ±24 m · 6
  rounds · Shift+wheel rounds` (`· ranged in`, `· out of range: the team
  will move` when they apply) or, in red, the refusal (`FIRE MISSION ·
  Unknown ground: no observer sees it and nothing was seen there`, `Too
  close: inside the minimum range`, `No bombs`, ...). A click at a refused
  point sends nothing (error sound; the tool stays). Shift+wheel sets the
  rounds 1..50. The ground shows the expected-miss ring (solid green for
  observed and direct, dashed amber for map fire, a red cross when
  refused), the would-be observer's link (gunner -> observer -> point,
  dashed) and the knowledge map (faint squares; trench cells amber). The
  range rings stay.
- **Observer link line** (`ground_overlay.gd`): a selected own mortar on
  a mission draws gunner -> observer (solid) and observer -> target
  (dashed); none for direct fire; a dotted red line to the target when
  the link is broken (map fire, or the observer fell).
- **Mortar card** (`detail_panel.gd`): the header status gives the phase
  and rounds left ("For effect · 6 rds"); a mission strip under the men:
  the mode tag (DIR blue / OBS amber / MAP red), "Ranging 2 · ±3 m ·
  15.1 s" (rounds seen land, aim error now, flight time of the last
  round; map fire: "No ranging") and a ranging bar (share of the first
  round's error ranged out).
- **Feed:** `fire_mission_refused` events ("Fire mission refused: ...").
- **Mock:** missions with modes, laying 10 s (direct 4 s), adjusting
  rounds that halve the error when observed, then fire for effect;
  refusals as events; the preview and intel. Screenshot staging
  `--capture_setup s5b_mortars` (the tool armed on an enemy squad with an
  observer in the link) and `s5b_card` (an observed mission ranging in,
  the mortar selected).

## S6: HQ, ammunition dumps and crates

Design: `docs/slices/S6-hq-crates.md`. The bridge fills these from the
render snapshot (`StructureSnapshot`, `CrateSnapshot`); the mock shows only
what a test stages (`stage_hq`, `stage_crate`, `set_tags`).

### State keys

- `squads[].tags: PackedStringArray` — the card's tags (`hq` builds the HQ,
  `bearers` carry crates; others as S4).
- `structures: Array` — own HQs and dumps, and enemy ones within sight of
  an own man, ascending id: `{id, owner, kind: "hq"|"dump", name, pos:
  Vector3 (pit centre on the ground), yaw (rad, sim: from +x counter-
  clockwise seen from above), radius (m), built: bool, build_pct (0..100
  while the pit is dug), radio: bool, stock_kg: {small_arms, explosives,
  shells}, cap_kg: {...}, boxes: [{part: "stock"|"radio", pct (hp left,
  0..100), size: Vector3 (along, up, across, m), offset: Vector2 (along,
  across the facing, m)}]}`.
- `crates: Array` — own crates and enemy ones within sight (set down) or on
  a seen man, ascending id: `{id, owner, name, kind: "small_arms"|
  "explosives"|"shells", kg, pos: Vector3 (on the ground, or the carrier's
  position), carrier: man id or -1}`.
- `soldiers[].crate: {id, kind, kg}` — the crate he carries (absent: none).
- `players[].crate_ready_s` (s until the base issues the next crate),
  `players[].crates`, `players[].shell_crates` (live crates; caps
  `crate_cap` / `shell_crate_cap`).
- `rules.supply: {crate_feed_m, low_pct}`; `rules.dig_profiles` gains
  `mortar_pit`, `hq_pit`, `dump_pit`.
- `events[]` kind `supply_refused` (text "Refused: <why>.", `reason` one
  of not_allowed, cannot_build, cap, rebuild, bad_spot, manpower,
  no_carrier, busy, no_crate; `squad`).

### Intents

- `build {squads, target: Vector3, what: "hq"|"dump", facing: Vector3}` —
  the first selected squad whose card may build it digs the pit there.
- `issue_crate {squads, kind}` — each selected squad takes one crate of
  `kind` in the own base (paid there).
- `place_crate {squads, target}` — the squads set their carried crates
  down at `target` (into an HQ or dump stock inside its ring).
- `pick_up_crate {squads, crate: id}` — the first selected squad picks up
  an own crate on the ground.
- `dig {profile}` also takes `mortar_pit`, `hq_pit`, `dump_pit`.

### Presenter

- Orders grid "supply" mode (only HQ squads or bearers selected): Build HQ,
  Dump, Shells, Ammo, Expl. in the top row; Place and Pick up beside the
  stance strip.
- Detail panel supply strip under the men: the HQ squad's HQ (three stock
  bars, kg of the cap per kind, the low marker at `rules.supply.low_pct`,
  radio state, or "Digging N %"); any squad carrying crates: "Crates:
  shells 32 kg ×2".
- Roster cards: `CRATE` / `CRATE×n` tag while the squad's men carry crates.
- Ground overlay: own structures' radius rings (dashed while digging) when
  an HQ squad, bearers or a mortar is selected or a supply tool is armed;
  5 m feed rings round own set-down shells crates with a mortar selected;
  the radius ghost under a build tool.
- `SupplyView` (`match/supply_view.gd`) draws structure boxes and set-down
  crates as plain boxes (ghosts while the pit is dug).
- Autoplay scene `armour_hq_supply` (sim `scenarios::scene("hq_supply")`).

## S7: assault guns, tank destroyers, open tops

### State keys

`rules.vehicle_types["<type>"]` gains (both sources; the mock has one
casemate type, `ger_stug3_g`, and one open-topped type, `usa_m10`):

```
role: String,                    # "tank" | "assault_gun" | "tank_destroyer"
casemate: bool,                  # the main gun sits in a casemate (no turret)
open_top: bool,                  # a turret-frame box has an open top
turret_floor: float,             # casemate: block bottom below the gun pivot (m)
boxes: [                         # armour boxes, box 0 the hull (sim frame:
  { id: String,                  #   x forward, y left, z up, m; a "turret"
    frame: String,               #   box from the pivot) "hull" | "turret"
    center: Vector3, half: Vector3,
    open: int },                 # open plate groups, bit 0 front 1 side 2 rear 3 top
],
gun_pivot: Vector3,              # turret pivot or casemate gun pivot (sim frame)
seat_ids: PackedStringArray,     # def seat ids in seat order ("driver", "asst_driver", ...)
```

For a casemate, `turret_length/width/height` and `turret_offset_x` describe
the casemate block (from the gun pivot), and `vehicles[].turret_yaw` is the
gun's yaw within its arc (the hull turns when a target lies outside it).
Seat pips stay by role (`seats`); an M10's assistant driver takes the
`radio_mg` pip (S7-4). Thrown items inside an open box ride with the
vehicle: `thrown[].pos` follows it (no new key).

### Presenter

- `VehicleView`: the "turret" part of a casemate type is its block, fixed
  to the hull; only the gun turns about the pivot. An open-topped turret
  is four walls without a roof (`meshgen::vehicle_turret_mesh`). Style-
  neutral blockouts; the GDScript fallback composes the same.
- Call-in tiles: the new cards come from the roster like any card (bridge:
  `ger_stug3_g_squad`, `usa_m10_squad`; mock: `MockVehicles.FACTION_TD`).

## S8a: half-tracks, passengers, Board and Unload

Spec `docs/slices/S8a-halftracks.md` 5 and 8 (the bridge only; the mock has
no half-track yet).

### State keys

`rules.vehicle_types["<type>"]` (Build A, plus the blockout keys):

```
role: "transport",               # an infantry carrier
has_gun: false,                  # no main gun
pintles: [{ pivot, muzzle: Vector3, arc: Vector2 }],   # sim frame, deg
shields: [{ pintle: int, center, half: Vector3 }],     # pintle frame or hull (-1)
slots: [{ center, half: Vector3 }],                    # passenger slots, hull frame
transport: true,                 # meshgen: hull from `boxes`, "turret" = the
turret_lift: float,              #   first pintle's shield (turret_* its size,
                                 #   turret_x/base its pivot), "gun" = that MG
```

`vehicles[]` (every vehicle; public, the men over the rim are seen):
`riders: int` (men aboard), `capacity: int` (slots; 0 for other vehicles),
`unload: String` ("" | "door" | "sides"). A vehicle without a main gun sends
its first MG's yaw as `turret_yaw` (the shield and MG turn with it). Own
transports add `passengers: [{ squad, name, men, mounted, health 0..1 }]`.

`squads[]`: a squad walking to or riding a half-track has `order: "embark"`,
`embark: <vehicle id>` and `mounted: int` (men aboard). Men aboard keep their
ids in `soldiers`, stand at the bottom of their slot and are drawn crouched
(seated); they take no orders (the sim refuses squad commands to a squad
with men aboard and drops them from soldier commands).

### Intents

- `embark {squads, vehicle}`: the selected infantry squads board own
  transport `vehicle` (one `Embark`; the sim refuses a squad that does not
  fit the free slots).
- `disembark {units, over_sides}`: the passengers of the listed own
  transports, and of the selected mounted squads' transports, get out by the
  door or over the sides (one `Disembark`).

### Presenter

- Hotkeys (S8a-7 (A)): V arms the Board tool (click an own half-track),
  right-click on an own half-track with infantry selected boards it too;
  U unloads by the door, Shift+U (or Shift+click on the button) over the
  sides.
- Orders grid: Board shares the Crew slot (soldier grid); Unload replaces
  Regroup while the selection has passengers, and takes the fifth top-row
  slot of the vehicle grid.
- Cards: a squad aboard reads "Mounted" (roster) and "Mounted (SdKfz 251)"
  (detail; "Boarding n/m" while men still climb in); a half-track's card
  lists "Aboard n/cap", each passenger squad with its men and a health bar,
  and its MG stowage in place of the main-gun ammunition row.
- `VehicleView`: half-track blockouts from the hull-frame boxes (a closed
  box solid, the open compartment four walls and a floor), rear tracks and
  front wheels, the MG shield and MG turning with the pintle.
- Autoplay: `--autoplay_scene armour_halftracks` (a loaded SdKfz 251 and a
  loaded M3A1, the 251 selected) saves `s3_halftracks.png`.

## S8b: wheels, recon cars, trucks, Drop Crates

Spec `docs/slices/S8b-trucks-recon.md` 10 (the bridge only; the mock has no
truck, the tests stage dictionaries).

- `vehicles[].components` lists only the parts the vehicle has (S8a-27):
  `wheels_f`, `wheels_r` replace the tracks on a wheeled vehicle, `cargo` is a
  truck's. `Vehicles.parts_of(v)` and `strip_of(v)` read what is present.
- `vehicles[]` of an own truck: `cargo_kg { shells, small_arms, explosives }`
  (float kg), `load_kg` (what it takes on in base), `cargo_cap_kg`,
  `drop_left` (crates of the current order still to set down).
- `rules.vehicle_types[t]` adds `wheels` (axles, 0 tracked), `screen` (a wire
  screen over the open turret), `no_weapon` (a truck), `cargo_center`,
  `cargo_half` (sim frame, m), `supply_radius_m`, `supply_vehicle_radius_m`.
- Call-in catalog: a card outside the vehicle limit has `own_cap: true`,
  `alive`, `max_alive`; its tile badge reads `alive/max` and `blocked_reason`
  gives `"card"` at its cap. `locked_s` follows the card's own unlock.
- Intent `drop_crates { units, target, kind, count }` (`kind`: `"shells"`,
  `"small_arms"`, `"explosives"`; `units`: own trucks).
- Hotkey I (`order_drop_crates`, S8b-19): arms the Drop tool on a selected
  truck; pressing I again changes the kind (default: the fullest share of its
  load); the button then names the kind. A click sends 4 crates.
- A truck's card has three stock gauges (the HQ's, `load_kg` as their cap)
  and `Drop n` while dropping; a selected truck draws its reach rings: the
  vehicle radius solid, the men's radius dashed.
- "MG dry" on the 251 card (S8a) was the `halftracks` scene, which empties
  its MGs to hold fire; a fresh half-track reports its stowage
  (`match_frame` test `a_fresh_half_track_reports_its_mg_stowage`).
- `--autoplay_scene armour_recon_trucks` (a 234/1, an M8 and a loaded truck,
  the truck selected) saves `s3_recon_trucks.png`.

## S9: AT guns

Spec `docs/slices/S9-at-guns.md` (13). The bridge only; the mock has no gun, the tests stage dictionaries.

- A gun is a vehicle (`vehicles[]`): `gun_state` is present on guns only:
  `"limbered" | "emplaced" | "emplacing" | "packing" | "pushing" | "hitched" | "hitching" | "unhitching"`,
  and `state` shows that word while the gun is not a wreck or neutral. `gun_men` / `gun_slots`
  (public: men at the slots and slots), `hitched` (the tow's id, when hitched),
  `hold_fire_m` (own only; 0: no ambush). A towing vehicle has `towing` (the gun's id).
  The crew are ordinary soldiers in `soldiers[]`, at their slots; no new soldier field.
- Neutral: `abandoned` true, `owner` -1, `last_owner`, `abandon_cause` `"gun_left"` (seen by all,
  tinted grey by `vehicle_view.gd`). Serve it with the Crew tool (K), as any abandoned vehicle.
- `rules.vehicle_types[t]` of a gun: `served` true, `arc_deg { lo, hi }` (degrees from the gun's
  front), `optic_m` (m), `boxes` (axle, shield, trail: the blockout, meshgen `gun_carriage_mesh`);
  `components` adds `sight` and `recoil` (`Vehicles.COMPONENTS`: 12).
- Intents (own guns only, `units`): `emplace { target }` (the point it faces), `pack_gun`, `unhitch`,
  `set_ambush { range_m }`, `push_gun { target }`, `hitch { tow }` (a vehicle id). A plain move of a
  gun squad is refused by the sim: right-click on the ground sends `push_gun` for a gun-only selection.
- HUD: a gun-only selection takes the gun grid (`orders_grid.gd` `ROW_GUN`). Keys reuse the actions its
  men do not need: Y Emplace, L Pack, V Hitch, U Unhitch, B Ambush (off, 400, 250 m; men inside 200 m),
  M Push; no new input actions, so nothing clashes (`test_match_view.gd` `_test_s9`).
- A selected emplaced gun draws two rays along its arc to `optic_m` (`ground_overlay.gd`).
- `--autoplay_scene armour_guns` (an emplaced Pak 40 selected, a second gun hitched to an Sd.Kfz. 11,
  the crew aboard) saves `s3_guns.png`.

## S10a: fortifications, wire, Bangalore (mechanics half)

Design: `docs/slices/S10a-fortifications.md` 6. The bridge fills these from
the render snapshot (`FortSnapshot`, `SquadSnapshot::materials_kg`,
`fort_refusals`); the mock shows only what a test stages (`s10_fort`,
`set_caps`). Structure selection stays out (S6-18).

### State keys

- `fort: {pieces, wire, sites, charges}` — own entries and enemy ones
  within sight of an own man, ascending id each:
  - `pieces[]: {id, owner, blueprint (short id, e.g. "mg_nest"), part:
    "sandbag"|"roof"|"timber", center: Vector3 (box centre), yaw (rad,
    sim: from +x counter-clockwise; the same angle about Godot +y), size:
    Vector3 (along the facing, up, across, m), hp (0..1 left)}`;
  - `wire[]: {id, owner, a: Vector3, b: Vector3 (section ends on the
    ground, 2 m apart), band_m, height_m, hp (0..1)}`;
  - `sites[]: {id, owner, blueprint, builder (squad id), step: "dig"|
    "materials"|"build", pos: Vector3 (the current step's spot: a line's
    current segment), yaw, line: PackedVector3Array (the ordered points),
    seg, segments, progress (0..1 of the step: pit dug, work done),
    materials (0..1 of the step's kg drawn)}`;
  - `charges[]: {id, owner, from: Vector3, to: Vector3 (the pipe), fuse_s}`.
- `squads[].materials_kg` (own squads) — construction kg carried.
- `crates[].kind` and `soldiers[].crate.kind` may be `"construction"`.
- `squads[].capabilities` may hold `build_structure`, `build_heavy`,
  `place_wire`, `clear_wire`, `breach`; `soldiers[].items` and
  `squads[].items` hold `bangalore`; a man laying a charge shows
  `throwing` (the placing windup).
- `rules.fortify: {blueprints: {<short id>: {id, name, shape: "point"|
  "line"|"over_hq", capability, kg (point: whole work; line: per
  segment), work_s, work_men, limit, segment_m, max_length_m, wire: bool,
  pit: bool}}, carry_kg_per_man, mp_per_t, cut_s, cut_reach_m,
  wire_speed_pct}`.
- `events[]` kind `fort_refused` (text "Refused: <why>.", `reason` one of
  not_allowed, cannot_build, cap, bad_spot, manpower, not_in_base,
  no_wire; `squad` when the command named one).

### Intents

- `fortify {squads, what, polyline | target + facing}` — `what` a
  blueprint short id (`sandbag_wall`, `barbed_wire`, `mg_nest`, `dugout`,
  `hq_dugout`) or full id; the first selected squad with the blueprint's
  capability builds it: a line blueprint along `polyline` (2..16 points,
  longer drags are thinned), a point or over-HQ one at `target` facing
  `facing` (a Vector3; default east).
- `load_materials {squads, kg?}` — every selected squad with
  `build_structure` or `place_wire` loads `kg` (default a full load,
  `carry_kg_per_man` per man) in the own base, paid there; a supply
  truck's squad fills its truck (default all the room left: its cap less
  cargo). The HUD's truck grid has a Materials button for it.
- `cut_wire {squads, target}` — the first selected `clear_wire` squad cuts
  the nearest standing section within `cut_reach_m`.
- `bangalore {squads, target}` — the men of the first selected `breach`
  squad that carries a Bangalore; the sim sends the nearest carrier.
- `issue_crate {kind: "construction"}` — a 40 kg construction crate.

### Presenter

- Orders grid: a `Build` button (key `, `order_build_menu`) in the
  Mission slot for a selection with a fortification capability and no
  mortar; it opens the "fort" mode: Sandbags, Wire (line tools as Dig in,
  capped at the blueprint's `max_length_m`), MG nest, Dugout, Roof HQ
  (point tools: one click, the camera's forward is the facing) on top;
  Materials, Crate, Cut wire, Bangalore (tools: click the wire), Back
  below; the stance strip stays. Tooltips give kg, time, men and the
  capability from `rules.fortify`; buttons the selection cannot use are
  grey.
- `FortView` (`match/fort_view.gd`) blockouts: pieces as boxes (sandbag
  tan, roof timber), wire as posts at each section end with a translucent
  band, sites as a ghost box and a ring of twelve ticks lit to the step's
  progress (amber while waiting for materials), charges as a thin pipe.

## S10b: mines, AT obstacles, the bunker (mechanics half)

Design: `docs/slices/S10b-obstacles.md` 1, 3, 4. The bridge fills these from
`FortSnapshot::mines`, `ChargeSnapshot::piece`, `SquadSnapshot::clear_lane`
and `demolish`; the mock shows only what a test stages (`s10_fort`,
`set_caps`). The AI uses none of it yet (S10b-ai).

### State keys

- `fort.mines[]: {id, owner, blueprint ("ap_minefield"|"at_minefield"),
  kind: "ap"|"at", pos: Vector3 (on the ground), known: bool}` — only mines
  the local player knows: its team's own, ones its team found by sweeping,
  and every one that has gone off (design 7.6, a presentation rule; the
  bridge drops the others, the sim keeps them all).
- `fort.charges[].piece` — the piece a demolition satchel is tamped
  against (-1: a Bangalore).
- `fort.pieces[].part` may be `"steel"` (hedgehog); blueprints `hedgehog`,
  `log_barrier` (timber) and `bunker` (`roof` part 0, timber walls).
- `squads[].clear_lane: PackedVector3Array [a, b]` and `clear_along` (m
  from a) — an own squad sweeping; `squads[].demolish_piece` — an own
  squad's demolition target; `order` `"clear_mines"`, `"demolish"`.
- `squads[].capabilities` may hold `place_mines`, `clear_mines`,
  `demolish`.
- `rules.fortify.blueprints.<id>` gains `mine: bool` and
  `blocks_vehicles: bool`; `rules.fortify` gains `heavy_by_field_pct,
  lane_max_m, lift_s, demolish_place_s, demolish_fuse_s, demolish_reach_m`.
- `events[]` `fort_refused` may carry reason `no_target`.

### Intents

- `fortify {what: "ap_minefield"|"at_minefield"|"log_barrier", polyline}`
  and `{what: "hedgehog"|"bunker", target, facing}` — as S10a; a
  `build_structure` squad may build the `build_heavy` hedgehog and bunker
  (at `heavy_by_field_pct` of its rate).
- `clear_mines {squads, polyline}` — the first selected `clear_mines`
  squad sweeps the lane from the first point to the last (at most
  `lane_max_m`).
- `demolish {squads, target}` — the first selected `demolish` squad; the
  sim takes the piece nearest `target` and the nearest satchel carrier.

### Presenter

- Orders grid "fort" mode: when the selection can use any S10b entry, the
  third row shows the ones it can (AP mines, AT mines, Hedgehog, Logs,
  Bunker, Clear mines, Demolish) instead of the stance strip. Line tools as
  Wire; Clear mines takes points like a line and sends the ends; Demolish
  clicks the piece (shown only with a satchel). Tooltips: kg, time, men,
  capability, the limit, "Stops vehicles", and the half-rate note for a
  field squad on a heavy blueprint.
- `FortView`: mines as small discs (AP grey, AT dark green), drawn when
  own or `known`; the hedgehog as three crossed steel bars; log segments as
  timber boxes; the bunker's roof earth-brown with a dark slit box under
  its front edge; a demolition charge as a satchel box until it bursts; an
  own sweep lane as a dashed yellow line with a cross at `clear_along`.

## S12: wounds, medics, aid posts, Reinforce (mechanics half)

Design: `docs/slices/S12-wounds-medics.md` 4 (W1-W11). Everything is off
unless the rules' `[wounds] enabled` is 1 (Reinforce also needs
`reinforce`). The bridge fills these from `RenderSnapshot::soldier_wound`,
`casualties` and `StructureSnapshot::beds`, `staff`; the mock shows none of
it yet. The AI uses none of it yet (S12a Build C).

### State keys

- `soldiers[].wound: {bandaged, legs, treated, bandaging: bool}` — only on
  a wounded man (absent: fit).
- `casualties[]: {id, owner, pos}` — downed men the local player sees
  (own: all); own ones add `stabilised: bool, bleed_s` (seconds to
  bleed-out, 0 once stabilised), `tend` (the medic, -1: none), `tend_s`,
  `carriers: PackedInt32Array` (0-2 men), `dest: Vector3` (while carried),
  `evac_s` (seconds to evacuation in base, 0: none). A downed man is also
  drawn as a body (he stays in the corpse layer, `corpses` count).
- `structures[].kind` may be `"aid"`; every structure has `beds[]: {man,
  seconds_left, evac: bool}` (an aid post's beds in use; the clock pauses
  while unstaffed) and `staff` (own medics in its radius now; it works at
  the def's `staff_min`).
- `squads[].capabilities` / role words: medic cards carry the tag `medic`
  (`ger_sanitaetstrupp`, `usa_aid_men`, ai_weight 0).

### Intents

- `tend_units {units, man}` — the nearest of the own `units` tends man
  `man`: a medic stabilises a downed man, anyone bandages a wounded man.
- `carry_units {units, man, target}` — one or two of `units` (medics
  first: stretcher pace, else drag pace) carry downed man `man` to
  `target`.
- `evacuate_units {units}` — wounded or downed own men at a staffed aid
  post with a free bed, or in base, leave the map (refund if paid).
- `reinforce {squads}` — each selected own infantry squad fills its gaps
  from base (manpower, pop room and the cooldown permitting).
- `build {what: "aid", ...}` — as the S6 dump: an aid post (`aid_pit`
  profile, built by `medic`-tagged squads).
- `events[]` `fort_refused` may carry the S12 refusals (`not_allowed`,
  `cap`, `manpower`).

### Presenter

Built (S12 Presenter P); it shows wounds only when the state does
(`MatchView.s12_on()`: `rules.wounds.enabled` when the bridge sends it,
else a wound, a casualty, an aid post or an own `medic` squad in the
state).

- `soldiers[].wound` draws a pip above the man (red bleeding, amber being
  bandaged, white bandaged, green treated; a tan low pip for a leg wound);
  the detail card says the word; the roster pip reads wounded.
- `casualties[]` draws a groundsheet and a pip (red bleeding out, white
  stabilised), a stretcher (two poles) while carried, and an overlay tag
  (own: seconds to bleed-out, Tending, Stable, Carried, Evac). A man
  with a `medic` squad tag wears a white armband. No red cross anywhere.
- Aid posts (`kind: "aid"`) draw the tent box and a cot per bed.
- Medic cards (tag `medic`) show a stretcher glyph and are hidden while the
  wound layer is off.
- Orders grid: an Aid button (Mission slot, or the build menu's third row)
  opens the aid menu: Tend (click a wounded or downed man: `tend_units`),
  Carry (click a downed man, then the ground: `carry_units`), Evacuate
  (selected wounded men go at once, else click a downed man:
  `evacuate_units`), Reinforce (`reinforce`, squads below `men_max`), Aid
  post (medic squads; `build {what: "aid"}`). Medic squads get the Build
  button; the build menu's third row holds Aid post and Aid.
- The mock stages it: `stage_wound`, `stage_downed`, `stage_aid_post`,
  `place_medics`; screenshot stage `s12_wounds`.

The bridge does not yet send `rules.wounds {enabled, reinforce}`; until it
does the medic card stays hidden and the Aid button appears once the first
wound, casualty or medic squad is in the state.

## S14: flamethrowers, burning ground, strikes (mechanics A-C, presenter P)

Design: `docs/slices/S14-assault-kit.md` 3, 5, 6. Everything is off unless
the rules switch it on (`[flame] enabled`, `[melee] enabled`, `[clear]
enabled`; all 1 in the shipped rules since S14 C), and every field below is
empty or absent while it is off. The bridge sends them; the mock shows one of each
once `stage_s14` is called (`rules.s14` on).

### State keys

- `rules.s14: {flame, melee, clear}: bool` -- the three switches
  (`MatchView.s14_on(key)`; the Clear trench and Strike buttons show with
  their switch on).
- `flame[]: {id, pos: Vector3, player}` -- flame particles in flight,
  ascending id (own always; the enemy's within sight of an own man). They
  live in their own list, never in `projectiles`.
- `patches[]: {id, pos: Vector3, player, seconds_left, radius}` -- burning
  ground (same sight rule, plus 6 m); it burns within `radius` m of `pos`
  (`[flame] patch_radius_cm`) until `seconds_left` runs out.
- `soldiers[].burning: bool` -- only when true (a flame hit or burning
  ground). A downed man (in `casualties[]`, S12) who burns carries no
  flag: `burning` is sent per standing soldier only.
- `soldiers[].strike: int` -- only when not 0: 1 wind-up, 2 recovery (the
  strike pose).
- Weapons: `WeaponClass::Flame` (`class` "flame" in defs); the bridge's
  sound cue for its shots is the MG's (`"mg"`).

### Intents

- `strike_units {units}` (direct control `V` while `rules.s14.melee` is
  on and an enemy is within 1.5 m of a selected man, else `V` boards; the
  Strike button) -- each of the own `units` with no strike running
  starts one at the nearest enemy within reach. Refused (`fort_refused`,
  `not_allowed`) with `[melee] enabled` 0 or when nobody is in reach. The
  bridge maps it to `Strike`; the mock does not handle it.
- `clear_trench {squads, target}` (the Clear button arms a click tool) --
  the first selected infantry squad clears the trench cell nearest
  `target` (`ClearTrench {squad, at}`): grenades, then the entrance.
  Refused (`fort_refused`) `not_allowed` (switch off, vehicle or gun squad,
  no frag grenade) or `bad_spot` (no trench near). The mock does not handle
  it.

### Presenter (`match/flame_view.gd`)

One MultiMesh of short bright boxes for the stream (at most 512: the rules
allow 256 per player, `MAX_FLAME_PER_PLAYER`, so a 1v1 never overflows it), a
flickering disc for each patch with a light under the first 3, a small
flame over each burning man, and the strike pose as a pale bar over the
striker (raised in the wind-up, thrust toward the nearest enemy in the
recovery; the soldiers are instanced meshes without a rig, so there is no
arm to move). Nothing is drawn while the fields are empty. Burning men are
not tinted (the instanced bodies take no per-man tint from GDScript); the
flame marker stands for it. `V` is also Board's key: with `rules.s14.melee`
on, men selected and an enemy within 1.5 m of one of them
(`STRIKE_KEY_M`, the sim's default reach plus a margin) `V` strikes;
otherwise it arms Board as before (S14-11 (A)).
