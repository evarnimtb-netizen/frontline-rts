class_name MatchView
extends CanvasLayer
## The match presenter's single interface (S1). Feed it one plain
## Dictionary per frame with update_state(); it draws the HUD and the
## ground overlay, plays effect sounds, runs selection and order input, and
## reports what the player wants through signals. It never touches the sim.
##
## The state schema is in godot/docs/match_state.md. Every intent is emitted
## twice: as `intent(kind, data)` (one handler can map all of them to sim
## Commands) and as the specific signal below. Squad ids are sorted
## ascending in every PackedInt32Array, matching the sim's iteration order.
##
## Wiring (see match.gd):
##   view.camera = $Camera            # rts_camera.gd
##   view.height_fn = sim.get_height_at   # (x, z) -> ground y; optional
##   view.intent.connect(source.handle_intent)
##   func _process(_d): view.update_state(source.get_state())

signal intent(kind: StringName, data: Dictionary)
signal selection_changed(squad_ids: PackedInt32Array)
signal move_ordered(squad_ids: PackedInt32Array, target: Vector3)
signal attack_move_ordered(squad_ids: PackedInt32Array, target: Vector3)
signal stop_ordered(squad_ids: PackedInt32Array)
signal retreat_ordered(squad_ids: PackedInt32Array)
signal dig_ordered(squad_ids: PackedInt32Array, polyline: PackedVector3Array)
signal barrage_ordered(squad_ids: PackedInt32Array, target: Vector3)
signal call_in_requested(squad_type: String)
signal surrender_requested
signal pause_requested(paused: bool)
signal quit_to_menu_requested

## Squad types that can dig / fire a barrage (S1 roster).
const DIGGERS := ["rifle", "engineer"]
const MORTARS := ["mortar"]

@export var camera_path: NodePath
@export var overlay_path: NodePath

## Ground height for picking and overlays: (x: float, z: float) -> float.
var height_fn: Callable
var camera: Camera3D
var overlay: Node3D
## Latest state as given (read-only for components).
var state: Dictionary = {}
var local_player := 0
var map_half := 512.0
## id -> squad Dictionary, and id -> screen position for squads in front of
## the camera (filled each update).
var squads_by_id: Dictionary = {}
var squad_screen: Dictionary = {}
var paused := false
var ended := false

@onready var hud: Control = $Hud
@onready var units: Control = $Hud/UnitOverlay
@onready var top_bar: Control = $Hud/TopBar
@onready var point_strip: Control = $Hud/PointStrip
@onready var event_feed: Control = $Hud/EventFeed
@onready var minimap: Control = $Hud/Minimap
@onready var call_in: Control = $Hud/CallInPanel
@onready var cards: Control = $Hud/SquadCards
@onready var commands: Control = $Hud/CommandPanel
@onready var pause_menu: Control = $PauseMenu
@onready var end_screen: Control = $EndScreen
@onready var selection: Node = $Selection


func _ready() -> void:
	if camera == null and not camera_path.is_empty():
		camera = get_node_or_null(camera_path)
	if overlay == null and not overlay_path.is_empty():
		overlay = get_node_or_null(overlay_path)
	for c in [top_bar, point_strip, event_feed, minimap, call_in, cards, commands, units, pause_menu, end_screen, selection]:
		if c.has_method("bind"):
			c.bind(self)
	MatchInput.ensure_actions()


## Consumes one frame of match state (schema: docs/match_state.md).
func update_state(s: Dictionary) -> void:
	state = s
	local_player = s.get("local_player", 0)
	map_half = float(s.get("map", {}).get("size_m", 1024.0)) * 0.5
	squads_by_id.clear()
	squad_screen.clear()
	var vp := get_viewport().get_visible_rect()
	for sq in s.get("squads", []):
		var id: int = sq.id
		squads_by_id[id] = sq
		var sp: Variant = sq.get("screen")
		if sp == null and camera != null:
			var p: Vector3 = sq.pos + Vector3(0, 1.8, 0)
			if not camera.is_position_behind(p):
				sp = camera.unproject_position(p)
		if sp != null and vp.grow(40.0).has_point(sp):
			squad_screen[id] = sp
	selection.prune()
	for c in [top_bar, point_strip, minimap, call_in, cards, commands, units]:
		c.update_state(s)
	if overlay != null:
		overlay.update_state(s, self)
	for e in s.get("events", []):
		event_feed.push(e)
	_play_effects(s.get("effects", []))
	var result: Dictionary = s.get("result", {})
	if not ended and result.get("status", "running") != "running":
		ended = true
		selection.cancel_tool()
		pause_menu.close(false)
		end_screen.show_result(s)
		AudioManager.play_ui(&"confirm" if result.status == "won" else &"error")


func _play_effects(effects: Array) -> void:
	for fx in effects:
		match fx.get("kind", ""):
			"shot":
				var w: String = fx.get("weapon", "rifle")
				AudioManager.play_3d(StringName(w) if w in ["rifle", "mg", "smg"] else &"rifle", fx.pos)
			"mortar_launch":
				AudioManager.play_3d(&"mortar_launch", fx.pos)
			"explosion":
				AudioManager.play_3d(&"explosion", fx.pos)
			"dig":
				AudioManager.play_3d(&"dig", fx.pos)


## True while a modal (pause menu, end screen) owns input.
func input_blocked() -> bool:
	return pause_menu.visible or ended


func own_squad(id: int) -> bool:
	var sq: Dictionary = squads_by_id.get(id, {})
	return not sq.is_empty() and int(sq.owner) == local_player


func player(idx: int) -> Dictionary:
	var ps: Array = state.get("players", [])
	return ps[idx] if idx >= 0 and idx < ps.size() else {}


func rules() -> Dictionary:
	return state.get("rules", {})


func ground_y(x: float, z: float) -> float:
	return height_fn.call(x, z) if height_fn.is_valid() else 0.0


## World point under a screen position, or null off the map / above the
## horizon. Marches the camera ray against height_fn, then bisects.
func ground_pick(screen: Vector2) -> Variant:
	if camera == null:
		return null
	var o := camera.project_ray_origin(screen)
	var d := camera.project_ray_normal(screen)
	var t := 0.0
	var prev := 0.0
	var step := 2.0
	var hit := -1.0
	while t < 3000.0:
		var p := o + d * t
		if p.y <= ground_y(p.x, p.z):
			hit = t
			break
		prev = t
		t += step
		step = minf(step * 1.08, 16.0)
	if hit < 0.0:
		return null
	var lo := prev
	var hi := hit
	for i in 12:
		var mid := (lo + hi) * 0.5
		var pm := o + d * mid
		if pm.y <= ground_y(pm.x, pm.z):
			hi = mid
		else:
			lo = mid
	var g := o + d * hi
	if absf(g.x) > map_half or absf(g.z) > map_half:
		return null
	g.y = ground_y(g.x, g.z)
	return g


func jump_camera(pos: Vector3) -> void:
	if camera != null and camera.has_method("jump_to"):
		camera.jump_to(pos)


# --- intents ---------------------------------------------------------------

func emit_intent(kind: StringName, data: Dictionary) -> void:
	intent.emit(kind, data)
	var ids: PackedInt32Array = data.get("squads", PackedInt32Array())
	match kind:
		&"select":
			selection_changed.emit(ids)
		&"move":
			move_ordered.emit(ids, data.target)
		&"attack_move":
			attack_move_ordered.emit(ids, data.target)
		&"stop":
			stop_ordered.emit(ids)
		&"retreat":
			retreat_ordered.emit(ids)
		&"dig":
			dig_ordered.emit(ids, data.polyline)
		&"barrage":
			barrage_ordered.emit(ids, data.target)
		&"call_in":
			call_in_requested.emit(data.type)
		&"surrender":
			surrender_requested.emit()
		&"pause":
			paused = data.paused
			pause_requested.emit(data.paused)
		&"quit_to_menu":
			quit_to_menu_requested.emit()
