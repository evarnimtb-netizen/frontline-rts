extends Node
## Scripted local player for `--autoplay` (S1 "Done when" 4): plays the
## human side against the sim's AI for Session.autoplay_minutes of game
## time and saves screenshots to Session.autoplay_dir():
##
##   s1_autoplay_menu.png      (taken by Session before the match)
##   s1_autoplay_overview.png  own base and the first moves
##   s1_autoplay_trench.png    engineers digging a trench in front of B
##   s1_autoplay_explosion.png a mortar shell landing
##   s1_autoplay_battle.png    small-arms fire near our men (or the front,
##                             late in the run)
##   s1_autoplay_end.png       the end screen (the script surrenders at the
##                             time limit if nobody has won)
##
## It plays through the same intents a human sends (MatchView.emit_intent),
## so it exercises the whole presenter → command path. Decisions use only
## what the state shows the local player (own squads, spotted enemies).

const SHOT_PREFIX := "s1_autoplay_"
## Wall-clock seconds to wait after the time limit before giving up.
const GRACE_S := 90.0

var m: Node3D
var view: MatchView
var source: Node
var camera: Camera3D
var minutes := 5.0
var dir := ""
var local := 0
var info: Dictionary = {}
var shots := {}
var dig_site := {}
var dig_ordered := false
var barrage_target: Variant = null
var barrage_at := -1.0
var explosion_wait := false
var mortar_moved := false
var ended_at := -1.0
var surrendered := false
var wall := 0.0
var _busy := false
var _next_buy := 0.0
var _next_log := 0.0
var _orders := {}
var _order_from := {}
var _size := Vector2i(1600, 900)


func begin(match_root: Node3D) -> void:
	m = match_root
	view = m.view
	source = m.source
	camera = m.camera
	minutes = Session.autoplay_minutes
	dir = Session.autoplay_dir()
	info = source.get_map_info()
	local = int(info.get("local_player", 0))
	source.set_speed(Session.autoplay_speed)
	camera.input_enabled = false
	_size = get_window().size
	print("autoplay: %.1f game minutes at %.1fx, screenshots in %s" % [minutes, Session.autoplay_speed, dir])


func on_state(s: Dictionary) -> void:
	wall += get_process_delta_time()
	var t: float = s.get("time_s", 0.0)
	if t >= _next_log:
		_next_log = t + 30.0
		_log(s)
	var result: Dictionary = s.get("result", {})
	if result.get("status", "running") != "running":
		_finish(s)
		return
	if _busy:
		return
	_play(s, t)
	if t >= minutes * 60.0 or wall > minutes * 60.0 / maxf(Session.autoplay_speed, 0.25) + GRACE_S:
		if not shots.has("battle"):
			_snap_front(s, "battle")
			return
		if not surrendered:
			surrendered = true
			print("autoplay: time limit reached at %s, surrendering" % UiPalette.mmss(t))
			view.emit_intent(&"surrender", {})


# --- playing -----------------------------------------------------------------

func _play(s: Dictionary, t: float) -> void:
	var own := _own(s)
	if t < 1.0 and not _orders.has("opening"):
		_orders.opening = true
		_opening(s, own)
		return
	if not shots.has("overview") and t >= 10.0:
		var c := Vector3.ZERO
		var own_now := _own(s)
		for sq in own_now:
			c += sq.pos / maxf(own_now.size(), 1)
		var ahead: Vector3 = (_point(s, "B").pos - c).normalized()
		_snap_at(c + ahead * 35.0, 60.0, -42.0, "overview")
		return
	# Engineers dig in front of B once they get there.
	if not dig_ordered and not dig_site.is_empty():
		for sq in own:
			if sq.type == "engineer" and sq.pos.distance_to(dig_site.center) < 14.0 and sq.order in ["idle", "move"]:
				dig_ordered = true
				view.selection.select_squads(PackedInt32Array([sq.id]))
				_intent(&"dig", {"squads": PackedInt32Array([sq.id]), "polyline": dig_site.line}, dig_site.center)
				print("autoplay: engineers digging at %s" % [dig_site.center])
	if dig_ordered and not shots.has("trench"):
		for sq in own:
			if sq.type == "engineer" and sq.has("dig_progress") and float(sq.dig_progress) >= 0.45:
				var c: Vector3 = dig_site.center
				var away: Vector3 = (c - _point(s, "C").pos).normalized()
				_snap_at(c + away * 4.0, 24.0, -42.0, "trench", rad_to_deg(atan2(away.x, away.z)))
				return
	# Mortars: walk behind B, then fire on what we can see.
	for sq in own:
		if sq.type != "mortar":
			continue
		if not mortar_moved:
			mortar_moved = true
			var b: Vector3 = _point(s, "B").pos
			var back: Vector3 = (_base() - b).normalized()
			_intent(&"move", {"squads": PackedInt32Array([sq.id]), "target": _ground(b + back * 30.0)}, b)
		elif barrage_target == null and sq.order == "idle":
			var tgt: Variant = _barrage_target(s, sq.pos, t)
			if tgt != null:
				barrage_target = tgt
				barrage_at = t
				view.selection.select_squads(PackedInt32Array([sq.id]))
				_intent(&"barrage", {"squads": PackedInt32Array([sq.id]), "target": tgt}, tgt)
				print("autoplay: barrage on %s at %s" % [tgt, UiPalette.mmss(t)])
				camera.height = 75.0
				camera.pitch_deg = -55.0
				camera.jump_to(tgt)
				explosion_wait = true
	if explosion_wait and not shots.has("explosion"):
		for fx in s.get("effects", []):
			if fx.kind == "explosion" and fx.pos.distance_to(barrage_target) < 90.0:
				explosion_wait = false
				camera.jump_to(fx.pos)
				_snap_delayed(0.3, "explosion")
				return
		if t - barrage_at > 60.0:
			explosion_wait = false
			print("autoplay: no explosion seen near the barrage target")
	# Reinforce: buy rifles (and an MG) when there is manpower.
	var me: Dictionary = view.player(local)
	if t >= _next_buy and float(me.get("manpower", 0)) >= 250.0 and s.callin.queue.size() < 2:
		_next_buy = t + 20.0
		var kind := "mg" if int(t / 20.0) % 3 == 1 else "rifle"
		_intent(&"call_in", {"type": kind})
	# Idle riflemen and MGs push to the nearest point we do not hold; so do
	# attackers that have not got anywhere for two minutes (re-path).
	for sq in own:
		if not sq.type in ["rifle", "mg"]:
			continue
		var since: float = t - float(_orders.get(sq.id, -999.0))
		var stalled: bool = sq.order == "attack_move" and since > 120.0 \
			and sq.pos.distance_to(_order_from.get(sq.id, Vector3.INF)) < 15.0
		if (sq.order == "idle" and since > 40.0) or stalled:
			var p: Variant = _next_point(s, sq.pos)
			if p != null:
				_orders[sq.id] = t
				_order_from[sq.id] = sq.pos
				_intent(&"attack_move", {"squads": PackedInt32Array([sq.id]), "target": p}, p)
		elif sq.order == "attack_move" and since > 120.0:
			# Still moving: measure the next two minutes from here.
			_orders[sq.id] = t - 1.0
			_order_from[sq.id] = sq.pos
	# The battle shot waits for small-arms fire near our men (or, late in the
	# run, settles for the front).
	if not shots.has("battle") and t > maxf(150.0, minutes * 36.0):
		var fire: Variant = _firefight(s)
		if fire != null:
			_snap_at(fire, 55.0, -45.0, "battle")
		elif t > minutes * 54.0 and _fight_near(s) != null:
			_snap_front(s, "battle")


func _opening(s: Dictionary, own: Array) -> void:
	_intent(&"call_in", {"type": "mortar"})
	_intent(&"call_in", {"type": "mg"})
	var b: Vector3 = _point(s, "B").pos
	var c: Vector3 = _point(s, "C").pos
	var to_c := (c - b).normalized()
	var side := Vector3(-to_c.z, 0, to_c.x)
	var center := _ground(b + to_c * 16.0)
	var line := PackedVector3Array()
	for k in [-1.0, -0.35, 0.35, 1.0]:
		var bend := -to_c * (1.5 if absf(k) < 0.5 else 0.0)
		line.append(_ground(center + side * 9.0 * k + bend))
	dig_site = {"center": center, "line": line}
	var rifles := []
	for sq in own:
		match sq.type:
			"engineer":
				_intent(&"move", {"squads": PackedInt32Array([sq.id]), "target": _ground(center - to_c * 3.0)}, center)
			"rifle":
				rifles.append(sq.id)
	if rifles.size() > 0:
		_intent(&"attack_move", {"squads": PackedInt32Array([rifles[0]]), "target": _ground(b + to_c * 25.0)}, b)
	if rifles.size() > 1:
		var a: Vector3 = _point(s, "A").pos
		_intent(&"attack_move", {"squads": PackedInt32Array([rifles[1]]), "target": a}, a)
	print("autoplay: opening orders sent (%d squads)" % own.size())


func _barrage_target(s: Dictionary, from: Vector3, t: float) -> Variant:
	var rules := view.rules()
	var lo := float(rules.get("mortar_min_m", 60.0)) + 15.0
	var hi := float(rules.get("mortar_max_m", 600.0)) - 20.0
	var best: Variant = null
	var best_d := INF
	for sq in s.get("squads", []):
		if int(sq.owner) == local:
			continue
		var d: float = from.distance_to(sq.pos)
		if d > lo and d < hi and d < best_d and not _near_own(s, sq.pos, 45.0):
			best_d = d
			best = sq.pos
	if best != null:
		return best
	if t < 170.0:
		return null
	# Nothing spotted: shell the nearest point we do not hold.
	for p in _points_by_distance(s, from):
		var d: float = from.distance_to(p.pos)
		if int(p.owner) != local and d > lo and d < hi and not _near_own(s, p.pos, 45.0):
			return p.pos
	return null


# --- screenshots ---------------------------------------------------------------

func _snap_at(pos: Vector3, height: float, pitch: float, name: String, yaw: float = INF) -> void:
	camera.height = height
	camera.pitch_deg = pitch
	if yaw != INF:
		camera.yaw_deg = yaw
	camera.jump_to(pos)
	_snap_delayed(0.6, name)


func _snap_front(s: Dictionary, name: String) -> void:
	var at: Variant = _fight_near(s)
	if at == null:
		at = _mid(_point(s, "B").pos, _point(s, "C").pos, 0.6)
	_snap_at(at, 70.0, -48.0, name)


func _snap_delayed(delay: float, name: String) -> void:
	_busy = true
	shots[name] = true
	# Keep shots at the size the run asked for, even if the OS resized the
	# window meanwhile.
	var win := get_window()
	if win.mode == Window.MODE_WINDOWED and win.size != _size:
		win.size = _size
	await get_tree().create_timer(delay, true, false, true).timeout
	await Session.save_screenshot(dir.path_join(SHOT_PREFIX + name + ".png"))
	_busy = false


func _finish(s: Dictionary) -> void:
	if ended_at < 0.0:
		ended_at = wall
		var r: Dictionary = s.result
		print("autoplay: match over at %s: %s (winner %d, %s)" % [UiPalette.mmss(s.time_s), r.status, r.winner, r.reason])
		_log(s)
		return
	if _busy or shots.has("end") or wall - ended_at < 2.0:
		return
	_busy = true
	shots.end = true
	await Session.save_screenshot(dir.path_join(SHOT_PREFIX + "end.png"))
	var missing := []
	for k in ["overview", "trench", "explosion", "battle", "end"]:
		if not shots.has(k):
			missing.append(k)
	print("autoplay: done; screenshots %s%s" % [shots.keys(), "" if missing.is_empty() else ", missing %s" % [missing]])
	Session.quit_game(0 if missing.is_empty() else 3)


# --- helpers -------------------------------------------------------------------

func _intent(kind: StringName, data: Dictionary, marker_at: Variant = null) -> void:
	view.emit_intent(kind, data)
	if marker_at != null and view.overlay != null and view.overlay.has_method("marker"):
		view.overlay.marker(marker_at, kind)


func _own(s: Dictionary) -> Array:
	return s.get("squads", []).filter(func(sq: Dictionary) -> bool: return int(sq.owner) == local)


func _point(s: Dictionary, id: String) -> Dictionary:
	for p in s.get("points", []):
		if p.id == id:
			return p
	return {"pos": Vector3.ZERO, "owner": -1, "id": id}


func _points_by_distance(s: Dictionary, from: Vector3) -> Array:
	var pts: Array = s.get("points", []).duplicate()
	pts.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return from.distance_to(a.pos) < from.distance_to(b.pos))
	return pts


func _next_point(s: Dictionary, from: Vector3) -> Variant:
	for p in _points_by_distance(s, from):
		if int(p.owner) != local:
			return p.pos
	return null


func _near_own(s: Dictionary, pos: Vector3, r: float) -> bool:
	for sq in _own(s):
		if sq.pos.distance_to(pos) < r:
			return true
	return false


## Centre of this frame's shot cues (one per firing squad) within 150 m of
## our squads, if at least two squads fire (a firefight in view).
func _firefight(s: Dictionary) -> Variant:
	var sum := Vector3.ZERO
	var n := 0
	for fx in s.get("effects", []):
		if fx.kind == "shot" and _near_own(s, fx.pos, 150.0):
			sum += fx.pos
			n += 1
	return sum / n if n >= 2 else null


## Midpoint of the closest own/enemy pair under 220 m, if any.
func _fight_near(s: Dictionary) -> Variant:
	var best: Variant = null
	var best_d := 220.0
	var foes: Array = s.get("squads", []).filter(func(sq: Dictionary) -> bool: return int(sq.owner) != local)
	for a in _own(s):
		for b in foes:
			var d: float = a.pos.distance_to(b.pos)
			if d < best_d:
				best_d = d
				best = a.pos.lerp(b.pos, 0.5)
	return best


func _base() -> Vector3:
	var bases: Array = info.get("bases", [])
	var own := int(info.get("own_base", 0))
	return bases[own].pos if own < bases.size() else Vector3.ZERO


func _mid(a: Vector3, b: Vector3, k: float) -> Vector3:
	return a.lerp(b, k)


func _ground(p: Vector3) -> Vector3:
	return Vector3(p.x, source.height_at(p.x, p.z), p.z)


func _log(s: Dictionary) -> void:
	var st: Dictionary = source.get_stats()
	var me: Dictionary = view.player(local)
	var them: Dictionary = view.player(1 - local)
	print("autoplay: t=%s tickets %d/%d manpower %d squads %d soldiers %d rounds %d tiles %d step %.2f ms (max %.2f at tick %d) late %d" % [
		UiPalette.mmss(s.get("time_s", 0.0)), int(me.get("tickets", 0)), int(them.get("tickets", 0)),
		int(me.get("manpower", 0)), s.get("squads", []).size(), int(st.get("soldiers", 0)),
		int(st.get("rounds", 0)), int(st.get("detail_tiles", 0)), float(st.get("sim_step_ms_last", 0.0)),
		float(st.get("sim_step_ms_max", 0.0)), int(st.get("sim_step_max_tick", 0)), int(st.get("sim_late_ticks", 0))])
	var parts := []
	for sq in _own(s):
		parts.append("%s#%d %s %d/%d (%d,%d)" % [sq.type, sq.id, sq.order, sq.men, sq.men_max, roundi(sq.pos.x), roundi(sq.pos.z)])
	print("autoplay:   own: %s" % ", ".join(parts))
