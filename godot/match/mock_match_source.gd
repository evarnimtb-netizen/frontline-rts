class_name MockMatchSource
extends Node
## Fake match for running and screenshotting the HUD without the sim. It
## produces the same Dictionary as the bridge will (docs/match_state.md)
## and accepts the same intents, with a crude stand-in for S1 rules:
## movement, capture, ticket drain, income, call-ins, dice-roll firefights
## with suppression, mortar barrages, digging, fog by distance, and an
## enemy that walks to points and buys squads.
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
]
const RULES := {
	"pop_cap": 120,
	"tickets_max": 500,
	"dig_max_m": 60.0,
	"mortar_min_m": 60.0,
	"mortar_max_m": 600.0,
	"capture_radius_m": 15.0,
}
const SPEED := {"rifle": 3.4, "mg": 2.8, "engineer": 3.4, "mortar": 2.6}
const RANGE := {"rifle": 160.0, "mg": 200.0, "engineer": 70.0, "mortar": 0.0}
const FIRE_GAP := {"rifle": 0.7, "mg": 0.12, "engineer": 0.25}
const WEAPON := {"rifle": "rifle", "mg": "mg", "engineer": "smg"}
const SIGHT_M := 160.0
const CAPTURE_S := 20.0
const QUEUE_MAX := 5
## Mock digging runs this much faster than §5.10.9 pacing so it shows.
const DIG_SPEEDUP := 6.0
const DIFFICULTY_INCOME := {"easy": 0.8, "normal": 1.0, "hard": 1.25}

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
var stats: Array[Dictionary] = []
var result := {"status": "running", "winner": -1, "reason": ""}

var _next_id := 1
var _queues: Array = [[], []]
var _events: Array[Dictionary] = []
var _effects: Array[Dictionary] = []
var _shells: Array[Dictionary] = []
var _ai_t := 0.0
var _tick := 0
var _minimap: Texture2D


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
	_next_id = 1
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
	_push_event(0, "info", "Skirmish on Kessel Ridge. Hold the points.", null)


func height_at(x: float, z: float) -> float:
	return MockMap.height(x, z)


func _v3(p: Vector2) -> Vector3:
	return Vector3(p.x, MockMap.height(p.x, p.y), p.y)


func _spawn(owner: int, type: String, at: Vector2) -> int:
	var def: Dictionary = {}
	for c in CATALOG:
		if c.type == type:
			def = c
	var id := _next_id
	_next_id += 1
	var hp: Array[float] = []
	for i in int(def.men):
		hp.append(100.0)
	var face := (Vector2.ZERO - at).normalized()
	squads[id] = {
		"id": id, "owner": owner, "type": type, "name": def.name,
		"hp": hp, "men_max": int(def.men), "suppression": 0.0,
		"order": "idle", "pos": at, "target": at, "facing": face,
		"fire_t": rng.randf() * 2.0, "setup_t": 0.0, "dig": {}, "barrage": {},
	}
	return id


# --- source interface --------------------------------------------------------

func handle_intent(kind: StringName, data: Dictionary) -> void:
	var ids: PackedInt32Array = data.get("squads", PackedInt32Array())
	match kind:
		&"move", &"attack_move":
			var t: Vector3 = data.target
			var n := 0
			for id in ids:
				var sq: Dictionary = squads.get(id, {})
				if sq.is_empty() or sq.owner != local_player:
					continue
				# Spread the group around the target so they do not stack.
				var off := Vector2(n % 3 - 1, floorf(n / 3.0)) * 14.0
				_order(sq, String(kind), Vector2(t.x, t.z) + off)
				n += 1
		&"stop":
			for id in ids:
				var sq: Dictionary = squads.get(id, {})
				if not sq.is_empty() and sq.owner == local_player:
					sq.order = "idle"
					sq.target = sq.pos
					sq.dig = {}
					sq.barrage = {}
		&"retreat":
			for id in ids:
				var sq: Dictionary = squads.get(id, {})
				if not sq.is_empty() and sq.owner == local_player:
					_order(sq, "retreat", MockMap.BASES[local_player])
		&"dig":
			var line: PackedVector3Array = data.polyline
			for id in ids:
				var sq: Dictionary = squads.get(id, {})
				if sq.is_empty() or sq.owner != local_player or not sq.type in ["rifle", "engineer"]:
					continue
				_order(sq, "dig", Vector2(line[0].x, line[0].z))
				sq.dig = {"line": line, "len": _poly_len(line), "progress": 0.0, "sound_t": 0.0}
		&"barrage":
			var t: Vector3 = data.target
			for id in ids:
				var sq: Dictionary = squads.get(id, {})
				if not sq.is_empty() and sq.owner == local_player and sq.type == "mortar":
					sq.order = "barrage"
					sq.target = sq.pos
					sq.barrage = {"target": Vector2(t.x, t.z), "rounds": 8, "t": 0.5}
		&"call_in":
			_call_in(local_player, data.type)
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
## (screenshot staging).
func place_squad(id: int, at: Vector2, facing := Vector2.UP) -> void:
	if not squads.has(id):
		return
	var sq: Dictionary = squads[id]
	_order(sq, "idle", at)
	sq.pos = at
	sq.facing = facing.normalized()


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


func set_minimap_texture(tex: Texture2D) -> void:
	_minimap = tex


# --- simulation stand-in -----------------------------------------------------

func _order(sq: Dictionary, order: String, target: Vector2) -> void:
	sq.order = order
	sq.target = target
	sq.barrage = {}
	if order != "dig":
		sq.dig = {}
	sq.setup_t = 0.0


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
	_step_queues(dt)
	_ai_t -= dt
	if _ai_t <= 0.0:
		_ai_t = 2.0
		_ai_plan(1 - local_player)
	var ids := squads.keys()
	ids.sort()
	for id in ids:
		if squads.has(id):
			_step_squad(squads[id], dt)
	_step_shells(dt)
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
			var id := _spawn(i, head.type, at)
			if i == local_player:
				_push_event(i, "squad_arrived", "%s arrived at base." % squads[id].name, _v3(at))


func _call_in(owner: int, type: String) -> bool:
	var def: Dictionary = {}
	for c in CATALOG:
		if c.type == type:
			def = c
	if def.is_empty():
		return false
	var pl := players[owner]
	var queued := 0
	for q in _queues[owner]:
		queued += int(q.men)
	if pl.manpower < def.cost or _queues[owner].size() >= QUEUE_MAX or _pop(owner) + queued + def.men > RULES.pop_cap:
		return false
	pl.manpower -= def.cost
	_queues[owner].append({"type": type, "remaining_s": float(def.delay_s), "total_s": float(def.delay_s), "men": def.men})
	stats[owner].squads_called += 1
	return true


func _pop(owner: int) -> int:
	var n := 0
	for id in squads:
		if squads[id].owner == owner:
			n += squads[id].hp.size()
	return n


func _ai_plan(ai: int) -> void:
	# Buy: keep a mix, cheapest first when poor.
	var want := ["rifle", "rifle", "mg", "engineer", "mortar"]
	_call_in(ai, want[rng.randi() % want.size()])
	for id in squads:
		var sq: Dictionary = squads[id]
		if sq.owner != ai:
			continue
		if sq.hp.size() * 1.0 / sq.men_max < 0.4 and sq.order != "retreat":
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


func _visible_to(viewer: int, sq: Dictionary) -> bool:
	if sq.owner == viewer:
		return true
	var sight := SIGHT_M * (0.5 if MockMap.in_woods(sq.pos) else 1.0)
	for id in squads:
		var o: Dictionary = squads[id]
		if o.owner == viewer and o.pos.distance_to(sq.pos) <= sight:
			return true
	return false


func _nearest_enemy(sq: Dictionary, max_d: float) -> int:
	var best := -1
	var best_d := max_d
	for id in squads:
		var o: Dictionary = squads[id]
		if o.owner == sq.owner or not _visible_to(sq.owner, o):
			continue
		var d: float = o.pos.distance_to(sq.pos)
		if d < best_d:
			best_d = d
			best = id
	return best


func _status(sq: Dictionary) -> String:
	if sq.order == "retreat":
		return "retreating"
	if sq.suppression > 0.7:
		return "pinned"
	if sq.suppression > 0.35:
		return "suppressed"
	return "ok"


func _step_squad(sq: Dictionary, dt: float) -> void:
	sq.suppression = maxf(sq.suppression - 0.12 * dt, 0.0)
	var status := _status(sq)
	var enemy := -1
	if sq.type != "mortar" and sq.order != "retreat":
		enemy = _nearest_enemy(sq, RANGE[sq.type])
	# Movement.
	var moving: bool = sq.order in ["move", "attack_move", "retreat", "dig"]
	if sq.order == "attack_move" and enemy != -1:
		moving = false
	if sq.order == "dig" and sq.pos.distance_to(sq.target) < 2.0:
		moving = false
	if moving and status != "pinned":
		var speed: float = SPEED[sq.type] * (1.6 if sq.order == "retreat" else 1.0) * (0.5 if status == "suppressed" else 1.0)
		var to: Vector2 = sq.target - sq.pos
		var step := speed * dt
		if to.length() <= step:
			sq.pos = sq.target
			if sq.order != "dig":
				sq.order = "setup" if sq.type == "mg" else "idle"
				sq.setup_t = 3.0
		else:
			sq.facing = to.normalized()
			sq.pos += to.normalized() * step
	if sq.order == "setup":
		sq.setup_t -= dt
		if sq.setup_t <= 0.0:
			sq.order = "idle"
	# Digging.
	if sq.order == "dig" and not moving and not sq.dig.is_empty():
		var rate := 20.0 / 210.0 * (2.0 if sq.type == "engineer" else 1.0) * DIG_SPEEDUP
		sq.dig.progress = minf(sq.dig.progress + rate * dt / maxf(sq.dig.len, 1.0), 1.0)
		sq.dig.sound_t -= dt
		if sq.dig.sound_t <= 0.0:
			sq.dig.sound_t = 0.6 + rng.randf() * 0.6
			var line: PackedVector3Array = sq.dig.line
			_effects.append({"kind": "dig", "pos": line[rng.randi() % line.size()]})
		if sq.dig.progress >= 1.0:
			stats[sq.owner].trench_m += sq.dig.len
			if sq.owner == local_player:
				_push_event(sq.owner, "info", "%s finished a %d m trench." % [sq.name, roundi(sq.dig.len)], _v3(sq.pos))
			sq.order = "idle"
			sq.dig = {}
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
	# Small arms.
	if enemy != -1 and status != "pinned" and not sq.hp.is_empty():
		sq.fire_t -= dt
		if sq.fire_t <= 0.0:
			var gap: float = FIRE_GAP.get(sq.type, 0.7)
			sq.fire_t = gap * (0.6 + rng.randf() * 0.8) * (6.0 / maxf(sq.hp.size(), 1.0) if sq.type == "rifle" else 1.0)
			if sq.type == "mg" and rng.randf() < 0.08:
				sq.fire_t = 1.5  # burst gap
			_effects.append({"kind": "shot", "weapon": WEAPON[sq.type], "pos": _v3(sq.pos)})
			_shot_at(sq, squads[enemy])


func _shot_at(shooter: Dictionary, target: Dictionary) -> void:
	var d: float = shooter.pos.distance_to(target.pos)
	var p := 0.05 * clampf(1.4 - d / 200.0, 0.2, 1.2)
	if target.order == "idle" and _in_trench(target):
		p *= 0.35
	if _status(target) == "pinned":
		p *= 0.5
	target.suppression = minf(target.suppression + (0.06 if shooter.type == "mg" else 0.025), 1.0)
	if rng.randf() < p:
		_hurt(target, rng.randf_range(35.0, 100.0), shooter.owner)


func _in_trench(sq: Dictionary) -> bool:
	return sq.get("in_trench", false)


func _hurt(sq: Dictionary, dmg: float, by: int) -> void:
	if sq.hp.is_empty():
		return
	var i: int = rng.randi() % sq.hp.size()
	sq.hp[i] -= dmg
	if sq.hp[i] <= 0.0:
		sq.hp.remove_at(i)
		stats[by].kills += 1
		stats[sq.owner].losses += 1
	if sq.hp.is_empty():
		if sq.owner == local_player:
			_push_event(sq.owner, "squad_lost", "%s lost." % sq.name, _v3(sq.pos))
		squads.erase(sq.id)
	elif sq.owner == local_player and time_s - sq.get("alert_t", -99.0) > 30.0:
		sq.alert_t = time_s
		_push_event(sq.owner, "under_attack", "%s under fire." % sq.name, _v3(sq.pos))


func _step_shells(dt: float) -> void:
	var landed: Array[Dictionary] = []
	for s in _shells:
		s.t -= dt
		if s.t <= 0.0:
			landed.append(s)
	for s in landed:
		_shells.erase(s)
		_effects.append({"kind": "explosion", "pos": _v3(s.pos)})
		var ids := squads.keys()
		ids.sort()
		for id in ids:
			if not squads.has(id):
				continue
			var sq: Dictionary = squads[id]
			var d: float = sq.pos.distance_to(s.pos)
			if d < 15.0:
				sq.suppression = minf(sq.suppression + 0.5 * (1.0 - d / 15.0), 1.0)
			if d < 8.0 and sq.owner != s.owner:
				var chance := 0.8 * (1.0 - d / 8.0) * (0.3 if _in_trench(sq) else 1.0)
				if rng.randf() < chance:
					_hurt(sq, 100.0, s.owner)


func _step_capture(dt: float) -> void:
	for p in points:
		var men := [0, 0]
		var pp := Vector2(p.pos.x, p.pos.z)
		for id in squads:
			var sq: Dictionary = squads[id]
			if sq.pos.distance_to(pp) <= p.radius + 6.0:
				men[sq.owner] += sq.hp.size()
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
	var ids := squads.keys()
	ids.sort()
	for id in ids:
		var sq: Dictionary = squads[id]
		if _visible_to(local_player, sq):
			out_squads.append(_squad_state(sq))
	var q: Array = []
	for e in _queues[local_player]:
		q.append(e.duplicate())
	var state := {
		"tick": _tick,
		"time_s": time_s,
		"paused": paused,
		"local_player": local_player,
		"map": {"id": setup.get("map", "kessel_ridge"), "name": "Kessel Ridge", "size_m": MockMap.SIZE_M, "minimap": _minimap},
		"rules": RULES,
		"players": out_players,
		"points": out_points,
		"squads": out_squads,
		"callin": {"catalog": CATALOG, "queue": q, "queue_max": QUEUE_MAX},
		"events": _events,
		"effects": _effects,
		"result": result.duplicate(),
		"stats": stats.duplicate(true),
	}
	_events = []
	_effects = []
	return state


func _squad_state(sq: Dictionary) -> Dictionary:
	var men: int = sq.hp.size()
	var hp_sum := 0.0
	for h in sq.hp:
		hp_sum += h
	var out := {
		"id": sq.id,
		"owner": sq.owner,
		"type": sq.type,
		"name": sq.name,
		"men": men,
		"men_max": sq.men_max,
		"health": hp_sum / (100.0 * sq.men_max),
		"suppression": sq.suppression,
		"status": _status(sq),
		"order": sq.order,
		"pos": _v3(sq.pos),
		"members": _members(sq),
	}
	if not sq.dig.is_empty():
		out.dig_line = sq.dig.line
		out.dig_progress = sq.dig.progress
	if not sq.barrage.is_empty():
		out.target = _v3(sq.barrage.target)
	elif sq.order in ["move", "attack_move", "retreat"]:
		out.target = _v3(sq.target)
	return out


## Member positions: a line across the facing, 2.5 m apart.
func _members(sq: Dictionary) -> PackedVector3Array:
	var out := PackedVector3Array()
	var n: int = sq.hp.size()
	var f: Vector2 = sq.facing
	var side := Vector2(-f.y, f.x)
	for i in n:
		var k := (i - (n - 1) * 0.5) * 2.5
		var jitter := Vector2(sin(sq.id * 3.1 + i * 1.7), cos(sq.id * 1.3 + i * 2.3)) * 0.8
		var p: Vector2 = sq.pos + side * k + f * (absf(k) * -0.15) + jitter
		out.append(_v3(p))
	return out
