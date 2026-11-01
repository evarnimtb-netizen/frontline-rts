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
                                 # | "dig" | "barrage" | "setup" (MG setting up)
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
| `barrage` | `{squads, target: Vector3}` | `barrage_ordered(squad_ids, target)` | Barrage (only mortar squads in range; a selected mortar man sends his squad) |
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
| B | barrage tool (mortars) |
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
                                   # BURNING, WRECK, KNOCKED_OUT
    state: String,                 # the badge: first that applies of "wreck",
                                   # "burning", "bogged", "immobilised",
                                   # "repairing", "crossing", "reversing",
                                   # "moving", "idle". Enemies: "wreck",
                                   # "burning", "moving" (|speed| > 1) or "idle"
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
    ammo: { ap: int, he: int,      # main-gun rounds left; S3b: apcr only on a
            apcr: int,             # gun that carries a special round (Panzer IV);
            mg: int },             # S3b Fix: MG rounds left in stowage (if limited)
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
                                   # | "knocked_out" | "bogged"
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
                                   #   a rack fire burned out; "crew": fewer
                                   #   than the rules' crew_ko_below alive)
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
rifleman). Squads never have type `"vehicle"`.

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

**Feed** (`events`), new kinds the bridge derives (the feed shows only
main-gun and rocket matters):

| `kind` | When | Example `text` |
|---|---|---|
| `vehicle_arrived` | an own vehicle call-in (S3b: spawns at once) | "Panzer IV deployed at base." |
| `vehicle_knocked_out` | any seen vehicle knocked out (`player` = its owner) | "Panzer IV knocked out." / "Enemy Sherman knocked out." |
| `vehicle_damaged` | an own component reaches 0 HP | "Panzer IV: track destroyed." |
| `vehicle_bogged` | an own vehicle bogs | "Sherman bogged in trench." |

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
