extends RefCounted
## The S3 part of `--autoplay` (S3 "Done when" 7 and 8): once vehicles
## unlock it buys tanks (and an AT team) through the call-in intent with
## the catalog `id`, drives them like a player (attack-move to the next
## point, an escort, the ammo toggle, face, reverse, repair, attack on a
## seen enemy vehicle: every S3 intent through MatchView.emit_intent), and
## records what the presenter was given:
##
##   - own vehicles drawn (distinct ids, most at once) and enemy vehicles
##     drawn (seen: in state.vehicles), wrecks, armour events by kind;
##   - s3_autoplay_tank.png (skirmish) or s3_<role>_tank.png (net): our
##     first tank on screen; s3_autoplay_enemy_tank.png: the first enemy
##     vehicle seen (skirmish only).
##
## missing() fails the run (autoplay.gd exit 3) unless at least
## MIN_OWN vehicles of ours and (skirmish) one enemy vehicle were drawn.
## In a net match the requirement is ours spawned (both instances check
## their own side: together, every side).

## Vehicles to buy (net: the S3 net check wants 2 per side).
const WANT_SKIRMISH := 2
const WANT_NET := 3
## Seconds before the unlock from which infantry buys stop, so a tank is
## affordable right at the unlock.
const SAVE_BEFORE_S := 75.0
const ORDER_EVERY_S := 12.0

var ap: Node
var view: MatchView
var camera: Camera3D
var local := 0
var net := false
var want := WANT_SKIRMISH
var bought := 0
var bought_at := {}
var at_team := false
var own_ids := {}
var enemy_ids := {}
var wreck_ids := {}
var own_max := 0
var enemy_max := 0
var events := {}
var drawn_ok := false
var _next_buy := 0.0
var _next_order := {}
var _exercised := {}
var _escort := -1
var _escort_at := 0.0
var _next_log := 0.0


func _init(autoplay: Node) -> void:
	ap = autoplay
	view = ap.view
	camera = ap.camera
	local = ap.local
	net = Session.net_mode
	want = WANT_NET if net else WANT_SKIRMISH


## Whether the S1 script should hold off buying infantry (saving for a
## tank around the unlock).
func saving(s: Dictionary) -> bool:
	if bought >= want:
		return false
	var e := _vehicle_entry(s)
	if e.is_empty():
		return false
	return float(e.get("locked_s", 0.0)) < SAVE_BEFORE_S


func missing() -> Array:
	var out := []
	var need_own := 2 if net else 1
	if own_ids.size() < need_own:
		out.append("s3: own vehicles drawn %d (want %d)" % [own_ids.size(), need_own])
	if not net and enemy_ids.is_empty():
		out.append("s3: no enemy vehicle drawn")
	if not drawn_ok:
		out.append("s3: VehicleView never drew a vehicle")
	return out


func report() -> String:
	return "s3: bought %d, own vehicles drawn %d (max %d at once), enemy vehicles seen %d (max %d), wrecks %d, armour events %s" % [
		bought, own_ids.size(), own_max, enemy_ids.size(), enemy_max, wreck_ids.size(), events]


## Every state; true when it took over this frame (a screenshot).
func step(s: Dictionary, t: float) -> bool:
	_record(s)
	if t >= _next_log:
		_next_log = t + 60.0
		print("autoplay: %s" % report())
	_buy(s, t)
	_drive(s, t)
	return _shots(s)


func _record(s: Dictionary) -> void:
	var own_now := 0
	var enemy_now := 0
	for v in s.get("vehicles", []):
		var o := int(v.owner)
		if o < 0:
			wreck_ids[int(v.id)] = true
		elif o == local:
			own_now += 1
			own_ids[int(v.id)] = true
		else:
			enemy_now += 1
			enemy_ids[int(v.id)] = true
	own_max = maxi(own_max, own_now)
	enemy_max = maxi(enemy_max, enemy_now)
	for e in s.get("armour_events", []):
		var k := String(e.get("kind", ""))
		events[k] = int(events.get(k, 0)) + 1
	var vv: VehicleView = ap.m.vehicle_view
	if vv != null and own_now + enemy_now > 0:
		var n := 0
		for key in vv.counts:
			n += int(vv.counts[key])
		drawn_ok = drawn_ok or n > 0


func _vehicle_entry(s: Dictionary) -> Dictionary:
	for c in s.get("callin", {}).get("catalog", []):
		if String(c.get("type", "")) == "vehicle":
			return c
	return {}


func _buy(s: Dictionary, t: float) -> void:
	if t < _next_buy:
		return
	_next_buy = t + 3.0
	var ci: Dictionary = s.get("callin", {})
	if ci.get("queue", []).size() >= int(ci.get("queue_max", 5)):
		return
	var me: Dictionary = view.player(local)
	var mp := float(me.get("manpower", 0))
	var e := _vehicle_entry(s)
	if bought < want and not e.is_empty() and float(e.get("locked_s", 0.0)) <= 0.0 \
			and int(ci.get("vehicles", 0)) < int(ci.get("vehicle_limit", 4)) and mp >= float(e.get("cost", 1e9)):
		ap._intent(&"call_in", {"type": "vehicle", "id": String(e.id)})
		bought += 1
		bought_at[bought] = t
		_next_buy = t + 8.0
		print("autoplay: s3 call-in %s #%d at %s (manpower %d)" % [e.id, bought, UiPalette.mmss(t), int(mp)])
		return
	# One AT team after the first tank.
	if bought >= 1 and not at_team:
		for c in ci.get("catalog", []):
			if String(c.get("type", "")) == "at" and mp >= float(c.get("cost", 1e9)):
				ap._intent(&"call_in", {"type": "at", "id": String(c.get("id", ""))})
				at_team = true
				print("autoplay: s3 AT team call-in at %s" % UiPalette.mmss(t))
				return


func _own_vehicles(s: Dictionary) -> Array:
	return s.get("vehicles", []).filter(func(v: Dictionary) -> bool: return int(v.owner) == local)


func _enemy_vehicles(s: Dictionary) -> Array:
	return s.get("vehicles", []).filter(func(v: Dictionary) -> bool: return int(v.owner) >= 0 and int(v.owner) != local)


## Orders like a player's: the intents of the S3 contract, each at least
## once, then attack-moves to the next point we do not hold.
func _drive(s: Dictionary, t: float) -> void:
	var own := _own_vehicles(s)
	if own.is_empty():
		return
	var foes := _enemy_vehicles(s)
	for v in own:
		var id := int(v.id)
		if t < float(_next_order.get(id, 0.0)):
			continue
		_next_order[id] = t + ORDER_EVERY_S
		var ids := PackedInt32Array([id])
		# A seen enemy vehicle within 500 m: attack it.
		var near: Variant = null
		var best := 500.0
		for f in foes:
			var d: float = v.pos.distance_to(f.pos)
			if d < best:
				best = d
				near = f
		if near != null:
			_once("attack_unit", &"attack_unit", {"units": ids, "target": int(near.id)}, true)
			continue
		var comps: Dictionary = v.get("components", {})
		var damaged := false
		for k in comps:
			if k != "ammo_rack" and float(comps[k]) < 0.6:
				damaged = true
		if damaged and String(v.get("repairing", "")) == "":
			ap._intent(&"repair_units", {"units": ids})
			_exercised["repair_units"] = true
			print("autoplay: s3 repair_units for #%d" % id)
			continue
		# The one-off intents on the first tank, in order.
		if not _exercised.has("set_ammo_he"):
			_exercised["set_ammo_he"] = true
			ap._intent(&"set_ammo", {"units": ids, "ammo": "he"})
			continue
		if not _exercised.has("face_units"):
			_exercised["face_units"] = true
			var ahead: Vector3 = ap._ground(v.pos + (ap._point(s, "C").pos - v.pos).normalized() * 30.0)
			ap._intent(&"face_units", {"units": ids, "target": ahead}, ahead)
			continue
		if not _exercised.has("reverse_units"):
			_exercised["reverse_units"] = true
			var back: Vector3 = ap._ground(v.pos + (ap._base() - v.pos).normalized() * 15.0)
			ap._intent(&"reverse_units", {"units": ids, "target": back}, back)
			continue
		if not _exercised.has("set_ammo_auto"):
			_exercised["set_ammo_auto"] = true
			ap._intent(&"set_ammo", {"units": ids, "ammo": "auto"})
			continue
		if not _exercised.has("follow_units"):
			var rifle := _escort_squad(s)
			if not rifle.is_empty():
				_exercised["follow_units"] = true
				_escort = int(rifle.id)
				_escort_at = t
				ap.s1b.reserved[_escort] = true
				ap._intent(&"follow_units", {"units": view.squad_units.get(_escort, PackedInt32Array()), "vehicle": id})
				print("autoplay: s3 follow_units: %s#%d escorts #%d" % [rifle.type, _escort, id])
		# Move on: the next point we do not hold (the escort follows).
		if String(v.get("state", "")) in ["idle", "moving"] or t - float(bought_at.get(1, 0.0)) > 60.0:
			var p: Variant = ap._next_point(s, v.pos)
			if p != null:
				ap._intent(&"attack_move_units", {"units": ids, "target": p}, p)
	# Release the escort after two minutes.
	if _escort >= 0 and t - _escort_at > 120.0:
		ap.s1b.reserved.erase(_escort)
		_escort = -1


func _once(key: String, kind: StringName, data: Dictionary, log := false) -> void:
	ap._intent(kind, data)
	if log and not _exercised.has(key):
		print("autoplay: s3 %s %s" % [kind, data])
	_exercised[key] = true


func _escort_squad(s: Dictionary) -> Dictionary:
	for sq in ap._own(s):
		if sq.type == "rifle" and not ap.s1b.reserved.has(int(sq.id)) and sq.order in ["idle", "attack_move", "move"]:
			return sq
	return {}


func _shots(s: Dictionary) -> bool:
	# Not while the S1b tour moves the camera and the selection.
	if ap._busy or not ap.s1b.done_selection:
		return false
	var prefix := "s3_%s_" % (Session.autoplay_net_role if net else "autoplay")
	if not ap.shots.has(prefix + "tank"):
		var own := _own_vehicles(s)
		if not own.is_empty():
			var v: Dictionary = own[0]
			camera.height = 30.0
			camera.pitch_deg = -40.0
			camera.jump_to(v.pos)
			_snap(prefix + "tank")
			return true
	if not net and not ap.shots.has(prefix + "enemy_tank"):
		var foes := _enemy_vehicles(s)
		if not foes.is_empty():
			camera.height = 45.0
			camera.pitch_deg = -42.0
			camera.jump_to(foes[0].pos)
			_snap(prefix + "enemy_tank")
			return true
	return false


func _snap(name: String) -> void:
	ap._busy = true
	ap.shots[name] = true
	await ap.get_tree().create_timer(0.5, true, false, true).timeout
	await Session.save_screenshot(ap.dir.path_join(name + ".png"))
	ap._busy = false
