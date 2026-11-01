class_name MockMatchSource
extends Node
## Fake match for running and screenshotting the HUD without the sim. It
## produces the same Dictionary as the bridge will (docs/match_state.md)
## and accepts the same intents, with a crude stand-in for S1/S1b rules:
## movement, capture, ticket drain, income, call-ins, dice-roll firefights
## with per-man suppression and wounds, mortar barrages, digging, fog by
## distance, an enemy that walks to points and buys squads, and (S1b)
## individual soldiers: detached men with their own orders, stance
## overrides, grenades, smoke and satchel charges with item counts and
## resupply at base, and (S3) tanks, wrecks, shells, AT rockets and armour
## events through MockVehicles (mock_vehicles.gd), AT teams, escorts and
## vehicle call-ins with the unlock countdown and the vehicle limit.
##
## Presenter-only and non-deterministic on purpose: none of this is the
## sim, and nothing here may be copied into crates/sim.
##
## Source interface (what match.gd expects of any source):
##   get_state() -> Dictionary      # one frame; events/effects drain
##   handle_intent(kind: StringName, data: Dictionary) -> void
##   height_at(x: float, z: float) -> float

const CATALOG := [
	{"type": "rifle", "name": "Rifle squad", "cost": 120, "delay_s": 20, "men": 8},
	{"type": "mg", "name": "MG team", "cost": 150, "delay_s": 25, "men": 3},
	{"type": "engineer", "name": "Engineer squad", "cost": 140, "delay_s": 20, "men": 5},
	{"type": "mortar", "name": "Mortar team", "cost": 200, "delay_s": 30, "men": 3},
	{"type": "at", "name": "AT team", "cost": 200, "delay_s": 20, "men": 2},
]
const RULES := {
	"pop_cap": 120,
	"tickets_max": 500,
	"dig_max_m": 60.0,
	"mortar_min_m": 60.0,
	"mortar_max_m": 600.0,
	"capture_radius_m": 15.0,
	"throw_range_m": {"frag": Vector2(5, 30), "smoke": Vector2(5, 30), "satchel": Vector2(3, 8)},
}
const SPEED := {"rifle": 3.4, "mg": 2.8, "engineer": 3.4, "mortar": 2.6, "at": 3.2}
const RANGE := {"rifle": 160.0, "mg": 200.0, "engineer": 70.0, "mortar": 0.0, "at": 120.0}
const FIRE_GAP := {"rifle": 0.7, "mg": 0.12, "engineer": 0.25, "at": 1.4}
const WEAPON := {"rifle": "rifle", "mg": "mg", "engineer": "smg", "at": "rifle"}
## AT launcher reload (S3: 8 s with the loader).
const ROCKET_RELOAD_S := 8.0
## Escort file: slots 1.5 m apart, 4 m behind the vehicle (S3 "Escort").
const ESCORT_BACK_M := 4.0
const ESCORT_GAP_M := 1.5
const SIGHT_M := 160.0
const CAPTURE_S := 20.0
const QUEUE_MAX := 5
## Mock digging runs this much faster than §5.10.9 pacing so it shows.
const DIG_SPEEDUP := 6.0
const DIFFICULTY_INCOME := {"easy": 0.8, "normal": 1.0, "hard": 1.25}
const SLOT_SPACING := 2.5
const PINNED := 0.7
const SUPPRESSED := 0.35
## Throws (S1b-abilities.md): wind-up, flight speed, total fuse from the
## start of the throw, blast radii (lethal, damage, suppression).
const WIND_UP_S := 1.0
const THROW_SPEED := 30.0
const FUSE_S := {"frag": 4.0, "satchel": 5.0}
const BLAST := {"frag": Vector3(2, 6, 10), "satchel": Vector3(4, 10, 18), "mortar": Vector3(3, 8, 15), "tank_he": Vector3(2, 6, 12), "rocket": Vector3(1, 3, 6)}
const SMOKE_GROW_S := 3.0
const SMOKE_HOLD_S := 25.0
const SMOKE_THIN_S := 5.0
const SMOKE_R := 8.0
const SMOKE_CAP := 64
const RESUPPLY_S := 20.0

@export var time_scale := 1.0

var setup := {"faction": "germany", "difficulty": "normal", "map": "kessel_ridge", "seed": 1}
var local_player := 0
var time_s := 0.0
var paused := false
var rng := RandomNumberGenerator.new()
var players: Array[Dictionary] = []
var points: Array[Dictionary] = []
## id -> squad (internal fields; see _squad_state for what is exported).
var squads := {}
## soldier id -> squad id (living men).
var man_squad := {}
var stats: Array[Dictionary] = []
var result := {"status": "running", "winner": -1, "reason": ""}
## Re-call cooldowns shown on the call-in tiles (type -> [left, total]);
## not an S1 rule, only here so the HUD's optional field is exercised.
var recall_cooldown := {}

var _next_id := 1
var _next_man := 1001
var _queues: Array = [[], []]
var _events: Array[Dictionary] = []
var _effects: Array[Dictionary] = []
var _shells: Array[Dictionary] = []
var _thrown: Array[Dictionary] = []
var _smokes: Array[Dictionary] = []
var _ai_t := 0.0
var _tick := 0
var _minimap: Texture2D
## S3: tanks, wrecks, shells and armour events.
var veh := MockVehicles.new(self)


func _ready() -> void:
	start(Session.setup if has_node("/root/Session") else setup)


## Resets the mock to a fresh match for `s` (Session.setup shape).
func start(s: Dictionary) -> void:
	setup = s.duplicate()
	rng.seed = int(setup.get("seed", 1)) * 7919 + 17
	var mine: String = setup.get("faction", "germany")
	var theirs := "usa" if mine == "germany" else "germany"
	players = [
		{"id": 0, "faction": mine, "name": "You", "is_ai": false, "tickets": 500.0, "manpower": 260.0, "income": 0.0, "pop": 0, "points_held": 0, "ticket_drain": 0.0},
		{"id": 1, "faction": theirs, "name": "AI (%s)" % setup.get("difficulty", "normal"), "is_ai": true, "tickets": 500.0, "manpower": 260.0, "income": 0.0, "pop": 0, "points_held": 0, "ticket_drain": 0.0},
	]
	stats = []
	for i in 2:
		stats.append({"kills": 0, "losses": 0, "points_held_s": 0.0, "squads_called": 0, "trench_m": 0.0})
	points = []
	for p in MockMap.POINTS:
		var pos: Vector2 = p.pos
		points.append({"id": p.id, "pos": _v3(pos), "radius": RULES.capture_radius_m, "owner": -1, "capturing": -1, "progress": 0.0, "contested": false})
	points[0].owner = 0
	points[4].owner = 1
	squads.clear()
	man_squad.clear()
	_next_id = 1
	_next_man = 1001
	# Opening positions: both sides already pushing towards the middle.
	var own := [["rifle", Vector2(-250, 170)], ["rifle", Vector2(-215, -150)], ["mg", Vector2(-120, 80)],
		["engineer", Vector2(-285, 228)], ["mortar", Vector2(-330, 310)], ["rifle", Vector2(-60, 40)]]
	var foe := [["rifle", Vector2(230, -170)], ["rifle", Vector2(200, 150)], ["mg", Vector2(60, -40)],
		["engineer", Vector2(280, -230)], ["mortar", Vector2(340, -320)], ["rifle", Vector2(40, 30)]]
	for e in own:
		_spawn(0, e[0], e[1])
	for e in foe:
		_spawn(1, e[0], e[1])
	time_s = 0.0
	result = {"status": "running", "winner": -1, "reason": ""}
	_queues = [[], []]
	_events.clear()
	_effects.clear()
	_shells.clear()
	_thrown.clear()
	_smokes.clear()
	recall_cooldown.clear()
	veh.reset()
	_push_event(0, "info", "Skirmish on Kessel Ridge. Hold the points.", null)


func height_at(x: float, z: float) -> float:
	return MockMap.height(x, z)


func _v3(p: Vector2) -> Vector3:
	return Vector3(p.x, MockMap.height(p.x, p.y), p.y)


## Starting items by squad type and slot (S1b-abilities.md "Items"): frags
## for rifle and engineer men (leader 2), smoke for their leaders, one
## satchel per engineer squad (slot 1). MG and mortar teams carry nothing.
static func loadout(type: String, slot: int) -> Dictionary:
	var out := {"frag": 0, "smoke": 0, "satchel": 0}
	if type in ["rifle", "engineer"]:
		out.frag = 2 if slot == 0 else 1
		out.smoke = 1 if slot == 0 else 0
	if type == "engineer" and slot == 1:
		out.satchel = 1
	return out


func _spawn(owner: int, type: String, at: Vector2) -> int:
	var def: Dictionary = {}
	for c in CATALOG:
		if c.type == type:
			def = c
	if type == "at":
		def = def.duplicate()
		def.name = MockVehicles.AT_TEAM.get(String(players[owner].faction), MockVehicles.AT_TEAM.germany).name
	var id := _next_id
	_next_id += 1
	var face := (Vector2.ZERO - at).normalized()
	var sq := {
		"id": id, "owner": owner, "type": type, "name": def.name,
		"men": [], "men_max": int(def.men),
		"order": "idle", "pos": at, "target": at, "facing": face,
		"fire_t": rng.randf() * 2.0, "setup_t": 0.0, "dig": {}, "barrage": {}, "unit_dig": {},
		"follow": -1, "rocket_t": 0.0, "rockets": 6,
	}
	squads[id] = sq
	for slot in int(def.men):
		var man := {
			"id": _next_man, "slot": slot, "hp": 100.0, "pos": at, "supp": 0.0,
			"detached": false, "order": "", "target": at, "stance_override": "auto",
			"items": loadout(type, slot), "throw": {}, "resupply_t": 0.0,
		}
		_next_man += 1
		sq.men.append(man)
		man_squad[man.id] = id
		man.pos = _slot_pos(sq, slot)
	return id


# --- source interface --------------------------------------------------------

func handle_intent(kind: StringName, data: Dictionary) -> void:
	var ids: PackedInt32Array = data.get("squads", PackedInt32Array())
	var uids: PackedInt32Array = data.get("units", PackedInt32Array())
	# Vehicle ids in `units` (S3); soldier ids below drop unknown ids.
	veh.handle_intent(kind, data)
	match kind:
		&"move", &"attack_move":
			_squads_move(ids, String(kind), data.target)
		&"stop":
			for sq in _own_squads(ids):
				_order(sq, "idle", sq.pos)
		&"retreat":
			for sq in _own_squads(ids):
				_order(sq, "retreat", MockMap.BASES[local_player])
		&"dig":
			for sq in _own_squads(ids):
				if sq.type in ["rifle", "engineer"]:
					_squad_dig(sq, data.polyline)
		&"barrage":
			var t: Vector3 = data.target
			for sq in _own_squads(ids):
				if sq.type == "mortar":
					sq.order = "barrage"
					sq.target = sq.pos
					sq.barrage = {"target": Vector2(t.x, t.z), "rounds": 8, "t": 0.5}
		&"regroup":
			for sq in _own_squads(ids):
				for man in sq.men:
					_rejoin(man)
		&"move_units", &"attack_move_units", &"stop_units", &"retreat_units", &"dig_units":
			_unit_order(String(kind).trim_suffix("_units"), uids, data)
		&"reverse_units":
			_unit_order("move", uids, data)
		&"attack_unit":
			# Launcher gunners focus the vehicle; other men attack-move to it.
			var t: Dictionary = veh.vehicles.get(int(data.get("target", -1)), {})
			if not t.is_empty():
				for man in _own_men(uids):
					squads[man_squad[man.id]].attack = int(t.id)
				_unit_order("attack_move", uids, {"target": _v3(t.pos)})
		&"follow_units":
			_follow(uids, int(data.get("vehicle", -1)))
		&"set_stance":
			var st: String = data.get("stance", "auto")
			if st in ["auto", "stand", "crouch", "prone"]:
				for man in _own_men(uids):
					man.stance_override = st
		&"throw", &"throw_each":
			_throw(uids, String(data.item), data.target, kind == &"throw_each")
		&"call_in":
			var entry := catalog_entry(local_player, String(data.get("id", "")))
			if entry.is_empty():
				entry = catalog_entry(local_player, String(data.get("type", "")))
			_call_in_entry(local_player, entry)
		&"surrender":
			_end(1 - local_player, "surrender")
		&"pause":
			paused = data.paused


func get_state() -> Dictionary:
	var dt := get_process_delta_time() * time_scale
	if not paused and result.status == "running":
		_step(minf(dt, 0.1))
	return _build_state()


## Runs `seconds` of match time at once (screenshots start mid-match).
func prewarm(seconds: float) -> void:
	var steps := int(seconds / 0.1)
	for i in steps:
		_step(0.1)
		_effects.clear()
	if _events.size() > 5:
		_events = _events.slice(_events.size() - 5)


## Ends the match now (screenshots, tests).
func force_end(winner: int, reason: String) -> void:
	_end(winner, reason)


## Moves a squad to a ground position at once and makes it hold there
## (screenshot staging); its men snap to their slots.
func place_squad(id: int, at: Vector2, facing := Vector2.UP) -> void:
	if not squads.has(id):
		return
	var sq: Dictionary = squads[id]
	_order(sq, "idle", at)
	sq.pos = at
	sq.facing = facing.normalized()
	for man in sq.men:
		man.pos = _slot_pos(sq, man.slot)


## World position of a squad (Vector3.ZERO when gone).
func squad_pos(id: int) -> Vector3:
	return _v3(squads[id].pos) if squads.has(id) else Vector3.ZERO


## Ascending ids of `owner`'s squads, optionally of one type.
func squad_ids(owner: int, type := "") -> PackedInt32Array:
	var out := PackedInt32Array()
	var ids := squads.keys()
	ids.sort()
	for id in ids:
		if squads[id].owner == owner and (type == "" or squads[id].type == type):
			out.append(id)
	return out


## Living soldier ids of a squad, ascending.
func unit_ids(squad_id: int) -> PackedInt32Array:
	var out := PackedInt32Array()
	if squads.has(squad_id):
		for man in squads[squad_id].men:
			out.append(man.id)
	out.sort()
	return out


## World position of a soldier (Vector3.ZERO when gone).
func soldier_pos(id: int) -> Vector3:
	var man := _man(id)
	return _v3(man.pos) if not man.is_empty() else Vector3.ZERO


## Staging: set a soldier's health and suppression (0..1); a suppression
## above PINNED is held for `hold_s` so a screenshot taken later shows it.
func set_soldier(id: int, hp: float, supp := -1.0, hold_s := 20.0) -> void:
	var man := _man(id)
	if man.is_empty():
		return
	man.hp = clampf(hp, 1.0, 100.0)
	if supp >= 0.0:
		man.supp = clampf(supp, 0.0, 1.0)
		if supp > PINNED:
			man.pin_t = hold_s


## Tests: kill a soldier now.
func kill_soldier(id: int) -> void:
	var man := _man(id)
	if not man.is_empty():
		_hurt(squads[man_squad[id]], man, 1000.0, 1 - squads[man_squad[id]].owner)


## Staging (S3): lifts the unlock (the test override, unlock_s = 0) and
## places four own tanks in different states around `at` (facing
## `facing`): one moving with an escort target, one bogged in a trench
## crossing, one burning, one repairing a track; two enemy tanks the
## player sees; a fresh wreck (smoking) and an old one. Returns the own
## tank ids in that order. `escort` (a squad id) follows the first tank.
func stage_vehicles(at: Vector2, facing: Vector2, escort := -1) -> PackedInt32Array:
	veh.unlock_s = 0.0
	var f := facing.normalized()
	var side := Vector2(-f.y, f.x)
	var yaw := Vehicles.yaw_toward(f.x, f.y)
	var mine := veh.faction_tank(local_player)
	var theirs := veh.faction_tank(1 - local_player)
	var own := PackedInt32Array()
	var spots := [side * -24.0, side * -6.0 + f * 6.0, side * 12.0, side * 30.0 - f * 4.0]
	for i in 4:
		own.append(veh.spawn(local_player, mine, at + spots[i], yaw + (i - 1.5) * 0.12))
	veh.stage(own[1], "bogged")
	veh.stage(own[1], "wounded_crew")
	veh.stage(own[2], "burning")
	veh.stage(own[3], "repairing")
	veh.stage(own[3], "swapping")
	var v0: Dictionary = veh.vehicles[own[0]]
	v0.ammo_choice = "he"
	v0.loaded = "he"
	v0.ammo.ap = 24
	v0.ammo.he = 18
	v0.reload = 0.3
	v0.turret_yaw = 0.35
	var e1 := veh.spawn(1 - local_player, theirs, at + f * 62.0 + side * 22.0, yaw + PI + 0.3)
	var e2 := veh.spawn(1 - local_player, theirs, at + f * 78.0 - side * 30.0, yaw + PI - 0.5)
	veh.vehicles[e2].order = "move"
	veh.vehicles[e2].target = at + f * 70.0 - side * 90.0
	var w1 := veh.spawn(1 - local_player, theirs, at + f * 40.0 + side * 52.0, yaw + 2.2)
	veh.stage(w1, "wreck")
	var w2 := veh.spawn(local_player, mine, at - f * 30.0 - side * 50.0, yaw - 1.1)
	veh.stage(w2, "old_wreck")
	veh.vehicles[e1].turret_yaw = -0.2
	# Nobody is loaded yet, so a short prewarm shows no duel (screenshots
	# time their shots with MockVehicles.volley).
	for id in [own[1], own[2], own[3], e1, e2]:
		veh.vehicles[id].reload = 0.0
	if squads.has(escort):
		_order(squads[escort], "follow", squads[escort].pos)
		squads[escort].follow = own[0]
	_step_followers()
	return own


## Staging: moves a tank (own or enemy) at once.
func place_vehicle(id: int, at: Vector2, yaw: float) -> void:
	if veh.vehicles.has(id):
		veh.vehicles[id].pos = at
		veh.vehicles[id].yaw = yaw
		veh.vehicles[id].target = at


func set_minimap_texture(tex: Texture2D) -> void:
	_minimap = tex


# --- orders ------------------------------------------------------------------

func _own_squads(ids: PackedInt32Array) -> Array:
	var out: Array = []
	for id in ids:
		var sq: Dictionary = squads.get(id, {})
		if not sq.is_empty() and sq.owner == local_player:
			out.append(sq)
	return out


## Living own men among `uids`, sorted and deduplicated (the sim's
## validation: wrong owner, dead and unknown ids are dropped).
func _own_men(uids: PackedInt32Array) -> Array:
	var ids := uids.duplicate()
	ids.sort()
	var out: Array = []
	var last := -1
	for id in ids:
		if id == last:
			continue
		last = id
		var man := _man(id)
		if not man.is_empty() and squads[man_squad[id]].owner == local_player:
			out.append(man)
	return out


func _man(id: int) -> Dictionary:
	if not man_squad.has(id):
		return {}
	for man in squads[man_squad[id]].men:
		if man.id == id:
			return man
	return {}


func _squads_move(ids: PackedInt32Array, kind: String, t: Vector3) -> void:
	var n := 0
	for sq in _own_squads(ids):
		# Spread the group around the target so they do not stack.
		var off := Vector2(n % 3 - 1, floorf(n / 3.0)) * 14.0
		_order(sq, kind, Vector2(t.x, t.z) + off)
		n += 1


func _squad_dig(sq: Dictionary, line: PackedVector3Array) -> void:
	_order(sq, "dig", Vector2(line[0].x, line[0].z))
	sq.dig = {"line": line, "len": _poly_len(line), "progress": 0.0, "sound_t": 0.0}


## A unit order: men of squads fully covered get the squad order (and
## regroup), the rest detach and follow their own order.
func _unit_order(order: String, uids: PackedInt32Array, data: Dictionary) -> void:
	var men := _own_men(uids)
	if order == "dig":
		men = men.filter(func(m: Dictionary) -> bool: return squads[man_squad[m.id]].type in ["rifle", "engineer"])
	if men.is_empty():
		return
	var per_squad := {}
	for man in men:
		var sid: int = man_squad[man.id]
		if not per_squad.has(sid):
			per_squad[sid] = 0
		per_squad[sid] += 1
	var loose: Array = []
	for man in men:
		var sq: Dictionary = squads[man_squad[man.id]]
		if per_squad[sq.id] == sq.men.size():
			continue
		loose.append(man)
	for sid in per_squad:
		var sq: Dictionary = squads[sid]
		if per_squad[sid] != sq.men.size():
			continue
		match order:
			"move", "attack_move":
				_order(sq, order, Vector2(data.target.x, data.target.z))
			"stop":
				_order(sq, "idle", sq.pos)
			"retreat":
				_order(sq, "retreat", MockMap.BASES[local_player])
			"dig":
				_squad_dig(sq, data.polyline)
	if loose.is_empty():
		return
	match order:
		"move", "attack_move":
			var t := Vector2(data.target.x, data.target.z)
			var c := Vector2.ZERO
			for man in loose:
				c += man.pos
			c /= loose.size()
			var dir := (t - c).normalized() if t.distance_to(c) > 0.1 else Vector2.UP
			var side := Vector2(-dir.y, dir.x)
			for i in loose.size():
				var k := (i - (loose.size() - 1) * 0.5) * SLOT_SPACING
				_detach(loose[i], order, t + side * k)
		"stop":
			for man in loose:
				_detach(man, "hold", man.pos)
		"retreat":
			var base: Vector2 = MockMap.BASES[local_player]
			for i in loose.size():
				_detach(loose[i], "retreat", base + Vector2(i % 4, floorf(i / 4.0)) * SLOT_SPACING)
		"dig":
			# Split the line among the diggers: each takes a stretch and
			# works from its middle.
			var line: PackedVector3Array = data.polyline
			var total := _poly_len(line)
			for i in loose.size():
				var at := _poly_at(line, total * (i + 0.5) / loose.size())
				_detach(loose[i], "dig", Vector2(at.x, at.z))
			var by_squad := {}
			for man in loose:
				var sid: int = man_squad[man.id]
				if not by_squad.has(sid):
					by_squad[sid] = PackedInt32Array()
				by_squad[sid].append(man.id)
			for sid in by_squad:
				squads[sid].unit_dig = {"line": line, "len": total, "progress": 0.0, "men": by_squad[sid], "sound_t": 0.0}


func _detach(man: Dictionary, order: String, target: Vector2) -> void:
	man.detached = true
	man.order = order
	man.target = target
	man.throw = {}


func _rejoin(man: Dictionary) -> void:
	man.detached = false
	man.order = ""
	man.throw = {}


## Squad-level order: also clears `detached` for the whole squad.
func _order(sq: Dictionary, order: String, target: Vector2) -> void:
	sq.order = order
	sq.target = target
	sq.barrage = {}
	if order != "dig":
		sq.dig = {}
	sq.setup_t = 0.0
	sq.unit_dig = {}
	sq.follow = -1
	for man in sq.men:
		_rejoin(man)


## Throw / throw_each: eligible = alive, own, not pinned, carrying the
## item, target within range. One thrower (nearest, then lowest id) or all.
func _throw(uids: PackedInt32Array, item: String, target: Vector3, each: bool) -> void:
	var t := Vector2(target.x, target.z)
	var r: Vector2 = RULES.throw_range_m.get(item, Vector2(5, 30))
	var eligible: Array = []
	for man in _own_men(uids):
		var d: float = man.pos.distance_to(t)
		if int(man.items.get(item, 0)) > 0 and man.supp <= PINNED and man.throw.is_empty() and d >= r.x and d <= r.y:
			eligible.append(man)
	if eligible.is_empty():
		return
	if not each:
		eligible.sort_custom(func(a: Dictionary, b: Dictionary) -> bool:
			var da: float = a.pos.distance_to(t)
			var db: float = b.pos.distance_to(t)
			return da < db or (da == db and a.id < b.id))
		eligible = [eligible[0]]
	for man in eligible:
		_detach(man, "throw", man.pos)
		man.throw = {"item": item, "target": t, "t": WIND_UP_S}


## FollowUnits (S3 escort): whole squads get a squad-level follow, the
## other men follow one by one and stay detached. Vehicle ids are dropped.
func _follow(uids: PackedInt32Array, vid: int) -> void:
	var v: Dictionary = veh.vehicles.get(vid, {})
	if v.is_empty() or int(v.owner) != local_player:
		return
	var men := _own_men(uids)
	var per_squad := {}
	for man in men:
		var sid: int = man_squad[man.id]
		per_squad[sid] = int(per_squad.get(sid, 0)) + 1
	for man in men:
		var sq: Dictionary = squads[man_squad[man.id]]
		if per_squad[sq.id] == sq.men.size():
			if sq.order != "follow" or sq.follow != vid:
				_order(sq, "follow", sq.pos)
				sq.follow = vid
		else:
			_detach(man, "follow", man.pos)
			man.follow = vid


## Ends every escort of a vehicle (knocked out): the followers stop.
func _stop_followers(vid: int) -> void:
	for id in squads:
		var sq: Dictionary = squads[id]
		if sq.order == "follow" and sq.follow == vid:
			_order(sq, "idle", sq.pos)
		for man in sq.men:
			if man.detached and man.order == "follow" and man.get("follow", -1) == vid:
				man.order = "hold"


## Escort slots: single file 1.5 m apart, starting 4 m behind the vehicle,
## on the line from the nearest seen enemy through it (else straight
## behind). Also counts each vehicle's followers.
func _step_followers() -> void:
	veh.followers.clear()
	var files := {}
	var ids := squads.keys()
	ids.sort()
	for id in ids:
		var sq: Dictionary = squads[id]
		var men: Array = []
		if sq.order == "follow":
			var v: Dictionary = veh.vehicles.get(sq.follow, {})
			if v.is_empty() or int(v.owner) != sq.owner:
				_order(sq, "idle", sq.pos)
				continue
			men = sq.men.duplicate()
		else:
			for man in sq.men:
				if man.detached and man.order == "follow":
					men.append(man)
		var anchored := false
		for man in men:
			var vid: int = man.get("follow", -1) if man.detached else sq.follow
			var v: Dictionary = veh.vehicles.get(vid, {})
			if v.is_empty():
				continue
			var k: int = files.get(vid, 0)
			files[vid] = k + 1
			veh.followers[vid] = k + 1
			var back := Vector2(sin(v.yaw), cos(v.yaw))
			var foe := _nearest_enemy_at(v.pos, sq.owner, 250.0)
			if foe != -1:
				back = (v.pos - squads[foe].pos).normalized()
			var side := Vector2(-back.y, back.x)
			# Half a hull length, then 4 m, then the file; a ninth man starts
			# a second file beside the first.
			var behind := 2.9 + ESCORT_BACK_M + ESCORT_GAP_M * (k % 8)
			var slot: Vector2 = v.pos + back * behind + side * (1.5 * floorf(k / 8.0))
			if man.detached:
				man.target = slot
			elif not anchored:
				anchored = true
				sq.target = slot


func _at_fire(sq: Dictionary, dt: float) -> void:
	sq.rocket_t -= dt
	if sq.rocket_t > 0.0 or sq.rockets <= 0 or sq.men.is_empty():
		return
	var at: Dictionary = MockVehicles.AT_TEAM.get(String(players[sq.owner].faction), MockVehicles.AT_TEAM.germany)
	var eyes := _eyes(sq.owner)
	var best := -1
	var best_d: float = at.range
	for id in veh.vehicles:
		var v: Dictionary = veh.vehicles[id]
		if int(v.owner) < 0 or int(v.owner) == sq.owner:
			continue
		var d: float = v.pos.distance_to(sq.pos)
		# Still / moving engagement distances (S3 weapon defs).
		var lim: float = at.range if absf(v.speed) <= 1.0 else at.range * 0.6
		if d < minf(best_d, lim) and _sees(eyes, v.pos):
			best = id
			best_d = d
	if sq.get("attack", -1) in veh.vehicles and int(veh.vehicles[sq.attack].owner) >= 0:
		var tv: Dictionary = veh.vehicles[sq.attack]
		if tv.pos.distance_to(sq.pos) <= at.range:
			best = sq.attack
	if best == -1:
		return
	sq.rocket_t = ROCKET_RELOAD_S * (1.0 if sq.men.size() >= 2 else 2.0)
	sq.rockets -= 1
	veh.fire_rocket(sq, best)


# --- simulation stand-in -----------------------------------------------------

func _step(dt: float) -> void:
	_tick += 1
	time_s += dt
	var held := [0, 0]
	for p in points:
		if p.owner >= 0:
			held[p.owner] += 1
	for i in 2:
		var pl := players[i]
		var mult: float = 1.0 if i == local_player else DIFFICULTY_INCOME.get(setup.get("difficulty", "normal"), 1.0)
		pl.income = (2.5 + 0.8 * held[i]) * mult
		pl.manpower += pl.income * dt
		pl.points_held = held[i]
		stats[i].points_held_s += held[i] * dt
	for type in recall_cooldown.keys():
		recall_cooldown[type][0] -= dt
		if recall_cooldown[type][0] <= 0.0:
			recall_cooldown.erase(type)
	_step_queues(dt)
	_ai_t -= dt
	if _ai_t <= 0.0:
		_ai_t = 2.0
		_ai_plan(1 - local_player)
	_step_followers()
	var ids := squads.keys()
	ids.sort()
	for id in ids:
		if squads.has(id):
			_step_squad(squads[id], dt)
	veh.step(dt)
	_step_shells(dt)
	_step_thrown(dt)
	_step_smoke(dt)
	_step_capture(dt)
	_step_tickets(dt, held)
	_check_end()


func _step_queues(dt: float) -> void:
	for i in 2:
		var q: Array = _queues[i]
		if q.is_empty():
			continue
		var head: Dictionary = q[0]
		head.remaining_s -= dt
		if head.remaining_s <= 0.0:
			q.pop_front()
			var base: Vector2 = MockMap.BASES[i]
			var at := base + Vector2(rng.randf_range(-25, 25), rng.randf_range(-25, 25))
			if head.has("vehicle"):
				veh.spawn(i, head.vehicle, at, Vehicles.yaw_toward(-at.x, -at.y))
				if i == local_player:
					_push_event(i, "vehicle_arrived", "%s arrived." % MockVehicles.TYPES[head.vehicle].short, _v3(at))
				continue
			var id := _spawn(i, head.type, at)
			if i == local_player:
				_push_event(i, "squad_arrived", "%s arrived at base." % squads[id].name, _v3(at))


## The call-in catalog of a player: the S1 squads, the faction's AT team
## and its tank (S3), each with a squad type id (`id`).
func catalog(owner: int) -> Array:
	var faction: String = players[owner].faction
	var short := "ger" if faction == "germany" else "usa"
	var out: Array = []
	for c in CATALOG:
		var e: Dictionary = c.duplicate()
		e.id = "ww2:%s_%s" % [short, c.type]
		e.pop = int(c.men)
		if c.type == "at":
			var at: Dictionary = MockVehicles.AT_TEAM.get(faction, MockVehicles.AT_TEAM.germany)
			e.name = at.name
			e.short = at.short
			e.cost = int(at.cost)
		out.append(e)
	var vkey := veh.faction_tank(owner)
	var vt: Dictionary = MockVehicles.TYPES[vkey]
	out.append({
		"id": vt.id + "_squad", "type": "vehicle", "vehicle": vkey, "name": vt.name, "short": vt.short,
		"cost": int(vt.cost), "delay_s": float(vt.call_in_s), "men": 0, "pop": int(vt.pop),
	})
	return out


## Catalog entry by squad type id or by type ("" or unknown: {}).
func catalog_entry(owner: int, key: String) -> Dictionary:
	for e in catalog(owner):
		if e.id == key or e.type == key:
			return e
	return {}


func _call_in(owner: int, type: String) -> bool:
	return _call_in_entry(owner, catalog_entry(owner, type))


func _call_in_entry(owner: int, def: Dictionary) -> bool:
	if def.is_empty():
		return false
	var type: String = def.type
	var pl := players[owner]
	var queued := 0
	for q in _queues[owner]:
		queued += int(q.get("pop", q.men))
	if pl.manpower < def.cost or _queues[owner].size() >= QUEUE_MAX or _pop(owner) + queued + int(def.pop) > RULES.pop_cap:
		return false
	if owner == local_player and recall_cooldown.has(type):
		return false
	if type == "vehicle" and veh.call_in_block(owner) != "":
		return false
	pl.manpower -= def.cost
	var entry := {"type": type, "id": def.id, "remaining_s": float(def.delay_s), "total_s": float(def.delay_s), "men": int(def.men), "pop": int(def.pop)}
	if def.has("vehicle"):
		entry.vehicle = def.vehicle
	_queues[owner].append(entry)
	stats[owner].squads_called += 1
	if owner == local_player and type == "mg":
		recall_cooldown[type] = [12.0, 12.0]
	return true


## Population: living soldiers plus each living vehicle's pop (S3).
func _pop(owner: int) -> int:
	var n := veh.pop(owner)
	for id in squads:
		if squads[id].owner == owner:
			n += squads[id].men.size()
	return n


func _ai_plan(ai: int) -> void:
	# Buy: keep a mix, cheapest first when poor.
	var want := ["rifle", "rifle", "mg", "engineer", "mortar", "at"]
	_call_in(ai, want[rng.randi() % want.size()])
	veh.ai_plan(ai)
	for id in squads:
		var sq: Dictionary = squads[id]
		if sq.owner != ai:
			continue
		if sq.men.size() * 1.0 / sq.men_max < 0.4 and sq.order != "retreat":
			_order(sq, "retreat", MockMap.BASES[ai])
			continue
		if sq.type == "mortar":
			if sq.order != "barrage" and rng.randf() < 0.25:
				var tgt := _nearest_enemy(sq, RULES.mortar_max_m)
				if tgt != -1 and squads[tgt].pos.distance_to(sq.pos) > RULES.mortar_min_m:
					sq.order = "barrage"
					sq.barrage = {"target": squads[tgt].pos, "rounds": 6, "t": 1.0}
			continue
		if sq.order == "idle" and rng.randf() < 0.5:
			var best := -1
			var best_d := INF
			for i in points.size():
				if points[i].owner == ai:
					continue
				var p: Vector3 = points[i].pos
				var d: float = sq.pos.distance_to(Vector2(p.x, p.z)) + rng.randf() * 150.0
				if d < best_d:
					best_d = d
					best = i
			if best != -1:
				var p: Vector3 = points[best].pos
				_order(sq, "attack_move", Vector2(p.x, p.z) + Vector2(rng.randf_range(-10, 10), rng.randf_range(-10, 10)))


## Whether a spotting ray from `a` to `b` passes through smoke.
func _smoked(a: Vector2, b: Vector2) -> bool:
	for c in _smokes:
		var r := _smoke_radius(c)
		if r <= 0.5 or _smoke_density(c) < 0.5:
			continue
		var cp := Geometry2D.get_closest_point_to_segment(c.pos, a, b)
		if cp.distance_to(c.pos) < r:
			return true
	return false


## Points `viewer` sees from: squad anchors and detached men.
func _eyes(viewer: int) -> PackedVector2Array:
	var out := PackedVector2Array()
	for id in squads:
		var o: Dictionary = squads[id]
		if o.owner != viewer:
			continue
		out.append(o.pos)
		for man in o.men:
			if man.detached:
				out.append(man.pos)
	# Vehicles look out too (S3: vehicles as observers).
	for id in veh.vehicles:
		var v: Dictionary = veh.vehicles[id]
		if int(v.owner) == viewer:
			out.append(v.pos)
	return out


func _sees(eyes: PackedVector2Array, p: Vector2) -> bool:
	var sight := SIGHT_M * (0.5 if MockMap.in_woods(p) else 1.0)
	for e in eyes:
		if e.distance_to(p) <= sight and not _smoked(e, p):
			return true
	return false


## Nearest squad of the other side that `owner` sees within `max_d` of `at`.
func _nearest_enemy_at(at: Vector2, owner: int, max_d: float) -> int:
	var best := -1
	var best_d := max_d
	var eyes := _eyes(owner)
	for id in squads:
		var o: Dictionary = squads[id]
		if o.owner == owner:
			continue
		var d: float = o.pos.distance_to(at)
		if d < best_d and _sees(eyes, o.pos):
			best_d = d
			best = id
	return best


func _nearest_enemy(sq: Dictionary, max_d: float) -> int:
	var best := -1
	var best_d := max_d
	var eyes := _eyes(sq.owner)
	for id in squads:
		var o: Dictionary = squads[id]
		if o.owner == sq.owner:
			continue
		var d: float = o.pos.distance_to(sq.pos)
		if d < best_d and _sees(eyes, o.pos):
			best_d = d
			best = id
	return best


func _suppression(sq: Dictionary) -> float:
	if sq.men.is_empty():
		return 0.0
	var s := 0.0
	for man in sq.men:
		s += man.supp
	return s / sq.men.size()


func _status(sq: Dictionary) -> String:
	if sq.order == "retreat":
		return "retreating"
	var s := _suppression(sq)
	if s > PINNED:
		return "pinned"
	if s > SUPPRESSED:
		return "suppressed"
	return "ok"


## Formation slot of a man around the squad anchor: a line across the
## facing, SLOT_SPACING apart, with a little jitter.
func _slot_pos(sq: Dictionary, slot: int) -> Vector2:
	var n: int = sq.men_max
	var f: Vector2 = sq.facing
	var side := Vector2(-f.y, f.x)
	var k := (slot - (n - 1) * 0.5) * SLOT_SPACING
	var jitter := Vector2(sin(sq.id * 3.1 + slot * 1.7), cos(sq.id * 1.3 + slot * 2.3)) * 0.8
	return sq.pos + side * k + f * (absf(k) * -0.15) + jitter


## Effective stance: the override (or Auto: stand while moving, else
## crouch); suppression forces lower, never higher.
func _stance(man: Dictionary, moving: bool) -> String:
	var order := ["stand", "crouch", "prone"]
	var want: String = man.stance_override
	if want == "auto":
		want = "stand" if moving else "crouch"
	var forced := "stand"
	if man.supp > PINNED:
		forced = "prone"
	elif man.supp > SUPPRESSED:
		forced = "crouch"
	return order[maxi(order.find(want), order.find(forced))]


func _step_squad(sq: Dictionary, dt: float) -> void:
	for man in sq.men:
		man.supp = maxf(man.supp - 0.12 * dt, 0.0)
		if man.get("pin_t", 0.0) > 0.0:
			man.pin_t -= dt
			man.supp = maxf(man.supp, 0.9)
	var status := _status(sq)
	var enemy := -1
	if sq.type != "mortar" and sq.order != "retreat":
		enemy = _nearest_enemy(sq, RANGE[sq.type])
	# AT team: the gunner fires at a seen enemy vehicle in range.
	if sq.type == "at" and status != "pinned":
		_at_fire(sq, dt)
	# Squad anchor movement.
	var moving: bool = sq.order in ["move", "attack_move", "retreat", "dig", "follow"]
	if sq.order == "attack_move" and enemy != -1:
		moving = false
	if sq.order == "dig" and sq.pos.distance_to(sq.target) < 2.0:
		moving = false
	var speed: float = SPEED[sq.type] * (1.6 if sq.order == "retreat" else 1.0) * (0.5 if status == "suppressed" else 1.0)
	if moving and status != "pinned":
		var to: Vector2 = sq.target - sq.pos
		var step := speed * dt
		if to.length() <= step:
			sq.pos = sq.target
			if sq.order != "dig" and sq.order != "follow":
				sq.order = "setup" if sq.type == "mg" else "idle"
				sq.setup_t = 3.0
		else:
			sq.facing = to.normalized()
			sq.pos += to.normalized() * step
	if sq.order == "setup":
		sq.setup_t -= dt
		if sq.setup_t <= 0.0:
			sq.order = "idle"
	_step_men(sq, dt, speed, enemy != -1)
	# Digging (squad order).
	if sq.order == "dig" and not moving and not sq.dig.is_empty():
		var rate := 20.0 / 210.0 * (2.0 if sq.type == "engineer" else 1.0) * DIG_SPEEDUP
		sq.dig.progress = minf(sq.dig.progress + rate * dt / maxf(sq.dig.len, 1.0), 1.0)
		_dig_sound(sq.dig, dt)
		if sq.dig.progress >= 1.0:
			stats[sq.owner].trench_m += sq.dig.len
			if sq.owner == local_player:
				_push_event(sq.owner, "info", "%s finished a %d m trench." % [sq.name, roundi(sq.dig.len)], _v3(sq.pos))
			sq.order = "idle"
			sq.dig = {}
	# Digging (detached men): progress per man at his spot.
	if not sq.unit_dig.is_empty():
		var job: Dictionary = sq.unit_dig
		var n := 0
		for man in sq.men:
			if job.men.has(man.id) and man.detached and man.order == "dig" and man.pos.distance_to(man.target) < 1.5:
				n += 1
		if n > 0:
			var per_man := 20.0 / 210.0 / 8.0 * (2.0 if sq.type == "engineer" else 1.0) * DIG_SPEEDUP
			job.progress = minf(job.progress + per_man * n * dt / maxf(job.len, 1.0), 1.0)
			_dig_sound(job, dt)
		if job.progress >= 1.0:
			stats[sq.owner].trench_m += job.len
			for man in sq.men:
				if job.men.has(man.id) and man.order == "dig":
					man.order = "hold"
			sq.unit_dig = {}
	# Mortar barrage.
	if sq.order == "barrage" and not sq.barrage.is_empty() and status != "pinned":
		sq.barrage.t -= dt
		if sq.barrage.t <= 0.0:
			sq.barrage.t = 3.5
			sq.barrage.rounds -= 1
			_effects.append({"kind": "mortar_launch", "pos": _v3(sq.pos)})
			var tgt: Vector2 = sq.barrage.target + Vector2(rng.randfn(0, 9), rng.randfn(0, 9))
			_shells.append({"t": 2.5, "pos": tgt, "owner": sq.owner})
			if sq.barrage.rounds <= 0:
				sq.order = "idle"
				sq.barrage = {}
	# Small arms (whole squad, from the anchor).
	if enemy != -1 and status != "pinned" and not sq.men.is_empty():
		sq.fire_t -= dt
		if sq.fire_t <= 0.0:
			var gap: float = FIRE_GAP.get(sq.type, 0.7)
			sq.fire_t = gap * (0.6 + rng.randf() * 0.8) * (6.0 / maxf(sq.men.size(), 1.0) if sq.type == "rifle" else 1.0)
			if sq.type == "mg" and rng.randf() < 0.08:
				sq.fire_t = 1.5  # burst gap
			_effects.append({"kind": "shot", "weapon": WEAPON[sq.type], "pos": _v3(sq.pos)})
			_shot_at(sq, squads[enemy])


func _step_men(sq: Dictionary, dt: float, speed: float, engaged: bool) -> void:
	var base: Vector2 = MockMap.BASES[sq.owner]
	for man in sq.men:
		man.moving = false
		# Resupply inside the own base zone.
		if man.pos.distance_to(base) < MockMap.BASE_RADIUS:
			man.resupply_t += dt
			if man.resupply_t >= RESUPPLY_S:
				man.resupply_t = 0.0
				var full := loadout(sq.type, man.slot)
				for item in full:
					if man.items[item] < full[item]:
						man.items[item] += 1
						break
		if man.supp > PINNED:
			continue
		if not man.throw.is_empty():
			man.throw.t -= dt
			if man.throw.t <= 0.0:
				_release_throw(sq, man)
			continue
		var goal: Vector2
		var pace := speed
		if man.detached:
			if not man.order in ["move", "attack_move", "retreat", "dig", "follow"]:
				continue
			if man.order == "attack_move" and engaged:
				continue
			goal = man.target
			if man.order == "retreat":
				pace *= 1.6
		else:
			goal = _slot_pos(sq, man.slot)
			pace *= 1.4  # catch up with the formation
		var to: Vector2 = goal - man.pos
		var step := pace * dt
		if to.length() <= step:
			man.pos = goal
			if man.detached and man.order in ["move", "attack_move", "retreat"]:
				man.order = "hold"
		else:
			man.pos += to.normalized() * step
			man.moving = to.length() > 0.6


func _release_throw(sq: Dictionary, man: Dictionary) -> void:
	var item: String = man.throw.item
	var t: Vector2 = man.throw.target
	man.throw = {}
	man.order = "hold"
	if int(man.items.get(item, 0)) <= 0:
		return
	man.items[item] -= 1
	var d: float = man.pos.distance_to(t)
	# Scatter: 6 % of the distance.
	var land := t + Vector2(rng.randfn(0.0, 1.0), rng.randfn(0.0, 1.0)) * d * 0.06
	var dur := maxf(d / THROW_SPEED, 0.3)
	_thrown.append({
		"kind": item, "from": man.pos, "to": land, "t": 0.0, "dur": dur, "owner": sq.owner,
		"fuse": maxf(float(FUSE_S.get(item, 0.0)) - WIND_UP_S - dur, 0.2), "landed": false,
	})


func _step_thrown(dt: float) -> void:
	var done: Array[Dictionary] = []
	for th in _thrown:
		if not th.landed:
			th.t += dt
			if th.t >= th.dur:
				th.landed = true
				if th.kind == "smoke":
					_smokes.append({"pos": th.to, "age": 0.0})
					while _smokes.size() > SMOKE_CAP:
						_smokes.pop_front()
					done.append(th)
			continue
		th.fuse -= dt
		if th.fuse <= 0.0:
			_explode(th.to, th.kind, th.owner)
			done.append(th)
	for th in done:
		_thrown.erase(th)


func _thrown_pos(th: Dictionary) -> Vector3:
	var f := clampf(th.t / th.dur, 0.0, 1.0)
	var p: Vector2 = th.from.lerp(th.to, f)
	var arc: float = sin(PI * f) * th.from.distance_to(th.to) * 0.25
	return _v3(p) + Vector3(0, 1.5 * (1.0 - f) + arc + 0.1, 0)


func _step_smoke(dt: float) -> void:
	for c in _smokes:
		c.age += dt
	_smokes = _smokes.filter(func(c: Dictionary) -> bool: return c.age < SMOKE_GROW_S + SMOKE_HOLD_S + SMOKE_THIN_S)


func _smoke_radius(c: Dictionary) -> float:
	return SMOKE_R * clampf(c.age / SMOKE_GROW_S, 0.0, 1.0)


func _smoke_density(c: Dictionary) -> float:
	var thin: float = c.age - SMOKE_GROW_S - SMOKE_HOLD_S
	return 1.0 if thin <= 0.0 else clampf(1.0 - thin / SMOKE_THIN_S, 0.0, 1.0)


func _dig_sound(job: Dictionary, dt: float) -> void:
	job.sound_t -= dt
	if job.sound_t <= 0.0:
		job.sound_t = 0.6 + rng.randf() * 0.6
		var line: PackedVector3Array = job.line
		_effects.append({"kind": "dig", "pos": line[rng.randi() % line.size()]})


func _shot_at(shooter: Dictionary, target: Dictionary) -> void:
	if target.men.is_empty():
		return
	var d: float = shooter.pos.distance_to(target.pos)
	var p := 0.05 * clampf(1.4 - d / 200.0, 0.2, 1.2)
	if target.order == "idle" and target.get("in_trench", false):
		p *= 0.35
	var man: Dictionary = target.men[rng.randi() % target.men.size()]
	if man.supp > PINNED:
		p *= 0.5
	var s := 0.12 if shooter.type == "mg" else 0.05
	man.supp = minf(man.supp + s, 1.0)
	for other in target.men:
		if other.pos.distance_to(man.pos) < 6.0:
			other.supp = minf(other.supp + s * 0.3, 1.0)
	if rng.randf() < p:
		_hurt(target, man, rng.randf_range(35.0, 100.0), shooter.owner)


func _hurt(sq: Dictionary, man: Dictionary, dmg: float, by: int) -> void:
	man.hp -= dmg
	if man.hp <= 0.0:
		sq.men.erase(man)
		man_squad.erase(man.id)
		stats[by].kills += 1
		stats[sq.owner].losses += 1
		if sq.unit_dig.has("men"):
			var i: int = sq.unit_dig.men.find(man.id)
			if i >= 0:
				sq.unit_dig.men.remove_at(i)
	if sq.men.is_empty():
		if sq.owner == local_player:
			_push_event(sq.owner, "squad_lost", "%s lost." % sq.name, _v3(sq.pos))
		squads.erase(sq.id)
	elif sq.owner == local_player and time_s - sq.get("alert_t", -99.0) > 30.0:
		sq.alert_t = time_s
		_push_event(sq.owner, "under_attack", "%s under fire." % sq.name, _v3(sq.pos))


## HE at `at`: lethal core, damage falloff, suppression (radii in BLAST).
func _explode(at: Vector2, kind: String, owner: int) -> void:
	_effects.append({"kind": "explosion", "pos": _v3(at), "size": kind})
	var b: Vector3 = BLAST.get(kind, BLAST.mortar)
	var ids := squads.keys()
	ids.sort()
	for id in ids:
		if not squads.has(id):
			continue
		var sq: Dictionary = squads[id]
		for man in sq.men.duplicate():
			if not squads.has(id):
				break
			var d: float = man.pos.distance_to(at)
			if d < b.z:
				man.supp = minf(man.supp + 0.6 * (1.0 - d / b.z), 1.0)
			if d < b.x:
				_hurt(sq, man, 100.0, owner)
			elif d < b.y and rng.randf() < 0.7 * (1.0 - d / b.y):
				_hurt(sq, man, rng.randf_range(30.0, 90.0), owner)


func _step_shells(dt: float) -> void:
	var landed: Array[Dictionary] = []
	for s in _shells:
		s.t -= dt
		if s.t <= 0.0:
			landed.append(s)
	for s in landed:
		_shells.erase(s)
		_explode(s.pos, "mortar", s.owner)


func _step_capture(dt: float) -> void:
	for p in points:
		var men := [0, 0]
		var pp := Vector2(p.pos.x, p.pos.z)
		for id in squads:
			var sq: Dictionary = squads[id]
			if sq.pos.distance_to(pp) <= p.radius + 6.0:
				men[sq.owner] += sq.men.size()
		p.contested = men[0] > 0 and men[1] > 0
		if p.contested:
			continue
		var side := 0 if men[0] > 0 else (1 if men[1] > 0 else -1)
		if side == -1:
			p.progress = maxf(p.progress - dt / CAPTURE_S * 0.5, 0.0)
			if p.progress == 0.0:
				p.capturing = -1
			continue
		if p.owner == side:
			p.capturing = -1
			p.progress = 0.0
			continue
		if p.capturing != side:
			p.capturing = side
			p.progress = 0.0
		p.progress += dt / CAPTURE_S * minf(0.8 + men[side] * 0.05, 1.5)
		if p.progress >= 1.0:
			var lost: int = p.owner
			p.owner = side
			p.capturing = -1
			p.progress = 0.0
			if side == local_player:
				_push_event(side, "point_captured", "Point %s captured." % p.id, p.pos)
			elif lost == local_player:
				_push_event(local_player, "point_lost", "Point %s lost." % p.id, p.pos)


func _step_tickets(dt: float, held: Array) -> void:
	for i in 2:
		var diff: int = held[1 - i] - held[i]
		var drain := 0.6 * diff if diff > 0 else 0.0
		var before: float = players[i].tickets
		players[i].tickets = maxf(before - drain * dt, 0.0)
		players[i].ticket_drain = drain
		if i == local_player and before > 100.0 and players[i].tickets <= 100.0:
			_push_event(i, "tickets_low", "Tickets below 100.", null)


func _check_end() -> void:
	for i in 2:
		if players[i].tickets <= 0.0:
			_end(1 - i, "tickets")
			return
		if _pop(i) == 0 and _queues[i].is_empty() and players[i].manpower < 120.0:
			_end(1 - i, "annihilation")
			return


func _end(winner: int, reason: String) -> void:
	if result.status != "running":
		return
	result = {"status": "won" if winner == local_player else "lost", "winner": winner, "reason": reason}


func _push_event(player_i: int, kind: String, text: String, pos: Variant) -> void:
	var e := {"kind": kind, "text": text, "player": player_i, "time_s": time_s}
	if pos != null:
		e.pos = pos
	_events.append(e)


func _poly_len(line: PackedVector3Array) -> float:
	var n := 0.0
	for i in range(1, line.size()):
		n += line[i - 1].distance_to(line[i])
	return n


## Point at `dist` m along a polyline.
func _poly_at(line: PackedVector3Array, dist: float) -> Vector3:
	for i in range(1, line.size()):
		var seg := line[i - 1].distance_to(line[i])
		if dist <= seg:
			return line[i - 1].lerp(line[i], dist / maxf(seg, 0.001))
		dist -= seg
	return line[line.size() - 1]


# --- export ------------------------------------------------------------------

func _build_state() -> Dictionary:
	for i in 2:
		players[i].pop = _pop(i)
	var out_players: Array = []
	for pl in players:
		out_players.append(pl.duplicate())
	var out_points: Array = []
	for p in points:
		out_points.append(p.duplicate())
	var out_squads: Array = []
	var out_soldiers: Array = []
	var eyes := _eyes(local_player)
	var ids := squads.keys()
	ids.sort()
	for id in ids:
		var sq: Dictionary = squads[id]
		var own: bool = sq.owner == local_player
		var seen: Array = []
		for man in sq.men:
			if own or _sees(eyes, man.pos):
				seen.append(man)
		if seen.is_empty():
			continue
		out_squads.append(_squad_state(sq, seen))
		for man in seen:
			out_soldiers.append(_soldier_state(sq, man, own))
	out_soldiers.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return a.id < b.id)
	var q: Array = []
	for e in _queues[local_player]:
		q.append(e.duplicate())
	var catalog: Array = []
	for entry in catalog(local_player):
		if recall_cooldown.has(entry.type):
			entry.cooldown_s = recall_cooldown[entry.type][0]
			entry.cooldown_total_s = recall_cooldown[entry.type][1]
		if entry.type == "vehicle" and veh.locked_s() > 0.0:
			entry.locked_s = veh.locked_s()
		catalog.append(entry)
	for i in 2:
		out_players[i].vehicles = veh.count(i)
		out_players[i].vehicle_limit = veh.limit
	var rules: Dictionary = RULES.duplicate()
	rules.merge(veh.rules())
	var vs := veh.export_state(eyes)
	var smokes: Array = []
	for c in _smokes:
		smokes.append({"pos": _v3(c.pos), "radius": _smoke_radius(c), "density": _smoke_density(c)})
	var thrown: Array = []
	for th in _thrown:
		thrown.append({"kind": th.kind, "pos": _thrown_pos(th) if not th.landed else _v3(th.to) + Vector3(0, 0.1, 0), "landed": th.landed})
	var state := {
		"tick": _tick,
		"time_s": time_s,
		"paused": paused,
		"local_player": local_player,
		"map": {"id": setup.get("map", "kessel_ridge"), "name": "Kessel Ridge", "size_m": MockMap.SIZE_M, "minimap": _minimap},
		"rules": rules,
		"players": out_players,
		"points": out_points,
		"squads": out_squads,
		"soldiers": out_soldiers,
		"smokes": smokes,
		"thrown": thrown,
		"callin": {"catalog": catalog, "queue": q, "queue_max": QUEUE_MAX, "vehicles": veh.count(local_player), "vehicle_limit": veh.limit},
		"vehicles": vs.vehicles,
		"shells": vs.shells,
		"armour_events": vs.armour_events,
		"events": _events,
		"effects": _effects,
		"result": result.duplicate(),
		"stats": stats.duplicate(true),
	}
	_events = []
	_effects = []
	return state


func _squad_state(sq: Dictionary, seen: Array) -> Dictionary:
	var men: int = sq.men.size()
	var hp_sum := 0.0
	var units := PackedInt32Array()
	var items := {"frag": 0, "smoke": 0, "satchel": 0}
	for man in sq.men:
		hp_sum += man.hp
	for man in seen:
		units.append(man.id)
		for k in items:
			items[k] += int(man.items.get(k, 0))
	units.sort()
	var out := {
		"id": sq.id,
		"owner": sq.owner,
		"type": sq.type,
		"name": sq.name,
		"men": men,
		"men_max": sq.men_max,
		"health": hp_sum / (100.0 * sq.men_max),
		"suppression": _suppression(sq),
		"status": _status(sq),
		"order": sq.order,
		"pos": _v3(sq.pos),
		"units": units,
	}
	if sq.owner == local_player:
		out.items = items
		if not sq.dig.is_empty():
			out.dig_line = sq.dig.line
			out.dig_progress = sq.dig.progress
		elif not sq.unit_dig.is_empty():
			out.dig_line = sq.unit_dig.line
			out.dig_progress = sq.unit_dig.progress
	if not sq.barrage.is_empty():
		out.target = _v3(sq.barrage.target)
	elif sq.order in ["move", "attack_move", "retreat"]:
		out.target = _v3(sq.target)
	# Escort (S3, opt; a proposed contract key): the vehicle followed.
	if sq.order == "follow" and sq.owner == local_player:
		out.follow = sq.follow
	return out


func _soldier_state(sq: Dictionary, man: Dictionary, own: bool) -> Dictionary:
	var moving: bool = man.get("moving", false)
	var out := {
		"id": man.id,
		"squad": sq.id,
		"owner": sq.owner,
		"slot": man.slot,
		"pos": _v3(man.pos),
		"hp": man.hp,
		"stance": _stance(man, moving),
		"pinned": man.supp > PINNED,
		"suppression": man.supp,
	}
	if own:
		out.stance_override = man.stance_override
		out.detached = man.detached
		out.items = man.items.duplicate()
		if man.detached:
			out.order = man.order
			if man.order in ["move", "attack_move", "retreat", "dig"]:
				out.target = _v3(man.target)
			elif man.order == "follow":
				out.follow = man.get("follow", -1)
			elif man.order == "throw" and not man.throw.is_empty():
				out.target = _v3(man.throw.target)
	return out


## Staging: every AT team with a vehicle in range fires now.
func at_volley() -> void:
	for id in squads:
		var sq: Dictionary = squads[id]
		if sq.type == "at":
			sq.rocket_t = 0.0
			_at_fire(sq, 0.0)
