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
      status: String,            # "ok" | "suppressed" | "pinned" | "retreating"
      order: String,             # "idle" | "move" | "attack_move" | "retreat"
                                 # | "dig" | "barrage" | "setup" (MG setting up)
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
      suppression: float,        # opt, 0..1
      in_trench: bool,           # opt; standing in a dug trench (bridge)
      # Own soldiers only (leave these out for enemies):
      stance_override: String,   # "auto" | "stand" | "crouch" | "prone"
      detached: bool,            # follows his own unit order
      order: String,             # opt, detached men: "move" | "attack_move"
                                 # | "retreat" | "dig" | "hold" | "throw"
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
  callin: {                      # local player only
    catalog: [ { type: String, name: String, cost: int, delay_s: float, men: int,
                 short: String,           # opt; tile name ("Engineers")
                 cooldown_s: float,       # opt; re-call cooldown left (not an
                 cooldown_total_s: float, #   S1 rule; drawn when sent)
               }, ... ],
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
- `players[].pop` counts living soldiers only (the call-in panel adds the
  queue); `income` and `ticket_drain` are per second.
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
`set_speed(x)` (0.25–8, scripted runs), `get_stats()`, `get_last_error()`.
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
`top_bar.gd` (with the A–E chips), `minimap.gd` (north-up rounded square;
setting `minimap_rotate` turns it with the camera as a disc with an off-map
hatch; `minimap_dock` puts it bottom-left), `event_feed.gd` (3 lines,
click-through), `roster.gd` (group tabs, squad cards with per-man pips),
`detail_panel.gd`, `orders_grid.gd` (abilities, orders, stance strip) and
`call_in_panel.gd`. Rings under selected and hovered soldiers are one
MultiMesh batch (`match/selection_rings.gd`, one buffer upload per frame);
detached men's targets, throw range rings and the throw target are drawn by
`ground_overlay.gd`; the hover tooltip and tool hints by `unit_overlay.gd`.

## Autoplay

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
  presenter input and layout tests on the mock.
- `Godot --headless --path godot -s res://tools/check_project.gd` loads every
  script and scene and runs the menu and the match for a few frames.
