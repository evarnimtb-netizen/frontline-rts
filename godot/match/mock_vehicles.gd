class_name MockVehicles
extends RefCounted
## S3 part of the mock match (MockMatchSource owns one): tanks and wrecks,
## main-gun shells and AT rockets in flight, armour events, and the S3
## state keys (docs/match_state.md "Vehicles (S3)"). A crude stand-in for
## the sim, like the rest of the mock: tanks drive straight at their
## target turning at the hull rate, lay the turret at the traverse rate,
## fire when laid and loaded, and every hit rolls a result from fixed
## odds. Bogging, fire, repair, crew swaps and knock-outs follow the S3
## timings closely enough to show every vehicle state.
##
## Presenter-only and non-deterministic on purpose; nothing here may be
## copied into crates/sim.

## Vehicle types of the match (rules.vehicle_types), keyed by type key.
const TYPES := {
	"ger_pz4_h": {
		"id": "ww2:ger_pz4_h", "name": "Panzer IV Ausf. H", "short": "Panzer IV", "faction": "germany",
		"role": "tank", "cost": 640, "pop": 12, "recall_s": 120.0, "speed": 6.0, "sight_m": 175.0,
		"hull_length": 5.8, "hull_width": 2.8, "hull_height": 1.36, "hull_ground": 0.40,
		"turret_x": 0.20, "turret_base": 1.76, "turret_length": 2.3, "turret_width": 1.9,
		"turret_height": 0.9, "turret_offset_x": 0.0, "gun_length": 4.5, "gun_height": 0.45,
		"seats": ["driver", "gunner", "commander", "loader", "radio_mg"],
		"ammo": [{"kind": "ap", "name": "7.5 cm PzGr 39", "max": 30}, {"kind": "he", "name": "7.5 cm Sprgr 34", "max": 30}],
	},
	"usa_m4a3_75": {
		"id": "ww2:usa_m4a3_75", "name": "M4A3 (75) Sherman", "short": "Sherman", "faction": "usa",
		"role": "tank", "cost": 560, "pop": 12, "recall_s": 120.0, "speed": 6.5, "sight_m": 175.0,
		"hull_length": 5.9, "hull_width": 2.62, "hull_height": 1.5, "hull_ground": 0.43,
		"turret_x": 0.10, "turret_base": 1.93, "turret_length": 2.2, "turret_width": 1.8,
		"turret_height": 0.85, "turret_offset_x": -0.1, "gun_length": 3.9, "gun_height": 0.38,
		"seats": ["driver", "gunner", "commander", "loader", "radio_mg"],
		"ammo": [{"kind": "ap", "name": "75 mm M61", "max": 30}, {"kind": "he", "name": "75 mm M48", "max": 40}],
	},
	# S7: one casemate type and one open-topped type (the sim's StuG III G
	# and M10; dims from their defs, the casemate block from the gun pivot).
	"ger_stug3_g": {
		"id": "ww2:ger_stug3_g", "name": "StuG III Ausf. G", "short": "StuG III", "faction": "germany",
		"role": "assault_gun", "cost": 500, "pop": 10, "recall_s": 120.0, "speed": 6.0, "sight_m": 160.0,
		"hull_length": 5.4, "hull_width": 2.9, "hull_height": 1.04, "hull_ground": 0.38,
		"turret_x": -1.10, "turret_base": 1.65, "turret_length": 3.0, "turret_width": 2.2,
		"turret_height": 0.66, "turret_offset_x": -0.7, "gun_length": 3.0, "gun_height": 0.0,
		"casemate": true, "open_top": false, "turret_floor": -0.23,
		"seats": ["driver", "gunner", "commander", "loader"],
		"ammo": [{"kind": "ap", "name": "7.5 cm PzGr 39", "max": 27}, {"kind": "he", "name": "7.5 cm Sprgr 34", "max": 24}],
	},
	"usa_m10": {
		"id": "ww2:usa_m10", "name": "3-inch GMC M10", "short": "M10", "faction": "usa",
		"role": "tank_destroyer", "cost": 520, "pop": 10, "recall_s": 120.0, "speed": 7.4, "sight_m": 175.0,
		"hull_length": 5.8, "hull_width": 3.0, "hull_height": 1.4, "hull_ground": 0.5,
		"turret_x": 0.20, "turret_base": 1.90, "turret_length": 2.2, "turret_width": 2.1,
		"turret_height": 0.66, "turret_offset_x": 0.0, "gun_length": 3.9, "gun_height": 0.35,
		"casemate": false, "open_top": true, "turret_floor": 0.0,
		"seats": ["driver", "gunner", "commander", "loader", "radio_mg"],
		"ammo": [{"kind": "ap", "name": "3-inch M62 APC", "max": 36}, {"kind": "he", "name": "3-inch M42A1 HE", "max": 18}],
	},
}
## Mock-only handling numbers (not part of the state).
const HANDLING := {
	"ger_pz4_h": {"turn": 30.0, "traverse": 14.0, "reverse": 2.0, "reload": 6.5, "elev": Vector2(-8, 20), "ap_speed": 790.0, "he_speed": 550.0},
	"usa_m4a3_75": {"turn": 32.0, "traverse": 24.0, "reverse": 2.5, "reload": 6.0, "elev": Vector2(-10, 25), "ap_speed": 618.0, "he_speed": 463.0},
	# S7: the casemate gun lays only within "arc" (deg) of the nose.
	"ger_stug3_g": {"turn": 30.0, "traverse": 6.0, "arc": 10.0, "reverse": 2.0, "reload": 6.5, "elev": Vector2(-6, 20), "ap_speed": 790.0, "he_speed": 550.0},
	"usa_m10": {"turn": 32.0, "traverse": 5.0, "reverse": 2.5, "reload": 6.0, "elev": Vector2(-10, 19), "ap_speed": 792.0, "he_speed": 579.0},
}
const FACTION_TANK := {"germany": "ger_pz4_h", "usa": "usa_m4a3_75"}
## S7: each faction's second vehicle card (a call-in tile of its own).
const FACTION_TD := {"germany": "ger_stug3_g", "usa": "usa_m10"}
const AT_TEAM := {
	"germany": {"name": "Panzerschreck team", "short": "Schreck", "cost": 200, "speed": 110.0, "range": 120.0},
	"usa": {"name": "Bazooka team", "short": "Bazooka", "cost": 180, "speed": 85.0, "range": 100.0},
}
const UNLOCK_S := 240.0
const LIMIT := 4
const WRECK_CAP := 32
const ENGAGE_M := 600.0
## Vehicles are big: spotted at twice the observer's sight (S3 "Spotting").
const SPOT_MULT := 2.0
const SEAT_SWAP_S := 4.0
const FIRE_KO_S := 20.0
const KO_SMOKE_S := 60.0
const ZONES := ["hull_front", "hull_side", "hull_rear", "turret_front", "turret_side", "turret_rear"]

## The owning MockMatchSource (untyped: the two scripts refer to each other).
var src
## id -> internal vehicle Dictionary (also wrecks).
var vehicles := {}
## Shells and rockets in flight: {id, kind, owner, shooter, pos, vel, t, target, aim}.
var shells: Array[Dictionary] = []
var unlock_s := UNLOCK_S
var limit := LIMIT
## Followers per vehicle, counted by the source each step.
var followers := {}
var _armour: Array[Dictionary] = []
var _seq := 0
var _shell_id := 1
## Staging: no feed lines for staged knock-outs.
var _quiet := false


func _init(source: Node) -> void:
	src = source


func reset() -> void:
	vehicles.clear()
	shells.clear()
	_armour.clear()
	followers.clear()
	_seq = 0
	unlock_s = UNLOCK_S


# --- spawning and call-in ----------------------------------------------------

## A new vehicle of `type_key` for `owner` at a ground point, facing `yaw`.
func spawn(owner: int, type_key: String, at: Vector2, yaw := 0.0) -> int:
	var id: int = src._next_man
	src._next_man += 1
	var sid: int = src._next_id
	src._next_id += 1
	var vt: Dictionary = TYPES[type_key]
	vehicles[id] = {
		"id": id, "squad": sid, "owner": owner, "last_owner": owner, "type": type_key,
		"pos": at, "yaw": yaw, "turret_yaw": 0.0, "gun_pitch": 0.0, "speed": 0.0,
		"order": "idle", "target": at, "face_yaw": yaw,
		"crew": {"driver": "ok", "gunner": "ok", "commander": "ok", "loader": "ok", "radio_mg": "ok"},
		"swap": {},
		"comp": {"engine": 1.0, "track_l": 1.0, "track_r": 1.0, "gun": 1.0, "ring": 1.0, "ammo_rack": 1.0, "transmission": 1.0},
		"repairing": "", "ammo": {"ap": int(vt.ammo[0].max), "he": int(vt.ammo[1].max), "mg": 2950, "mg_full": 3150},
		"loaded": "ap", "ammo_choice": "auto", "reload": 1.0,
		"aim": -1, "aim_squad": -1, "attack": -1, "focus_shots": 0,
		"bog_t": 0.0, "crossing": false, "burn_t": -1.0, "fire_roll_t": 4.0,
		"wreck_t": -1.0, "hold_t": 0.0,
	}
	return id


## Live vehicles of a player (the "2/4" badge and the limit; S3b: a
## bought vehicle is alive at once, nothing is queued).
func count(owner: int) -> int:
	var n := 0
	for id in vehicles:
		if int(vehicles[id].owner) == owner:
			n += 1
	return n


func pop(owner: int) -> int:
	var n := 0
	for id in vehicles:
		var v: Dictionary = vehicles[id]
		if int(v.owner) == owner:
			n += int(TYPES[v.type].pop)
	return n


func locked_s() -> float:
	return maxf(unlock_s - src.time_s, 0.0)


## Why `owner` may not call in a vehicle now ("" = may).
func call_in_block(owner: int) -> String:
	if locked_s() > 0.0:
		return "locked"
	if count(owner) >= limit:
		return "limit"
	return ""


func faction_tank(owner: int) -> String:
	return FACTION_TANK.get(String(src.players[owner].faction), "ger_pz4_h")


## S7: the faction's assault gun or tank destroyer.
func faction_td(owner: int) -> String:
	return FACTION_TD.get(String(src.players[owner].faction), "ger_stug3_g")


# --- intents -----------------------------------------------------------------

## Own living vehicles among `ids`.
func _own(ids: PackedInt32Array) -> Array:
	var out: Array = []
	for id in ids:
		var v: Dictionary = vehicles.get(id, {})
		if not v.is_empty() and int(v.owner) == src.local_player:
			out.append(v)
	return out


## Applies the vehicle part of an intent (ids in `units`). Soldier ids are
## the source's business.
func handle_intent(kind: StringName, data: Dictionary) -> void:
	var uids: PackedInt32Array = data.get("units", PackedInt32Array())
	var own := _own(uids)
	match kind:
		&"move_units", &"attack_move_units":
			var t: Vector3 = data.target
			var n := 0
			for v in own:
				var off := Vector2((n % 2) * 10.0 - 5.0, floorf(n / 2.0) * 10.0) if own.size() > 1 else Vector2.ZERO
				var goal := Vector2(t.x, t.z) + off
				# A Move to a point behind and within 40 m reverses (S3).
				var to: Vector2 = goal - v.pos
				var fwd := Vector2(-sin(v.yaw), -cos(v.yaw))
				var back := to.length() < 40.0 and to.normalized().dot(fwd) < -0.5
				_order(v, "reverse" if back and kind == &"move_units" else String(kind).trim_suffix("_units"), goal)
				n += 1
		&"stop_units":
			for v in own:
				_order(v, "idle", v.pos)
		&"retreat_units":
			for v in own:
				_order(v, "retreat", MockMap.BASES[src.local_player])
		&"reverse_units":
			var t: Vector3 = data.target
			for v in own:
				_order(v, "reverse", Vector2(t.x, t.z))
		&"face_units":
			var t: Vector3 = data.target
			for v in own:
				_order(v, "face", v.pos)
				v.face_yaw = Vehicles.yaw_toward(t.x - v.pos.x, t.z - v.pos.y)
		&"attack_unit":
			var target := int(data.get("target", -1))
			if vehicles.has(target) and int(vehicles[target].owner) == 1 - src.local_player:
				for v in own:
					v.attack = target
		&"set_ammo":
			var a := String(data.get("ammo", "auto"))
			if a in Vehicles.AMMO_CHOICES:
				for v in own:
					v.ammo_choice = a
		&"repair_units":
			for v in own:
				_start_repair(v)
		&"abandon":
			# S3c (mock): the crew bail out at once (no hatch times).
			for v in own:
				abandon(v, "order")


func _order(v: Dictionary, order: String, target: Vector2) -> void:
	v.order = order
	v.target = target
	if order != "face" and v.repairing != "":
		v.repairing = ""


func _start_repair(v: Dictionary) -> void:
	var worst := ""
	var worst_f := 2.0
	for c in ["engine", "track_l", "track_r", "ring", "transmission"]:
		var cap := 1.0 if c.begins_with("track") else 0.5
		var f: float = v.comp[c]
		if f < cap and f < worst_f:
			worst = c
			worst_f = f
	if worst != "":
		v.repairing = worst
		v.order = "idle"


# --- simulation stand-in -----------------------------------------------------

func step(dt: float) -> void:
	var ids := vehicles.keys()
	ids.sort()
	for id in ids:
		if vehicles.has(id):
			_step_vehicle(vehicles[id], dt)
	_step_shells(dt)
	_cap_wrecks()


func _alive(v: Dictionary) -> bool:
	return int(v.owner) >= 0 and not v.get("abandoned", false)


## S3c (mock): the crew of `v` bail out: it stays, abandoned (nobody's),
## and `crew` men (the living seats) become a crew squad beside it. The
## crew squad's id, or -1 with nobody left.
func abandon(v: Dictionary, cause := "order") -> int:
	if not _alive(v):
		return -1
	var men := 0
	for s in v.crew:
		if v.crew[s] in ["ok", "wounded", "swapping"]:
			men += 1
		v.crew[s] = "dead"
	v.abandoned = true
	v.abandon_cause = cause
	v.last_owner = int(v.owner)
	v.speed = 0.0
	v.order = "idle"
	v.repairing = ""
	v.recrew = {}
	src._stop_followers(v.id)
	var pos: Vector3 = src._v3(v.pos)
	_event("abandoned", v, -1, cause, "", "", pos)
	if int(v.owner) == src.local_player and not _quiet:
		src._push_event(v.owner, "vehicle_abandoned", "%s abandoned (ordered): %d of 5 bailing out." % [_short(v), men], pos)
	if men == 0:
		v.crew_squad = -1
		return -1
	var side := Vector2(cos(v.yaw), -sin(v.yaw)) * 3.5
	v.crew_squad = src._spawn(int(v.owner), "crew", v.pos + side, men)
	return v.crew_squad


## S3c (mock): squad `sid` of `owner` re-crews abandoned `v` after 3 s.
func recrew(v: Dictionary, sid: int, owner: int) -> void:
	if v.get("abandoned", false):
		v.recrew = {"squad": sid, "owner": owner, "t": 3.0}


func _step_recrew(v: Dictionary, dt: float) -> void:
	var r: Dictionary = v.get("recrew", {})
	if r.is_empty():
		return
	r.t -= dt
	if r.t > 0.0:
		return
	v.abandoned = false
	v.recrew = {}
	var own: bool = int(r.owner) == int(v.last_owner)
	v.owner = int(r.owner)
	for s in v.crew:
		v.crew[s] = "ok"
	src.squads.erase(int(r.squad))
	v.crew_squad = -1
	var pos: Vector3 = src._v3(v.pos)
	_event("recrewed", v, int(v.last_owner), "own" if own else "captured", "", "", pos)
	if int(v.owner) == src.local_player:
		src._push_event(v.owner, "vehicle_recrewed" if own else "vehicle_captured", ("%s re-crewed." if own else "Captured an enemy %s.") % _short(v), pos)


func _immobile(v: Dictionary) -> bool:
	return v.comp.engine <= 0.0 or v.comp.track_l <= 0.0 or v.comp.track_r <= 0.0 or v.comp.transmission <= 0.0 or v.crew.driver != "ok" and v.crew.driver != "wounded"


func _step_vehicle(v: Dictionary, dt: float) -> void:
	if not _alive(v):
		v.speed = 0.0
		_step_recrew(v, dt)
		return
	var h: Dictionary = HANDLING[v.type]
	var vt: Dictionary = TYPES[v.type]
	# Seat swaps finishing.
	for seat in v.swap.keys():
		v.swap[seat] -= dt
		if v.swap[seat] <= 0.0:
			v.crew[seat] = "ok"
			v.swap.erase(seat)
	# Bogging, fire, repair.
	if v.bog_t > 0.0:
		v.bog_t -= dt
		if v.bog_t <= 0.0:
			v.crossing = false
	if v.burn_t >= 0.0:
		v.burn_t += dt
		v.comp.engine = maxf(v.comp.engine - 0.05 * dt, 0.0)
		v.fire_roll_t -= dt
		if v.fire_roll_t <= 0.0:
			v.fire_roll_t = 4.0
			if src.rng.randf() < 0.25:
				v.burn_t = -1.0
		if v.burn_t >= FIRE_KO_S:
			_knock_out(v, "fire", -1)
			return
	if v.repairing != "":
		var c: String = v.repairing
		var rate := 1.0 / 30.0 if c.begins_with("track") else (0.5 / 60.0 if c == "engine" or c == "transmission" else 0.5 / 40.0)
		var cap := 1.0 if c.begins_with("track") else 0.5
		v.comp[c] = minf(v.comp[c] + rate * dt, cap)
		if v.comp[c] >= cap:
			v.repairing = ""
	_drive(v, h, vt, dt)
	_gunnery(v, h, vt, dt)


func _drive(v: Dictionary, h: Dictionary, vt: Dictionary, dt: float) -> void:
	var turn := deg_to_rad(float(h.turn))
	var want_speed := 0.0
	var stuck: bool = v.bog_t > 0.0 or _immobile(v) or v.repairing != ""
	if not stuck:
		match v.order:
			"move", "attack_move", "retreat", "reverse":
				var to: Vector2 = v.target - v.pos
				if to.length() < 2.0:
					v.order = "idle"
				else:
					var reverse: bool = v.order == "reverse"
					var want := Vehicles.yaw_toward(to.x, to.y) + (PI if reverse else 0.0)
					var err := wrapf(want - v.yaw, -PI, PI)
					v.yaw += clampf(err, -turn * dt, turn * dt)
					if absf(err) < 0.6:
						var top: float = -float(h.reverse) if reverse else float(vt.speed)
						if v.comp.engine < Vehicles.DAMAGED:
							top *= 0.5
						# Escorted: walking pace.
						if int(followers.get(v.id, 0)) > 0:
							top = clampf(top, -3.4, 3.4)
						if v.crossing:
							top = clampf(top, -1.0, 1.0)
						want_speed = top * clampf(to.length() / 8.0, 0.3, 1.0)
					if v.order == "attack_move" and v.aim != -1 and v.reload >= 1.0:
						want_speed = 0.0  # halts to fire the main gun
			"face":
				var err := wrapf(float(v.face_yaw) - v.yaw, -PI, PI)
				v.yaw += clampf(err, -turn * dt, turn * dt)
				if absf(err) < 0.01:
					v.order = "idle"
	var acc := 1.5 if absf(want_speed) > absf(v.speed) else 4.0
	v.speed = move_toward(v.speed, want_speed, acc * dt)
	var fwd := Vector2(-sin(v.yaw), -cos(v.yaw))
	v.pos += fwd * v.speed * dt


func _gunnery(v: Dictionary, h: Dictionary, vt: Dictionary, dt: float) -> void:
	# Target: the focused vehicle while seen, else the nearest seen enemy
	# vehicle in range, else (HE) the nearest seen enemy squad within 300 m.
	var eyes: PackedVector2Array = src._eyes(v.owner)
	var aim := -1
	if vehicles.has(v.attack) and _alive(vehicles[v.attack]) and _seen(eyes, vehicles[v.attack].pos):
		aim = v.attack
	else:
		v.attack = -1
		var best := ENGAGE_M
		for id in vehicles:
			var o: Dictionary = vehicles[id]
			if _alive(o) and int(o.owner) != int(v.owner):
				var d: float = o.pos.distance_to(v.pos)
				if d < best and _seen(eyes, o.pos):
					best = d
					aim = id
	if aim != v.aim:
		v.focus_shots = 0
	v.aim = aim
	v.aim_squad = -1
	var aim_at: Variant = null
	if aim != -1:
		aim_at = vehicles[aim].pos
	else:
		var sq: int = src._nearest_enemy_at(v.pos, int(v.owner), 300.0)
		if sq != -1 and v.ammo_choice != "ap":
			v.aim_squad = sq
			aim_at = src.squads[sq].pos
	var trav := deg_to_rad(float(h.traverse)) * (0.5 if v.comp.ring < Vehicles.DAMAGED else 1.0)
	if v.comp.ring <= 0.0:
		trav = 0.0
	var want_turret := 0.0
	var want_pitch := 0.0
	if aim_at != null:
		var to: Vector2 = aim_at - v.pos
		want_turret = wrapf(Vehicles.yaw_toward(to.x, to.y) - v.yaw, -PI, PI)
		var dy: float = MockMap.height(aim_at.x, aim_at.y) - MockMap.height(v.pos.x, v.pos.y)
		want_pitch = clampf(atan2(dy, maxf(to.length(), 1.0)) + to.length() * 0.00004, deg_to_rad(h.elev.x), deg_to_rad(h.elev.y))
	# S7: a casemate gun lays only within its arc.
	if h.has("arc"):
		want_turret = clampf(want_turret, -deg_to_rad(float(h.arc)), deg_to_rad(float(h.arc)))
	var err := wrapf(want_turret - v.turret_yaw, -PI, PI)
	v.turret_yaw = wrapf(v.turret_yaw + clampf(err, -trav * dt, trav * dt), -PI, PI)
	v.gun_pitch = move_toward(v.gun_pitch, want_pitch, deg_to_rad(6.0) * dt)
	# Loading: the round that fits the target (Auto), or the forced one.
	var want_round: String = v.ammo_choice
	if want_round == "auto":
		want_round = "ap" if aim != -1 else "he"
	if v.ammo[want_round] <= 0:
		want_round = "he" if want_round == "ap" else "ap"
	var reload_s: float = h.reload * (2.0 if v.crew.loader == "dead" else (1.5 if v.crew.loader == "wounded" else 1.0))
	if v.loaded != want_round and v.reload >= 1.0 and v.ammo[want_round] > 0:
		v.loaded = want_round
		v.reload = 0.0
	v.reload = minf(v.reload + dt / reload_s, 1.0)
	var can_fire: bool = v.repairing == "" and v.comp.gun > 0.0 and v.crew.gunner in ["ok", "wounded"]
	if aim_at == null or v.reload < 1.0 or not can_fire or absf(err) > deg_to_rad(1.5) or v.loaded == "" or v.ammo[v.loaded] <= 0:
		return
	if v.aim_squad != -1 and v.loaded != "he":
		return
	_fire(v, h, aim_at)


func _muzzle(v: Dictionary) -> Array:
	var vt: Dictionary = TYPES[v.type]
	var base: Vector3 = src._v3(v.pos)
	var fwd := Vector3(-sin(v.yaw), 0.0, -cos(v.yaw))
	var ty: float = v.yaw + v.turret_yaw
	var dir := Vector3(-sin(ty) * cos(v.gun_pitch), sin(v.gun_pitch), -cos(ty) * cos(v.gun_pitch))
	var pivot: Vector3 = base + fwd * float(vt.turret_x) + Vector3(0, float(vt.turret_base) + float(vt.gun_height), 0)
	return [pivot + dir * float(vt.gun_length), dir]


func _fire(v: Dictionary, h: Dictionary, aim_at: Vector2) -> void:
	var round: String = v.loaded
	v.ammo[round] -= 1
	v.reload = 0.0
	var m := _muzzle(v)
	var dir: Vector3 = m[1]
	var speed: float = h.ap_speed if round == "ap" else h.he_speed
	var target: Vector3 = src._v3(aim_at) + Vector3(0, 1.4 if v.aim != -1 else 0.3, 0)
	# Ranging in: each shot at the same target is tighter (S3 gunnery).
	var spread := 2.5 * pow(0.8, mini(v.focus_shots, 3))
	v.focus_shots += 1
	var miss: bool = src.rng.randf() < 0.18 + 0.04 * spread
	if miss or v.aim_squad != -1:
		var scatter := Vector3(src.rng.randfn(0, 1), 0, src.rng.randfn(0, 1)) * (6.0 if miss else 3.0)
		target += scatter
		target.y = src.height_at(target.x, target.z)
	var vel: Vector3 = (target - m[0]).normalized() * speed
	src._effects.append({"kind": "cannon", "pos": m[0], "dir": dir, "round": round, "type": v.type})
	_event("fired", v, -1, round, "", "", m[0])
	shells.append({
		"id": _shell_id, "kind": round, "owner": int(v.owner), "shooter": int(v.id), "pos": m[0], "vel": vel,
		"t": m[0].distance_to(target) / speed, "target": -1 if miss else v.aim, "to": target,
	})
	_shell_id += 1


## An AT team's gunner fires a rocket at a vehicle (called by the source).
func fire_rocket(sq: Dictionary, target_id: int) -> void:
	var at: Dictionary = AT_TEAM.get(String(src.players[sq.owner].faction), AT_TEAM.germany)
	var from: Vector3 = src._v3(sq.pos) + Vector3(0, 1.2, 0)
	var tv: Dictionary = vehicles[target_id]
	var to: Vector3 = src._v3(tv.pos) + Vector3(0, 1.1, 0)
	var miss: bool = src.rng.randf() < 0.25
	if miss:
		to += Vector3(src.rng.randfn(0, 2.5), 0, src.rng.randfn(0, 2.5))
		to.y = src.height_at(to.x, to.z)
	var speed: float = at.speed
	src._effects.append({"kind": "rocket_launch", "pos": from, "dir": (to - from).normalized()})
	shells.append({
		"id": _shell_id, "kind": "heat", "owner": int(sq.owner), "shooter": int(sq.id), "pos": from,
		"vel": (to - from).normalized() * speed, "t": from.distance_to(to) / speed,
		"target": -1 if miss else target_id, "to": to,
	})
	_shell_id += 1


func _step_shells(dt: float) -> void:
	var done: Array[Dictionary] = []
	for s in shells:
		s.t -= dt
		s.pos += s.vel * dt
		if s.t <= 0.0:
			done.append(s)
	for s in done:
		shells.erase(s)
		var tid: int = s.target
		if vehicles.has(tid) and _alive(vehicles[tid]):
			_hit(vehicles[tid], s)
		elif s.kind != "ap":
			src._explode(Vector2(s.to.x, s.to.z), "tank_he" if s.kind == "he" else "rocket", s.owner)


## A shell or rocket strikes a vehicle: roll the result from fixed odds.
func _hit(v: Dictionary, s: Dictionary) -> void:
	var at: Vector3 = s.to
	var zone: String = ZONES[src.rng.randi() % ZONES.size()]
	var roll: float = src.rng.randf()
	var result := "pen" if roll < 0.45 else ("no_pen" if roll < 0.78 else "ricochet")
	if s.kind == "he":
		result = "no_pen"
	src._effects.append({"kind": "armour_hit", "result": result, "pos": at})
	_event(result, v, s.owner, "", zone, s.kind, at)
	if s.kind != "ap":
		src._effects.append({"kind": "explosion", "pos": at, "size": "tank_he" if s.kind == "he" else "rocket"})
	v.repairing = ""
	if result == "ricochet":
		return
	if result == "no_pen":
		if zone == "hull_side" and src.rng.randf() < 0.5:
			_damage(v, "track_l" if src.rng.randf() < 0.5 else "track_r", 0.6, s.owner, at)
		return
	# Penetration: one or two components, maybe crew, maybe worse.
	for k in 1 + src.rng.randi() % 2:
		var parts: Array = Vehicles.COMPONENTS.filter(func(p: String) -> bool: return v.comp.has(p))
		var c: String = parts[src.rng.randi() % parts.size()]
		_damage(v, c, src.rng.randf_range(0.3, 0.9), s.owner, at)
		if not _alive(v):
			return
	if src.rng.randf() < 0.6:
		var seat: String = Vehicles.SEATS[src.rng.randi() % Vehicles.SEATS.size()]
		_crew_hit(v, seat, s.owner, at)
	if _alive(v) and src.rng.randf() < 0.12:
		_knock_out(v, "crew", s.owner)


func _damage(v: Dictionary, c: String, amount: float, by: int, at: Vector3) -> void:
	var before: float = v.comp[c]
	v.comp[c] = maxf(before - amount, 0.0)
	if c == "engine" and amount >= 0.4 and v.burn_t < 0.0 and src.rng.randf() < 0.3:
		v.burn_t = 0.0
	if v.comp[c] <= 0.0 and before > 0.0:
		_event("disabled", v, by, c, "", "", at)
		if int(v.owner) == src.local_player:
			src._push_event(v.owner, "vehicle_damaged", "%s: %s destroyed." % [_short(v), String(Vehicles.COMPONENT_NAMES[c]).to_lower()], src._v3(v.pos))
		if c == "ammo_rack":
			_knock_out(v, "ammo_rack", by)
	else:
		_event("component_hit", v, by, c, "", "", at)


func _crew_hit(v: Dictionary, seat: String, by: int, at: Vector3) -> void:
	if v.crew[seat] == "dead":
		return
	v.crew[seat] = "dead" if v.crew[seat] == "wounded" or src.rng.randf() < 0.5 else "wounded"
	_event("crew_hit", v, by, seat, "", "", at)
	# Auto swap: radio, then loader, then commander takes the driver's or
	# gunner's seat (4 s).
	if v.crew[seat] == "dead" and seat in ["gunner", "driver"]:
		for from in ["radio_mg", "loader", "commander"]:
			if v.crew[from] in ["ok", "wounded"]:
				v.crew[from] = "dead"
				v.crew[seat] = "swapping"
				v.swap[seat] = SEAT_SWAP_S
				break
	var alive := 0
	for s in v.crew:
		if v.crew[s] != "dead":
			alive += 1
	if alive == 0:
		_knock_out(v, "crew", by)


func _knock_out(v: Dictionary, cause: String, by: int) -> void:
	if not _alive(v):
		return
	var pos: Vector3 = src._v3(v.pos)
	_event("knocked_out", v, by, cause, "", "", pos)
	if cause == "ammo_rack":
		src._effects.append({"kind": "cook_off", "pos": pos + Vector3(0, 1.5, 0)})
	var seen: bool = not _quiet and (int(v.owner) == src.local_player or _seen(src._eyes(src.local_player), v.pos))
	if seen:
		var own: bool = int(v.owner) == src.local_player
		src._push_event(v.owner, "vehicle_knocked_out", ("%s knocked out." if own else "Enemy %s knocked out.") % _short(v), pos)
	if by >= 0:
		src.stats[by].kills += 5
		src.stats[v.owner].losses += 5
	v.last_owner = v.owner
	v.owner = -1
	v.wreck_t = src.time_s
	v.speed = 0.0
	v.burn_t = -1.0
	v.repairing = ""
	v.order = "idle"
	for s in v.crew:
		v.crew[s] = "dead"
	for id in vehicles:
		if int(vehicles[id].attack) == int(v.id):
			vehicles[id].attack = -1
	src._stop_followers(v.id)


func _cap_wrecks() -> void:
	var wrecks: Array = []
	for id in vehicles:
		if int(vehicles[id].owner) < 0:
			wrecks.append(vehicles[id])
	if wrecks.size() <= WRECK_CAP:
		return
	wrecks.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return a.wreck_t < b.wreck_t)
	for i in wrecks.size() - WRECK_CAP:
		vehicles.erase(wrecks[i].id)


func _event(kind: String, v: Dictionary, attacker: int, component: String, zone: String, ammo: String, pos: Vector3) -> void:
	_seq += 1
	_armour.append({
		"kind": kind, "vehicle": int(v.id), "type": v.type, "owner": int(v.last_owner if not _alive(v) else v.owner),
		"attacker": attacker, "component": component, "zone": zone, "ammo": ammo, "pos": pos,
		"seq": _seq, "time_s": src.time_s,
	})


func _short(v: Dictionary) -> String:
	return String(TYPES[v.type].short)


func _seen(eyes: PackedVector2Array, p: Vector2) -> bool:
	var sight: float = float(src.SIGHT_M) * SPOT_MULT * (0.5 if MockMap.in_woods(p) else 1.0)
	for e in eyes:
		if e.distance_to(p) <= sight:
			return true
	return false


## Enemy AI: buys a tank now and then after the unlock, and sends idle
## tanks at the nearest point it does not hold.
func ai_plan(ai: int) -> void:
	if call_in_block(ai) == "" and count(ai) < 2 and src.rng.randf() < 0.2:
		src._call_in_entry(ai, src.catalog_entry(ai, "vehicle"))
	for id in vehicles:
		var v: Dictionary = vehicles[id]
		if int(v.owner) != ai or v.order != "idle" or src.rng.randf() > 0.4:
			continue
		var best := -1
		var best_d := INF
		for i in src.points.size():
			if src.points[i].owner == ai:
				continue
			var p: Vector3 = src.points[i].pos
			var d: float = v.pos.distance_to(Vector2(p.x, p.z))
			if d < best_d:
				best_d = d
				best = i
		if best != -1:
			var p: Vector3 = src.points[best].pos
			_order(v, "attack_move", Vector2(p.x, p.z) + Vector2(src.rng.randf_range(-20, 20), src.rng.randf_range(-20, 20)))


# --- staging (screenshots, tests) --------------------------------------------

## Sets a staged state on a vehicle: "bogged" (in a trench crossing),
## "burning", "repairing" (a broken track), "immobilised", "reversing",
## "moving", "wounded_crew", "swapping", "wreck", "old_wreck".
func stage(id: int, what: String) -> void:
	var v: Dictionary = vehicles.get(id, {})
	if v.is_empty():
		return
	match what:
		"bogged":
			v.bog_t = 30.0
			v.crossing = true
		"burning":
			v.burn_t = 2.0
			v.fire_roll_t = 15.0
			v.comp.engine = 0.35
		"repairing":
			v.comp.track_l = 0.0
			v.repairing = "track_l"
		"immobilised":
			v.comp.track_r = 0.0
		"wounded_crew":
			v.crew.loader = "wounded"
			v.crew.radio_mg = "dead"
			v.comp.ring = 0.45
		"swapping":
			v.crew.gunner = "swapping"
			v.crew.radio_mg = "dead"
			v.swap["gunner"] = 30.0
		"wreck":
			_quiet = true
			_knock_out(v, "ammo_rack", -1)
			_quiet = false
		"old_wreck":
			_quiet = true
			_knock_out(v, "crew", -1)
			_quiet = false
			v.wreck_t = src.time_s - KO_SMOKE_S - 5.0
		"abandoned":
			# S3c: gun and a track out, the crew bailed out (three alive).
			v.comp.gun = 0.0
			v.comp.track_l = 0.0
			v.crew.loader = "dead"
			v.crew.radio_mg = "dead"
			_quiet = true
			abandon(v, "useless")
			_quiet = false


# --- export ------------------------------------------------------------------

func flags(v: Dictionary) -> int:
	if v.get("abandoned", false):
		return Vehicles.ABANDONED | (Vehicles.BOARDING if not v.get("recrew", {}).is_empty() else 0)
	if not _alive(v):
		return Vehicles.WRECK | (Vehicles.KNOCKED_OUT if src.time_s - float(v.wreck_t) < KO_SMOKE_S else 0)
	var f := 0
	if absf(v.speed) > 1.0:
		f |= Vehicles.MOVING
	if v.speed < -0.3:
		f |= Vehicles.REVERSING
	if v.bog_t > 0.0:
		f |= Vehicles.BOGGED
	if _immobile(v):
		f |= Vehicles.IMMOBILISED
	if v.burn_t >= 0.0:
		f |= Vehicles.BURNING
	if v.repairing != "":
		f |= Vehicles.REPAIRING
	if v.crossing:
		f |= Vehicles.CROSSING
	return f


static func state_of(f: int, speed: float, own: bool) -> String:
	if f & Vehicles.WRECK:
		return "wreck"
	if f & Vehicles.ABANDONED:
		return "abandoned"
	if f & Vehicles.BURNING:
		return "burning"
	if own:
		for pair in [[Vehicles.BOGGED, "bogged"], [Vehicles.IMMOBILISED, "immobilised"], [Vehicles.REPAIRING, "repairing"], [Vehicles.CROSSING, "crossing"], [Vehicles.REVERSING, "reversing"], [Vehicles.MOVING, "moving"]]:
			if f & int(pair[0]):
				return pair[1]
		return "idle"
	return "moving" if absf(speed) > 1.0 else "idle"


## vehicles, shells and armour_events for the local player; drains the
## armour events.
func export_state(eyes: PackedVector2Array) -> Dictionary:
	var local: int = src.local_player
	var out_v: Array = []
	var ids := vehicles.keys()
	ids.sort()
	var seen_ids := {}
	for id in ids:
		var v: Dictionary = vehicles[id]
		var own := int(v.owner) == local
		var wreck := not _alive(v)
		if not own and not wreck and not _seen(eyes, v.pos):
			continue
		seen_ids[id] = true
		out_v.append(_vehicle_state(v, own))
	var out_s: Array = []
	for s in shells:
		var shooter_seen: bool = int(s.owner) == local or seen_ids.has(int(s.shooter)) or src._sees(eyes, Vector2(s.pos.x, s.pos.z))
		if shooter_seen:
			out_s.append({"id": s.id, "kind": s.kind, "owner": s.owner, "shooter": s.shooter, "pos": s.pos, "vel": s.vel})
	var out_e: Array = []
	for e in _armour:
		if int(e.owner) == local or seen_ids.has(int(e.vehicle)) or _near_men(e.pos, 40.0):
			out_e.append(e)
	_armour.clear()
	return {"vehicles": out_v, "shells": out_s, "armour_events": out_e}


func _near_men(p: Vector3, r: float) -> bool:
	for sid in src.squads:
		var sq: Dictionary = src.squads[sid]
		if sq.owner == src.local_player and sq.pos.distance_to(Vector2(p.x, p.z)) < r:
			return true
	return false


func _vehicle_state(v: Dictionary, own: bool) -> Dictionary:
	var vt: Dictionary = TYPES[v.type]
	var f := flags(v)
	var base: Vector3 = src._v3(v.pos)
	# Hull pitch and roll from the ground under the tracks.
	var fwd := Vector2(-sin(v.yaw), -cos(v.yaw))
	var right := Vector2(cos(v.yaw), -sin(v.yaw))
	var hl := float(vt.hull_length) * 0.5
	var hw := float(vt.hull_width) * 0.5
	var hf := MockMap.height(v.pos.x + fwd.x * hl, v.pos.y + fwd.y * hl)
	var hb := MockMap.height(v.pos.x - fwd.x * hl, v.pos.y - fwd.y * hl)
	var hlft := MockMap.height(v.pos.x - right.x * hw, v.pos.y - right.y * hw)
	var hr := MockMap.height(v.pos.x + right.x * hw, v.pos.y + right.y * hw)
	var bogged := (f & Vehicles.BOGGED) != 0
	var out := {
		"id": int(v.id),
		"squad": int(v.squad) if _alive(v) else -1,
		"owner": int(v.owner),
		"type": v.type,
		"name": vt.name,
		"pos": base + Vector3(0, -0.45 if bogged else 0.0, 0),
		"yaw": v.yaw,
		"pitch": atan2(hf - hb, hl * 2.0) + (-0.16 if bogged else 0.0),
		"roll": atan2(hlft - hr, hw * 2.0) + (0.05 if bogged else 0.0),
		"turret_yaw": v.turret_yaw,
		"gun_pitch": v.gun_pitch,
		"speed": v.speed,
		"flags": f if own else f & (Vehicles.BURNING | Vehicles.WRECK | Vehicles.KNOCKED_OUT | Vehicles.ABANDONED | Vehicles.BOARDING),
		"state": state_of(f, v.speed, own),
		"abandoned": v.get("abandoned", false),
	}
	if v.get("abandoned", false):
		# S3c: nobody's; public last owner, cause, crew squad, parts out.
		out.owner = -1
		out.squad = -1
		out.last_owner = int(v.last_owner)
		out.abandon_cause = String(v.get("abandon_cause", ""))
		out.crew_squad = int(v.get("crew_squad", -1))
		out.inside = 0
		out.boarding = not v.get("recrew", {}).is_empty()
		var parts := PackedStringArray()
		for c in Vehicles.COMPONENTS:
			if float(v.comp.get(c, 1.0)) <= 0.0:
				parts.append(c)
		out.parts_out = parts
	if own:
		var crew := PackedStringArray()
		for seat in Vehicles.SEATS:
			crew.append(String(v.crew[seat]) if vt.seats.has(seat) else "none")
		out.crew = crew
		out.components = v.comp.duplicate()
		out.repairing = v.repairing
		out.ammo = v.ammo.duplicate()
		out.loaded = v.loaded
		out.ammo_choice = v.ammo_choice
		out.reload = v.reload
		out.target = v.aim
		out.followers = int(followers.get(v.id, 0))
	return out


## rules.vehicles / vehicle_types / dig_profiles.
func rules() -> Dictionary:
	return {
		"vehicles": {"unlock_s": unlock_s, "limit": limit},
		"dig_profiles": {
			"trench": {"max_m": 60.0, "depth_m": 1.6, "width_m": 1.6},
			"at_ditch": {"max_m": 30.0, "depth_m": 2.3, "width_m": 3.4, "engineers_only": true},
		},
		"vehicle_types": _types_state(),
	}


## TYPES as the state sends them (seats as a PackedStringArray).
static func _types_state() -> Dictionary:
	var out := {}
	for k in TYPES:
		var t: Dictionary = TYPES[k].duplicate(true)
		t.seats = PackedStringArray(t.seats)
		out[k] = t
	return out


## Staging: every live tank of `owner` (-1: all) lays on its target at once
## and fires (screenshots time effects into the shot). Returns the shots.
func volley(owner := -1) -> int:
	var n := 0
	var ids := vehicles.keys()
	ids.sort()
	for id in ids:
		var v: Dictionary = vehicles[id]
		if not _alive(v) or (owner != -1 and int(v.owner) != owner):
			continue
		var h: Dictionary = HANDLING[v.type]
		v.reload = 0.0
		_gunnery(v, h, TYPES[v.type], 0.0)
		var aim_at: Variant = null
		if v.aim != -1:
			aim_at = vehicles[v.aim].pos
		elif v.aim_squad != -1 and src.squads.has(v.aim_squad):
			aim_at = src.squads[v.aim_squad].pos
		if aim_at == null:
			continue
		var to: Vector2 = aim_at - v.pos
		v.turret_yaw = wrapf(Vehicles.yaw_toward(to.x, to.y) - v.yaw, -PI, PI)
		v.reload = 1.0
		if v.loaded == "" or v.ammo[v.loaded] <= 0:
			continue
		_fire(v, h, aim_at)
		n += 1
	return n
