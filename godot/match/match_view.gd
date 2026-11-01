class_name MatchView
extends CanvasLayer
## The match presenter's single interface (S1, soldier-level since S1b).
## Feed it one plain Dictionary per frame with update_state(); it draws the
## HUD and the ground overlay, plays effect sounds, runs selection and order
## input, and reports what the player wants through signals. It never
## touches the sim.
##
## The state schema is in godot/docs/match_state.md. Every intent is emitted
## twice: as `intent(kind, data)` (one handler can map all of them to sim
## Commands) and as the specific signal below. Squad and soldier ids are
## sorted ascending in every PackedInt32Array, matching the sim's iteration
## order.
##
## Selection is a set of soldier ids (S1b-soldier-control.md). A state
## without `soldiers` (the S1 bridge) gets one stand-in soldier per squad,
## with the squad's id, so squad-level play keeps working until the bridge
## sends soldiers.
##
## Wiring (see match.gd):
##   view.camera = $Camera            # rts_camera.gd
##   view.height_fn = sim.get_height_at   # (x, z) -> ground y; optional
##   view.intent.connect(source.handle_intent)
##   func _process(_d): view.update_state(source.get_state())
##
## Net mode (S2) is on while the state carries a `net` Dictionary: the
## top bar shows the net readout, NetOverlay the desync banner and the
## waiting / paused-by panels, NetDisconnectScreen the reason the link
## ended. Positional orders get a pending marker from click time until
## the order executes (state `pending_orders`; docs/match_state.md).
##
## Vehicles (S3) arrive in state.vehicles. Own vehicle ids share the
## soldier id space and sit in the same selection; picking hits a vehicle
## inside its projected footprint or within 14 px of its centre. Observed
## main-gun and rocket impacts (state.armour_events) become short hit
## markers for 1.5 s, at most 6.

signal intent(kind: StringName, data: Dictionary)
signal selection_changed(squad_ids: PackedInt32Array)
signal soldier_selection_changed(unit_ids: PackedInt32Array)
signal move_ordered(squad_ids: PackedInt32Array, target: Vector3)
signal attack_move_ordered(squad_ids: PackedInt32Array, target: Vector3)
signal stop_ordered(squad_ids: PackedInt32Array)
signal retreat_ordered(squad_ids: PackedInt32Array)
signal dig_ordered(squad_ids: PackedInt32Array, polyline: PackedVector3Array)
signal barrage_ordered(squad_ids: PackedInt32Array, target: Vector3)
signal regroup_ordered(squad_ids: PackedInt32Array)
signal move_units_ordered(unit_ids: PackedInt32Array, target: Vector3)
signal attack_move_units_ordered(unit_ids: PackedInt32Array, target: Vector3)
signal stop_units_ordered(unit_ids: PackedInt32Array)
signal retreat_units_ordered(unit_ids: PackedInt32Array)
signal dig_units_ordered(unit_ids: PackedInt32Array, polyline: PackedVector3Array)
signal stance_ordered(unit_ids: PackedInt32Array, stance: String)
signal throw_ordered(unit_ids: PackedInt32Array, item: String, target: Vector3, each: bool)
signal reverse_units_ordered(unit_ids: PackedInt32Array, target: Vector3)
signal face_units_ordered(unit_ids: PackedInt32Array, target: Vector3)
signal attack_unit_ordered(unit_ids: PackedInt32Array, target: int)
signal set_ammo_ordered(unit_ids: PackedInt32Array, ammo: String)
signal repair_units_ordered(unit_ids: PackedInt32Array)
signal follow_units_ordered(unit_ids: PackedInt32Array, vehicle: int)
signal call_in_requested(squad_type: String)
signal surrender_requested
signal pause_requested(paused: bool)
signal quit_to_menu_requested

## Squad types that can dig / fire a barrage (S1 roster).
const DIGGERS := ["rifle", "engineer"]
const MORTARS := ["mortar"]
## Throwable items, in ability-row order.
const ITEMS := ["frag", "smoke", "satchel"]
## Throw range (min, max) in m when the state's rules do not say
## (S1b-abilities.md "Items").
const THROW_RANGE := {"frag": Vector2(5, 30), "smoke": Vector2(5, 30), "satchel": Vector2(3, 8)}
## Ground radius the throw tool previews at the target (m).
const THROW_EFFECT_M := {"frag": 6.0, "smoke": 8.0, "satchel": 10.0}
const STANCES := ["auto", "stand", "crouch", "prone"]
## Body-centre height above the feet per stance, for picking (m).
const PICK_LIFT := {"stand": 1.0, "crouch": 0.7, "prone": 0.25}
## Half a standing man's height above his pick point (m): the pick reach.
const BODY_HALF_M := 0.85
## Below this HP (of 100) a man shows as wounded.
const WOUNDED_HP := 60.0
## Names by squad type and slot when the state gives none.
const MAN_NAMES := {
	"rifle": ["Sgt (lead)", "Rifle 2", "Rifle 3", "Rifle 4", "Rifle 5", "Rifle 6", "Rifle 7", "Rifle 8"],
	"mg": ["Gunner", "Loader", "Carrier"],
	"engineer": ["Sgt (lead)", "Sapper 2", "Sapper 3", "Sapper 4", "Sapper 5"],
	"mortar": ["Gunner", "Loader", "Carrier"],
	"at": ["Gunner", "Loader"],
}
## Weapon by faction and squad type when the state gives none.
const WEAPONS := {
	"germany": {"rifle": "Kar98k", "mg": "MG42", "engineer": "MP40", "mortar": "GrW 34", "at": "Panzerschreck"},
	"usa": {"rifle": "M1 Garand", "mg": "M1919", "engineer": "M3", "mortar": "M1 mortar", "at": "Bazooka"},
}
const ORDER_TEXT := {
	"idle": "Holding", "move": "Moving", "attack_move": "Attacking", "retreat": "Retreating",
	"dig": "Digging", "barrage": "Firing", "setup": "Setting up", "hold": "Holding",
	"throw": "Throwing", "follow": "Escorting", "suppress": "Suppressing",
}
## Intents that get a pending-order marker in net mode, by marker kind.
const PENDING_KINDS := {
	&"move": "move", &"move_units": "move", &"attack_move": "attack_move",
	&"attack_move_units": "attack_move", &"barrage": "barrage", &"throw": "throw",
	&"throw_each": "throw", &"dig": "dig", &"dig_units": "dig",
	&"reverse_units": "move", &"face_units": "move", &"suppress_units": "throw",
}
## A click-time marker lives this long (s) unless the state's
## pending_orders confirms it; once confirmed the state's entry is drawn.
const LOCAL_PENDING_S := 0.5
## A state entry within this distance (m) of a click marker of the same
## kind confirms it.
const PENDING_MATCH_M := 3.0
## Short squad names for cards and tiles.
const SHORT_NAMES := {"rifle": "Rifle", "mg": "MG team", "engineer": "Engineers", "mortar": "Mortar", "at": "AT team"}
## Hit markers (S3): at most this many at once, each for HIT_TIME s.
const HIT_MAX := 6
const HIT_TIME := 1.5
## Vehicle centre for picking and markers: this far above the hull origin.
const VEHICLE_PICK_LIFT := 1.2
## World radius around a vehicle's pick point that holds its whole screen
## footprint (half diagonal of the largest hull, 3.3 m, plus the lift
## difference and a perspective margin): picks skip the footprint test
## outside it.
const VEHICLE_PICK_REACH_M := 5.5

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
## id -> soldier Dictionary (own and spotted enemy, living).
var soldiers_by_id: Dictionary = {}
## squad id -> PackedInt32Array of its living soldier ids, ascending.
var squad_units: Dictionary = {}
## True when the state had no `soldiers` (one stand-in per squad).
var proxy_soldiers := false
## Picking arrays, parallel: soldiers in front of the camera, their body
## centre on screen, camera distance, side (1 own, 0 enemy) and the
## screen length of half his body (px; head and feet are that far from the
## centre, so close up a click anywhere on him picks him).
var pick_ids := PackedInt32Array()
var pick_screen := PackedVector2Array()
var pick_depth := PackedFloat32Array()
var pick_own := PackedByteArray()
var pick_reach := PackedFloat32Array()
var paused := false
var ended := false
## Net mode (S2): the state's `net` Dictionary ({} when not networked).
var net: Dictionary = {}
var net_mode := false
## Pending-order markers to draw this frame: [{pos, kind, age}], from the
## state's pending_orders plus click-time markers not yet confirmed.
var pending_draw: Array = []
## Net mode: the slot whose 60 s "waiting for" timer ran down (the relay
## then surrenders for it), -1 none. The end screen says "timed out".
var net_timed_out_slot := -1
## S3 vehicles: id -> vehicle Dictionary (own, seen enemy and wrecks), and
## the match's vehicle types (rules.vehicle_types).
var vehicles_by_id: Dictionary = {}
var vehicle_types: Dictionary = {}
## Vehicle picking arrays, parallel (live vehicles in front of the camera):
## id, centre on screen, camera distance, side (1 own, 0 enemy).
var veh_pick_ids := PackedInt32Array()
var veh_pick_screen := PackedVector2Array()
var veh_pick_depth := PackedFloat32Array()
var veh_pick_own := PackedByteArray()
## Screen radius (px) of VEHICLE_PICK_REACH_M at each indexed vehicle.
var veh_pick_reach := PackedFloat32Array()
## Hit markers to draw: {word, pos, t (age s)}, oldest first.
var hit_markers: Array = []
## CPU time of the last vehicle index in microseconds (tests).
var vehicle_index_us := 0
var _pending_local: Array = []
var _clock := 0.0
var _veh_no := {}
## Stable per-type squad numbers for labels: id -> n, "owner:type" -> count.
var _squad_no := {}
var _type_count := {}

@onready var hud: Control = $Hud
@onready var units: Control = $UnitOverlay
@onready var top_bar: Control = $Hud/TopBar
@onready var minimap: Control = $Hud/Minimap
@onready var event_feed: Control = $Hud/EventFeed
@onready var roster: Control = $Hud/Roster
@onready var detail: Control = $Hud/Detail
@onready var orders: Control = $Hud/Orders
@onready var call_in: Control = $Hud/CallIn
@onready var pause_menu: Control = $PauseMenu
@onready var end_screen: Control = $EndScreen
@onready var selection: Node = $Selection
## Built in _ready (both match scenes get them without scene edits).
var net_readout: Control
var net_overlay: Control
var net_disconnect: Control
## Debug data-reload errors (S3b, DataErrorBanner).
var data_error: DataErrorBanner


func _ready() -> void:
	if camera == null and not camera_path.is_empty():
		camera = get_node_or_null(camera_path)
	if overlay == null and not overlay_path.is_empty():
		overlay = get_node_or_null(overlay_path)
	net_readout = NetReadout.new()
	net_readout.name = "NetReadout"
	hud.add_child(net_readout)
	net_overlay = NetOverlay.new()
	net_overlay.name = "NetOverlay"
	hud.add_child(net_overlay)
	data_error = DataErrorBanner.new()
	data_error.name = "DataError"
	hud.add_child(data_error)
	net_disconnect = NetDisconnectScreen.new()
	net_disconnect.name = "NetDisconnect"
	add_child(net_disconnect)
	# Above the HUD, below the pause menu and the end screen.
	move_child(net_disconnect, pause_menu.get_index())
	for c in [hud, top_bar, minimap, event_feed, roster, detail, orders, call_in, units, pause_menu, end_screen, selection, net_readout, net_overlay, net_disconnect, data_error]:
		if c.has_method("bind"):
			c.bind(self)
	MatchInput.ensure_actions()


## Consumes one frame of match state (schema: docs/match_state.md).
func update_state(s: Dictionary) -> void:
	state = s
	local_player = s.get("local_player", 0)
	_clock += get_process_delta_time()
	# Once the match is over the readout keeps its last values: the other
	# side stops talking after its goodbye, which would read "unstable".
	if not ended or not net_mode:
		net = s.get("net", {})
	net_mode = s.has("net")
	if net_mode and String(net.get("state", "")) == "waiting" and float(net.get("waiting_s_left", 99.0)) < 3.0:
		net_timed_out_slot = int(net.get("waiting_slot", -1))
	_update_pending(s)
	map_half = float(s.get("map", {}).get("size_m", 1024.0)) * 0.5
	squads_by_id.clear()
	squad_screen.clear()
	var vp := get_viewport().get_visible_rect()
	for sq in s.get("squads", []):
		var id: int = sq.id
		squads_by_id[id] = sq
		if not _squad_no.has(id):
			var key := "%d:%s" % [int(sq.owner), sq.type]
			_type_count[key] = int(_type_count.get(key, 0)) + 1
			_squad_no[id] = _type_count[key]
		var sp: Variant = sq.get("screen")
		if sp == null and camera != null:
			var p: Vector3 = sq.pos + Vector3(0, 1.8, 0)
			if not camera.is_position_behind(p):
				sp = camera.unproject_position(p)
		if sp != null and vp.grow(40.0).has_point(sp):
			squad_screen[id] = sp
	_index_soldiers(s, vp)
	_index_vehicles(s, vp)
	_note_hits(s.get("armour_events", []))
	selection.prune()
	hud.update_state(s)
	for c in [top_bar, minimap, call_in, roster, detail, orders, units]:
		c.update_state(s)
	if overlay != null:
		overlay.update_state(s, self)
	for e in s.get("events", []):
		event_feed.push(e)
	event_feed.update_state(s)
	for c in [net_readout, net_overlay, net_disconnect]:
		c.update_state(s)
	_play_effects(s.get("effects", []))
	var result: Dictionary = s.get("result", {})
	if not ended and result.get("status", "running") != "running":
		ended = true
		selection.cancel_tool()
		pause_menu.close(false)
		end_screen.show_result(s)
		AudioManager.play_ui(&"confirm" if result.status == "won" else &"error")


func _index_soldiers(s: Dictionary, vp: Rect2) -> void:
	soldiers_by_id.clear()
	squad_units.clear()
	proxy_soldiers = not s.has("soldiers")
	var list: Array = s.get("soldiers", []) if not proxy_soldiers else _proxy_soldiers()
	pick_ids.clear()
	pick_screen.clear()
	pick_depth.clear()
	pick_own.clear()
	pick_reach.clear()
	var cam_pos := camera.global_position if camera != null else Vector3.ZERO
	var area := vp.grow(20.0)
	for so in list:
		var id: int = so.id
		soldiers_by_id[id] = so
		var sid: int = so.squad
		if not squad_units.has(sid):
			squad_units[sid] = PackedInt32Array()
		squad_units[sid].append(id)
		if camera == null:
			continue
		var p: Vector3 = so.pos + Vector3(0, PICK_LIFT.get(so.get("stance", "stand"), 1.0), 0)
		if proxy_soldiers:
			p = so.pos + Vector3(0, 1.8, 0)
		if camera.is_position_behind(p):
			continue
		var sp := camera.unproject_position(p)
		if not area.has_point(sp):
			continue
		pick_ids.append(id)
		pick_screen.append(sp)
		pick_depth.append(cam_pos.distance_to(p))
		pick_own.append(1 if int(so.owner) == local_player else 0)
		var top := p + Vector3(0, BODY_HALF_M, 0)
		pick_reach.append(0.0 if camera.is_position_behind(top) else sp.distance_to(camera.unproject_position(top)))
	# Lists arrive ascending; keep squad_units ascending even if not.
	for sid in squad_units:
		squad_units[sid].sort()


## One stand-in soldier per squad (id = squad id) for states without
## `soldiers`: whole-squad selection only, squad-level intents only.
func _proxy_soldiers() -> Array:
	var out: Array = []
	for sq in state.get("squads", []):
		out.append({
			"id": sq.id, "squad": sq.id, "owner": sq.owner, "slot": 0, "pos": sq.pos,
			"hp": 100.0 * float(sq.get("health", 1.0)), "stance": "stand",
			"stance_override": "auto", "detached": false, "items": sq.get("items", {}),
			"pinned": sq.get("status", "") == "pinned",
		})
	return out


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
			"cannon":
				AudioManager.play_3d(&"cannon", fx.pos)
			"rocket_launch":
				AudioManager.play_3d(&"rocket_launch", fx.pos)
			"armour_hit":
				AudioManager.play_3d(&"ap_impact", fx.pos, -4.0 if fx.get("result", "") == "ricochet" else 0.0)
			"cook_off":
				AudioManager.play_3d(&"explosion", fx.pos, 3.0, 0.02)


## True while a modal (pause menu, end screen, net disconnect) owns input,
## and in net mode while the match is halted (paused, waiting, desync):
## orders given then would only queue up behind the pause.
func input_blocked() -> bool:
	return pause_menu.visible or ended or net_halted() or net_disconnect.visible


## Net mode: the relay is not closing turns (paused, waiting for a
## player, or stopped on a desync).
func net_halted() -> bool:
	return net_mode and String(net.get("state", "ok")) in ["paused", "waiting", "desync"]


# --- pending orders (net mode) -----------------------------------------------

func _note_pending(kind: StringName, data: Dictionary) -> void:
	if not net_mode or not PENDING_KINDS.has(kind):
		return
	var pos: Variant = data.get("target")
	if pos == null and data.has("polyline"):
		var pl: PackedVector3Array = data.polyline
		if not pl.is_empty():
			pos = pl[pl.size() / 2]
	if pos is Vector3:
		_pending_local.append({"pos": pos, "kind": PENDING_KINDS[kind], "t": _clock, "confirmed": false})


## pending_draw = the state's pending_orders (what the source says is sent
## but not executed) plus click-time markers it has not confirmed yet.
## A confirmed click marker is dropped for good, so it does not come back
## when the order executes and leaves the state's list.
func _update_pending(s: Dictionary) -> void:
	pending_draw.clear()
	if not net_mode:
		_pending_local.clear()
		return
	for p in s.get("pending_orders", []):
		var pos: Variant = p.get("pos") if p is Dictionary else null
		if not pos is Vector3:
			continue
		var kind := String(p.get("kind", "move"))
		kind = String(PENDING_KINDS.get(StringName(kind), kind))
		var age := 0.0
		for l in _pending_local:
			if l.kind == kind and Vector2(l.pos.x, l.pos.z).distance_to(Vector2(pos.x, pos.z)) <= PENDING_MATCH_M:
				l.confirmed = true
				age = maxf(age, _clock - float(l.t))
		pending_draw.append({"pos": pos, "kind": kind, "age": age})
	_pending_local = _pending_local.filter(func(l: Dictionary) -> bool: return not l.confirmed and _clock - float(l.t) < LOCAL_PENDING_S)
	for l in _pending_local:
		pending_draw.append({"pos": l.pos, "kind": l.kind, "age": _clock - float(l.t)})


## Indexes state.vehicles and projects the live ones for picking.
func _index_vehicles(s: Dictionary, vp: Rect2) -> void:
	var t0 := Time.get_ticks_usec()
	vehicles_by_id.clear()
	vehicle_types = s.get("rules", {}).get("vehicle_types", vehicle_types)
	veh_pick_ids.clear()
	veh_pick_screen.clear()
	veh_pick_depth.clear()
	veh_pick_own.clear()
	veh_pick_reach.clear()
	var cam_pos := camera.global_position if camera != null else Vector3.ZERO
	var cam_right := camera.global_transform.basis.x * VEHICLE_PICK_REACH_M if camera != null else Vector3.ZERO
	var area := vp.grow(60.0)
	for v in s.get("vehicles", []):
		var id: int = v.id
		vehicles_by_id[id] = v
		if not _veh_no.has(id) and int(v.owner) >= 0:
			var key := "%d:v:%s" % [int(v.owner), v.type]
			_type_count[key] = int(_type_count.get(key, 0)) + 1
			_veh_no[id] = _type_count[key]
		if camera == null or Vehicles.is_wreck(v):
			continue
		var p: Vector3 = v.pos + Vector3(0, VEHICLE_PICK_LIFT, 0)
		if camera.is_position_behind(p):
			continue
		var sp := camera.unproject_position(p)
		if not area.has_point(sp):
			continue
		veh_pick_ids.append(id)
		veh_pick_screen.append(sp)
		veh_pick_depth.append(cam_pos.distance_to(p))
		veh_pick_own.append(1 if int(v.owner) == local_player else 0)
		veh_pick_reach.append(sp.distance_to(camera.unproject_position(p + cam_right)))
	vehicle_index_us = Time.get_ticks_usec() - t0


## Turns observed main-gun and rocket impacts into hit markers.
func _note_hits(events: Array) -> void:
	var dt := get_process_delta_time()
	for h in hit_markers:
		h.t += dt
	hit_markers = hit_markers.filter(func(h: Dictionary) -> bool: return h.t < HIT_TIME)
	for e in events:
		var word: String = Vehicles.HIT_WORDS.get(String(e.get("kind", "")), "")
		if word == "" or not e.get("pos") is Vector3:
			continue
		hit_markers.append({"word": word, "pos": e.pos, "t": 0.0, "kind": String(e.kind), "own": int(e.get("owner", -1)) == local_player})
	while hit_markers.size() > HIT_MAX:
		hit_markers.pop_front()


func is_vehicle(id: int) -> bool:
	return vehicles_by_id.has(id)


## An own vehicle that is not a wreck.
func own_vehicle(id: int) -> bool:
	var v: Dictionary = vehicles_by_id.get(id, {})
	return not v.is_empty() and int(v.owner) == local_player and not Vehicles.is_wreck(v)


## An own living soldier or vehicle (what the selection may hold).
func own_unit(id: int) -> bool:
	return own_soldier(id) or own_vehicle(id)


## Where a selected unit stands (soldier feet or hull origin).
func unit_pos(id: int) -> Vector3:
	if soldiers_by_id.has(id):
		return soldiers_by_id[id].pos
	if vehicles_by_id.has(id):
		return vehicles_by_id[id].pos
	return Vector3.ZERO


func vehicle_type(id: int) -> Dictionary:
	var v: Dictionary = vehicles_by_id.get(id, {})
	return vehicle_types.get(String(v.get("type", "")), {})


## Card label of a vehicle: its short type name, numbered when the side
## has had several of that type ("Panzer IV 2"); long form adds the full
## name.
func vehicle_label(id: int, long := false) -> String:
	var v: Dictionary = vehicles_by_id.get(id, {})
	if v.is_empty():
		return ""
	var vt := vehicle_type(id)
	var base := Vehicles.short_name(vt, String(v.get("name", "Vehicle")))
	var key := "%d:v:%s" % [int(v.owner), v.type]
	var label := "%s %d" % [base, _veh_no.get(id, 1)] if int(_type_count.get(key, 0)) > 1 else base
	var full := String(v.get("name", vt.get("name", base)))
	return "%s · %s" % [label, full] if long and full != base else label


## Projected footprint of a vehicle (4 screen points), empty when any
## corner is behind the camera.
func vehicle_screen_footprint(id: int) -> PackedVector2Array:
	var v: Dictionary = vehicles_by_id.get(id, {})
	var out := PackedVector2Array()
	if v.is_empty() or camera == null:
		return out
	for c in Vehicles.footprint(v.pos, float(v.yaw), vehicle_type(id)):
		var p: Vector3 = c + Vector3(0, 0.4, 0)
		if camera.is_position_behind(p):
			return PackedVector2Array()
		out.append(camera.unproject_position(p))
	return out


func own_squad(id: int) -> bool:
	var sq: Dictionary = squads_by_id.get(id, {})
	return not sq.is_empty() and int(sq.owner) == local_player


func own_soldier(id: int) -> bool:
	var so: Dictionary = soldiers_by_id.get(id, {})
	return not so.is_empty() and int(so.owner) == local_player


func soldier_squad(id: int) -> int:
	var so: Dictionary = soldiers_by_id.get(id, {})
	return int(so.get("squad", -1))


func squad_type(squad_id: int) -> String:
	return String(squads_by_id.get(squad_id, {}).get("type", ""))


## Card label: short type name, numbered when the side has had several of
## that type ("Rifle 2"). Numbers are given in order of first sighting and
## never reused, so a card keeps its name when another squad dies.
func squad_label(squad_id: int, long := false) -> String:
	var sq: Dictionary = squads_by_id.get(squad_id, {})
	if sq.is_empty():
		return ""
	var base: String = SHORT_NAMES.get(sq.type, sq.get("name", sq.type))
	var key := "%d:%s" % [int(sq.owner), sq.type]
	var label := "%s %d" % [base, _squad_no.get(squad_id, 1)] if int(_type_count.get(key, 0)) > 1 else base
	# Long form: the card's label first, so card and detail panel name the
	# squad the same way, then the unit's own name ("Rifle 1 · Schützengruppe").
	var full: String = sq.get("name", UiPalette.SQUAD_NAMES.get(sq.type, sq.type))
	return "%s · %s" % [label, full] if long and full != base else label


## Card status: [text, alarm]; alarm (accent) for under fire and pinned.
func squad_status(sq: Dictionary) -> Array:
	match sq.get("status", "ok"):
		"pinned":
			return ["Pinned", true]
		"suppressed":
			return ["Under fire", true]
		"heads_down":
			return ["Heads down", true]
		"retreating":
			return ["Retreating", false]
	var o: String = sq.get("order", "idle")
	return [ORDER_TEXT.get(o, o.capitalize()), false]


func soldier_name(so: Dictionary) -> String:
	if so.has("name"):
		return so.name
	var names: Array = MAN_NAMES.get(squad_type(int(so.get("squad", -1))), [])
	var slot := int(so.get("slot", 0))
	return names[slot] if slot >= 0 and slot < names.size() else "Man %d" % (slot + 1)


func soldier_weapon(so: Dictionary) -> String:
	if so.has("weapon"):
		return so.weapon
	var faction: String = player(int(so.get("owner", 0))).get("faction", "germany")
	return WEAPONS.get(faction, {}).get(squad_type(int(so.get("squad", -1))), "")


func soldier_pinned(so: Dictionary) -> bool:
	return bool(so.get("pinned", false))


func soldier_wounded(so: Dictionary) -> bool:
	return float(so.get("hp", 100.0)) < WOUNDED_HP


func item_count(so: Dictionary, item: String) -> int:
	return int(so.get("items", {}).get(item, 0))


## (min, max) throw range in m for an item.
func throw_range(item: String) -> Vector2:
	var r: Variant = rules().get("throw_range_m", {}).get(item)
	return r if r is Vector2 else THROW_RANGE.get(item, Vector2(5, 30))


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
	_note_pending(kind, data)
	intent.emit(kind, data)
	var ids: PackedInt32Array = data.get("squads", PackedInt32Array())
	var uids: PackedInt32Array = data.get("units", PackedInt32Array())
	match kind:
		&"select":
			selection_changed.emit(ids)
			soldier_selection_changed.emit(uids)
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
		&"regroup":
			regroup_ordered.emit(ids)
		&"move_units":
			move_units_ordered.emit(uids, data.target)
		&"attack_move_units":
			attack_move_units_ordered.emit(uids, data.target)
		&"stop_units":
			stop_units_ordered.emit(uids)
		&"retreat_units":
			retreat_units_ordered.emit(uids)
		&"dig_units":
			dig_units_ordered.emit(uids, data.polyline)
		&"set_stance":
			stance_ordered.emit(uids, data.stance)
		&"throw", &"throw_each":
			throw_ordered.emit(uids, data.item, data.target, kind == &"throw_each")
		&"reverse_units":
			reverse_units_ordered.emit(uids, data.target)
		&"face_units":
			face_units_ordered.emit(uids, data.target)
		&"attack_unit":
			attack_unit_ordered.emit(uids, int(data.target))
		&"set_ammo":
			set_ammo_ordered.emit(uids, data.ammo)
		&"repair_units":
			repair_units_ordered.emit(uids)
		&"follow_units":
			follow_units_ordered.emit(uids, int(data.vehicle))
		&"call_in":
			call_in_requested.emit(data.type)
		&"surrender":
			surrender_requested.emit()
		&"pause":
			paused = data.paused
			pause_requested.emit(data.paused)
		&"quit_to_menu":
			quit_to_menu_requested.emit()
