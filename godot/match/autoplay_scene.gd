extends Node
## Staged S3 autoplay scenes (S3 "Done when" 7): `--autoplay_scene
## armour_<name>` starts the match scene on a staged world from
## `frontline_sim::scenarios` (FrontlineMatch.start_match({scene: name})),
## frames it, waits for its snapshot condition, saves
## `s3_<name>.png` into Session.autoplay_dir() and quits. It exits with 0
## only when both the sim's condition (`state.scene.ready_tick`, the
## sim's own `scenarios::scene_ready`) and this script's check on the
## presenter state (below) held when the picture was taken; otherwise it
## prints why and exits with 1.
##
## The checks are on what the presenter was given, so they also cover the
## bridge: the vehicle in `state.vehicles` with the flag set, the effect
## or armour event that drew the picture, the locked call-in tile.

## Wall-clock seconds before a scene gives up.
const TIMEOUT_S := 75.0

## Per scene: camera (height, pitch, yaw toward), extra seconds to wait
## after the condition first holds (effects build up, the tank drives),
## and what the presenter state must show.
const SCENES := {
	"callin_locked": {"height": 70.0, "pitch": -48.0, "settle": 1.5},
	"vehicle_card": {"height": 20.0, "pitch": -36.0, "yaw": -60.0, "settle": 1.5},
	"tank_firing": {"height": 14.0, "pitch": -20.0, "yaw": -97.0, "settle": 0.0},
	"penetration_marker": {"height": 22.0, "pitch": -36.0, "yaw": -55.0, "settle": 0.1},
	"bogged_in_trench": {"height": 22.0, "pitch": -38.0, "yaw": -120.0, "settle": 1.5},
	"at_ditch": {"height": 30.0, "pitch": -40.0, "yaw": -90.0, "settle": 1.5},
	"at_team_firing": {"height": 34.0, "pitch": -38.0, "yaw": 0.0, "settle": 0.15},
	"burning": {"height": 22.0, "pitch": -34.0, "yaw": -40.0, "settle": 2.5},
	"escort": {"height": 34.0, "pitch": -42.0, "yaw": 60.0, "settle": 9.0},
	# S3c: the abandoned tank and its crew squad (selected: the card reads
	# "Crew — Panzer IV").
	"bailout": {"height": 16.0, "pitch": -32.0, "yaw": -150.0, "settle": 1.5},
	# S6: the HQ squad selected (its card's strip: the HQ's stock bars),
	# the radius ring, the bearers' card with its crate tag.
	"hq_supply": {"height": 26.0, "pitch": -40.0, "yaw": -20.0, "settle": 3.0},
	# S7: a StuG III (casemate blockout) and an M10 (open-topped turret).
	"casemate_td": {"height": 11.0, "pitch": -34.0, "yaw": -35.0, "settle": 1.0},
	# S8a: a loaded SdKfz 251 and a loaded M3A1 (open compartments, the
	# MG shields, seated riders); the 251 selected (its card lists the
	# passengers).
	"halftracks": {"height": 14.0, "pitch": -40.0, "yaw": -20.0, "settle": 1.5},
}

var m: Node3D
var view: MatchView
var source: Node
var camera: Camera3D
var scene := ""
var conf: Dictionary = {}
var focus := Vector3.ZERO
var wall := 0.0
var _armed_at := -1.0
var _staged := false
var _done := false
## What this script saw in the state (cumulative: events and effects come
## once).
var _seen := {}


func begin(match_root: Node3D) -> void:
	m = match_root
	view = m.view
	source = m.source
	camera = m.camera
	scene = Session.autoplay_scene
	conf = SCENES.get(scene, {})
	var info: Dictionary = source.get_map_info()
	focus = info.get("scene_focus", Vector3.ZERO)
	camera.input_enabled = false
	source.set_speed(1.0)
	_frame(focus)
	print("autoplay_scene: %s, focus %s" % [scene, focus])


func _frame(at: Vector3) -> void:
	camera.height = float(conf.get("height", 40.0))
	camera.pitch_deg = float(conf.get("pitch", -45.0))
	if conf.has("yaw"):
		camera.yaw_deg = float(conf.yaw)
	camera.jump_to(at)


func on_state(s: Dictionary) -> void:
	if _done:
		return
	wall += get_process_delta_time()
	_observe(s)
	# Staging waits for what it selects (the first commands run a tick
	# or two after the start), at most a few seconds.
	if not _staged and s.get("vehicles", []).size() + s.get("squads", []).size() > 0:
		_staged = _stage(s) or wall > 4.0
	var sc: Dictionary = s.get("scene", {})
	var ready := int(sc.get("ready_tick", -1)) >= 0 and _check(s) == ""
	if ready and _armed_at < 0.0:
		_armed_at = wall
		print("autoplay_scene: %s ready at tick %d (sim tick %d)" % [scene, int(sc.ready_tick), int(s.get("tick", 0))])
	if _armed_at >= 0.0:
		_follow(s)
		if wall - _armed_at >= float(conf.get("settle", 1.0)):
			_capture(s)
		return
	var late: bool = int(s.get("tick", 0)) > int(sc.get("max_ticks", 900)) + 90
	if wall > TIMEOUT_S or late:
		_fail("condition never held (tick %d, sim ready_tick %d, state check: %s)" % [
			int(s.get("tick", 0)), int(sc.get("ready_tick", -1)), _check(s)])


## Selection and camera once the first state with units is in. False:
## not there yet, try again next frame.
func _stage(s: Dictionary) -> bool:
	var own := _own_vehicles(s)
	match scene:
		"halftracks":
			# The 251 selected (its card lists the passengers), both in
			# frame from the scene's focus between them.
			if not own.is_empty():
				view.selection.select_only(PackedInt32Array([int(own[0].id)]))
				view.selection.focus_squad = int(own[0].squad)
		"vehicle_card", "bogged_in_trench", "burning":
			if not own.is_empty():
				var v: Dictionary = own[0]
				view.selection.select_only(PackedInt32Array([int(v.id)]))
				view.selection.focus_squad = int(v.squad)
				_frame(v.pos)
		"tank_firing":
			# Just behind our tank, looking down the lane at the enemy.
			if not own.is_empty():
				var v: Dictionary = own[0]
				_frame(v.pos + Vector3(10, 0, 0))
				view.selection.select_only(PackedInt32Array([int(v.id)]))
		"penetration_marker":
			# Our tank, hit in the flank from the west.
			if not own.is_empty():
				_frame(own[0].pos)
		"at_ditch":
			# From the tank's side of the ditch, looking west across it.
			var eng := _own_squads(s, "engineer")
			if not eng.is_empty():
				view.selection.select_squads(PackedInt32Array([int(eng[0].id)]))
			_frame(focus + Vector3(6, 0, 0))
		"at_team_firing":
			# From the south: the team in its trench on the left, the tank
			# 70 m east on the right.
			# (The scene puts the tank 70 m east of the team.)
			var at := _own_squads(s, "at")
			if not at.is_empty():
				view.selection.select_squads(PackedInt32Array([int(at[0].id)]))
				_frame(at[0].pos + Vector3(35, 0, 0))
		"escort":
			for sq in _own_squads(s, "rifle"):
				if int(sq.get("follow", -1)) >= 0:
					view.selection.select_squads(PackedInt32Array([int(sq.id)]))
					return true
			return false
		"callin_locked":
			view.selection.select_only(PackedInt32Array())
		"hq_supply":
			for sq in s.get("squads", []):
				if int(sq.owner) == int(s.get("local_player", 0)) and PackedStringArray(sq.get("tags", PackedStringArray())).has("hq"):
					view.selection.select_squads(PackedInt32Array([int(sq.id)]))
					return true
			return false
		"bailout":
			# Wait for the crew squad; select it and frame the tank.
			for sq in _own_squads(s, "crew"):
				if sq.has("crew_of"):
					view.selection.select_squads(PackedInt32Array([int(sq.id)]))
					var v: Dictionary = view.vehicles_by_id.get(int(sq.crew_of), {})
					if not v.is_empty():
						_frame(v.pos)
					return true
			return false
	return true


## Keeps a moving subject in frame while waiting to capture.
func _follow(s: Dictionary) -> void:
	if scene != "escort":
		return
	var own := _own_vehicles(s)
	if not own.is_empty():
		camera.jump_to(own[0].pos + Vector3(10, 0, 0))


## Remembers effects and events (they come once).
func _observe(s: Dictionary) -> void:
	for fx in s.get("effects", []):
		var k := String(fx.get("kind", ""))
		if k == "cannon" or k == "rocket_launch" or k == "armour_hit":
			_seen[k] = true
		if k == "cannon":
			for v in _own_vehicles(s):
				if v.pos.distance_to(fx.pos) < 8.0:
					_seen["own_cannon"] = true
	for e in s.get("armour_events", []):
		_seen["ev_" + String(e.get("kind", ""))] = true
	for sh in s.get("shells", []):
		if String(sh.get("kind", "")) == "heat" and int(sh.get("owner", -1)) == int(s.get("local_player", 0)):
			_seen["own_rocket"] = true


## "" when the presenter state shows the scene's condition, else why not.
func _check(s: Dictionary) -> String:
	var own := _own_vehicles(s)
	match scene:
		"callin_locked":
			for c in s.get("callin", {}).get("catalog", []):
				if String(c.get("type", "")) == "vehicle" and float(c.get("locked_s", 0.0)) > 0.0:
					return ""
			return "no locked vehicle tile in callin.catalog"
		"vehicle_card":
			for v in own:
				if String(v.get("ammo_choice", "")) == "he":
					return ""
			return "no own vehicle with ammo_choice he"
		"tank_firing":
			return "" if _seen.has("own_cannon") and _seen.has("ev_fired") else "no cannon effect at our tank yet"
		"penetration_marker":
			return "" if _seen.has("ev_pen") and _seen.has("armour_hit") else "no pen event yet"
		"bogged_in_trench":
			return _flag(own, Vehicles.BOGGED, "BOGGED")
		"burning":
			return _flag(own, Vehicles.BURNING, "BURNING")
		"bailout":
			for v in s.get("vehicles", []):
				if Vehicles.is_abandoned(v) and int(v.get("inside", 1)) == 0 and int(v.get("crew_squad", -1)) >= 0:
					for sq in _own_squads(s, "crew"):
						if int(sq.get("crew_of", -1)) == int(v.id) and int(sq.get("men", 0)) >= 3:
							return ""
					return "abandoned tank but no crew squad card link"
			return "no abandoned tank with all men out"
		"at_ditch":
			var d := 0.0
			if source.has_method("dug_depth_at"):
				d = source.dug_depth_at(focus.x, focus.z)
			return "" if d > 1.5 else "AT ditch not dug at the focus (%.2f m)" % d
		"at_team_firing":
			return "" if _seen.has("own_rocket") and _seen.has("rocket_launch") else "no own rocket in flight yet"
		"hq_supply":
			var hq := false
			for st in s.get("structures", []):
				if int(st.owner) == int(s.get("local_player", 0)) and bool(st.get("built", false)) and float(st.stock_kg.get("shells", 0.0)) > 0.0:
					hq = true
			if not hq:
				return "no built own HQ with shells in structures"
			for so in s.get("soldiers", []):
				if so.has("crate"):
					return ""
			return "no soldier carries a crate"
		"escort":
			for v in own:
				if int(v.get("followers", 0)) > 0:
					for sq in s.get("squads", []):
						if int(sq.get("follow", -1)) == int(v.id):
							return ""
					return "followers but no squads[].follow link"
			return "no own vehicle with followers"
		"halftracks":
			var loaded := 0
			for v in s.get("vehicles", []):
				if int(v.get("riders", 0)) > 0 and int(v.get("capacity", 0)) > 0:
					loaded += 1
			return "" if loaded >= 2 else "%d loaded half-tracks drawn" % loaded
		"casemate_td":
			var vt: Dictionary = s.get("rules", {}).get("vehicle_types", {})
			var kinds := {}
			for v in s.get("vehicles", []):
				var t: Dictionary = vt.get(String(v.type), {})
				if bool(t.get("casemate", false)):
					kinds["casemate"] = true
				if bool(t.get("open_top", false)):
					kinds["open_top"] = true
			return "" if kinds.size() == 2 else "no casemate and open-topped vehicle drawn: %s" % kinds.keys()
	return "unknown scene %s" % scene


func _flag(vs: Array, bit: int, name: String) -> String:
	for v in vs:
		if Vehicles.has(v, bit):
			return ""
	return "no own vehicle with %s" % name


func _capture(s: Dictionary) -> void:
	_done = true
	# Persistent conditions must still hold in the picture.
	var why := _check(s)
	if why != "":
		_fail("condition lost before the capture: %s" % why)
		return
	var path := Session.autoplay_dir().path_join("s3_%s.png" % scene)
	var err: int = await Session.save_screenshot(path)
	var st: Dictionary = source.get_stats()
	print("autoplay_scene: %s captured at tick %d (own vehicles %d, enemy %d, wrecks %d, shells %d)" % [
		scene, int(s.get("tick", 0)), int(st.get("vehicles_own", 0)), int(st.get("vehicles_enemy", 0)),
		int(st.get("wrecks", 0)), int(st.get("shells", 0))])
	Session.quit_game(0 if err == OK else 1)


func _fail(why: String) -> void:
	_done = true
	push_error("autoplay_scene %s: %s" % [scene, why])
	Session.quit_game(1)


func _own_vehicles(s: Dictionary) -> Array:
	var local := int(s.get("local_player", 0))
	return s.get("vehicles", []).filter(func(v: Dictionary) -> bool: return int(v.owner) == local)


func _enemy_vehicles(s: Dictionary) -> Array:
	var local := int(s.get("local_player", 0))
	return s.get("vehicles", []).filter(func(v: Dictionary) -> bool: return int(v.owner) != local and int(v.owner) >= 0)


func _own_squads(s: Dictionary, type: String) -> Array:
	var local := int(s.get("local_player", 0))
	return s.get("squads", []).filter(func(sq: Dictionary) -> bool: return int(sq.owner) == local and String(sq.type) == type)
