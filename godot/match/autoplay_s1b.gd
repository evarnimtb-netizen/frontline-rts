extends RefCounted
## The S1b part of `--autoplay` (S1b-soldier-control.md "Done when" 3,
## S1b-abilities.md "Done when" 4): soldier selection and unit orders
## driven through the real input path, with screenshots saved as
## docs/slices/screens/s1b_*.png:
##
##   s1b_click_one.png        one man clicked: his ring, his detail row
##   s1b_double_click.png     a double-click on him: his whole squad
##   s1b_box_two_squads.png   a box across two squads (stored as group 1)
##   s1b_one_man_forward.png  one man right-clicked forward on his own
##                            (detached: target ring and dashed line)
##   s1b_prone.png            the same man after Z: lying prone (close up)
##   s1b_group_recall.png     group 1 recalled with 1, 1 (camera centres)
##   s1b_smoke_tool.png       T armed on a squad: range ring and target
##   s1b_throw_flight.png     the smoke grenade in the air (its trail)
##   s1b_smoke.png            the smoke cloud standing
##   s1b_frag_blast.png       the lone man's practice frag (G, click) going
##                            off on open ground: the frag-sized blast
##   s1b_hud_1600.png         the full HUD at 1600x900
##   s1b_hud_1280.png         the full HUD at 1280x800
##   s1b_frag_tool.png        G armed on a man within 30 m of an enemy in
##                            a trench
##   s1b_frag_trench.png      the frag bursting in the enemy trench
##   s1b_frag_enemy.png       (only if the trench throw has not happened by
##                            9:00) a frag at any spotted enemy in range
##
## Mouse and keys are InputEvents pushed into the viewport (GUI first, then
## the selection controller's _unhandled_input), at the screen positions
## MatchView projects for picking, so these steps exercise the same code
## as a player's hand. Only the camera is moved by script.
##
## The frag step needs a spotted enemy soldier standing in a trench: the
## script waits for one, sends the nearest own squad with frags at him
## (a plain move, so it does not stop to trade fire at range), and throws
## when one of its unpinned men is within FRAG_RANGE_M. It retries with
## another target or squad, and reports when it never got there.

const SHOTS := [
	"click_one", "double_click", "box_two_squads", "one_man_forward", "prone",
	"group_recall", "smoke_tool", "throw_flight", "smoke", "frag_blast", "hud_1600", "hud_1280", "frag_tool",
	"frag_trench",
]
const PREFIX := "s1b_"
## Throw from a bit inside the 30 m frag range (the target moves).
const FRAG_RANGE_M := 26.0
## Game seconds an assault squad gets to close in before the script tries
## another target: this plus the walk at ASSAULT_SPEED.
const ASSAULT_S := 120.0
const ASSAULT_SPEED := 0.8
## Only trenches this close to one of our squads are assaulted (m).
const ASSAULT_REACH_M := 320.0
## Game seconds to wait for the frag burst after the throw.
const BURST_WAIT_S := 12.0
## Game time after which any spotted enemy in range is a frag target too.
const OPEN_FRAG_S := 540.0
## Dug at least this deep (m), and long (see _elongated), counts as a
## trench. The AI's trenches are often left half dug (about 0.8 m), so
## this does not wait for full depth (1.6 m).
const TRENCH_DEPTH_M := 0.5

var ap: Node
var view: MatchView
var camera: Camera3D
var local := 0
## Squads the S1 script must not re-order (the assault squad).
var reserved := {}
var done_selection := false
var _lone := -1
var _assault := {}
var _next_scan := 0.0
var _next_scan_log := 0.0
var _next_try := 0.0
## Known enemy trench spots ({pos, foe}), nearest our squads first.
var _cells: Array = []
var _tried := {}


func _init(autoplay: Node) -> void:
	ap = autoplay
	view = ap.view
	camera = ap.camera
	local = ap.local


func missing() -> Array:
	var out := []
	for k in SHOTS:
		if not ap.shots.has(PREFIX + k):
			out.append(PREFIX + k)
	return out


## Called by autoplay.gd each state while it is not busy. Returns true when
## it started something (autoplay then waits).
func step(s: Dictionary, t: float) -> bool:
	if not done_selection and t >= 14.0:
		done_selection = true
		_selection_tour(s)
		return true
	if done_selection and not ap.shots.has(PREFIX + "hud_1600") and t >= 160.0:
		# Where an own squad and a spotted enemy squad are within 140 m,
		# else whatever the front is by 6:40.
		var pair := _contact(s, 120.0 if t < 400.0 else 2000.0)
		if not pair.is_empty():
			_hud_shots(pair)
			return true
	if done_selection and not ap.shots.has(PREFIX + "frag_trench") and t >= 60.0:
		return _frag_assault(s, t)
	return false


# --- selection, unit orders, stance, groups, smoke, HUD -----------------------

func _selection_tour(s: Dictionary) -> void:
	ap._busy = true
	var pair := _two_close_squads(s)
	if pair.is_empty():
		print("autoplay s1b: no two own squads with men to select")
		ap._busy = false
		return
	var a: Dictionary = pair[0]
	var b: Dictionary = pair[1]
	await _look_at(a.pos.lerp(b.pos, 0.5), 38.0, -52.0)
	var men_a: PackedInt32Array = view.squad_units.get(int(a.id), PackedInt32Array())
	var man := men_a[mini(2, men_a.size() - 1)]
	# Click one man.
	await _click_soldier(man, false)
	await _snap("click_one", 0.35)
	# Double-click him: his squad.
	await _click_soldier(man, true)
	await _snap("double_click", 0.35)
	# Box across the two squads: the halves of each that face the other,
	# then Ctrl+1 stores the exact set.
	var box := _box_between(int(a.id), int(b.id))
	if box.size == Vector2.ZERO:
		print("autoplay s1b: the two squads are not both on screen for the box")
	else:
		await _drag(box.position, box.end)
		await _key(KEY_1, true)
		print("autoplay s1b: box selected %d men in squads %s" % [view.selection.selected.size(), view.selection.selected_squads()])
	await _snap("box_two_squads", 0.35)

	# One man out on his own, 30 m off to the side of his squad's march,
	# then prone.
	_lone = men_a[mini(4, men_a.size() - 1)]
	var lp: Vector3 = view.soldiers_by_id[_lone].pos
	var march: Vector3 = ap._point(s, "B").pos - lp
	march.y = 0.0
	var side := Vector3(-march.z, 0.0, march.x).normalized()
	var ahead: Vector3 = ap._ground(lp + side * 30.0 + march.normalized() * 10.0)
	await _look_at(lp.lerp(ahead, 0.5), 44.0, -55.0)
	await _click_soldier(_lone, false)
	await _right_click_ground(ahead)
	await _wait_game(8.0)
	var now: Vector3 = view.soldiers_by_id.get(_lone, {"pos": lp}).pos
	var sq_now: Dictionary = view.squads_by_id.get(int(a.id), a)
	await _look_at(now.lerp(sq_now.pos, 0.4), 46.0, -55.0)
	var so: Dictionary = view.soldiers_by_id.get(_lone, {})
	print("autoplay s1b: lone man %d detached=%s order=%s" % [_lone, so.get("detached", false), so.get("order", "")])
	await _snap("one_man_forward", 0.3)
	await _key(KEY_Z)
	for i in 30:
		if view.soldiers_by_id.get(_lone, {}).get("stance", "") == "prone":
			break
		await _wait_real(0.1)
	so = view.soldiers_by_id.get(_lone, {})
	print("autoplay s1b: lone man stance=%s override=%s" % [so.get("stance", "?"), so.get("stance_override", "?")])
	if not so.is_empty():
		await _look_at(so.pos, 9.0, -28.0)
	await _snap("prone", 0.4)

	# Recall group 1 twice quickly: selected again, camera centred on it.
	await _look_at(a.pos, 60.0, -55.0)
	await _key(KEY_1)
	await _wait_real(0.08)
	await _key(KEY_1)
	print("autoplay s1b: group 1 recalled, %d men" % view.selection.selected.size())
	await _snap("group_recall", 0.4)

	# Smoke from squad A (its carrier throws), 18 m to its side:
	# double-click a man to select the squad, T, click the spot.
	var carrier := -1
	for id in view.squad_units.get(int(a.id), PackedInt32Array()):
		if view.item_count(view.soldiers_by_id[id], "smoke") > 0:
			carrier = id
			break
	if carrier == -1:
		print("autoplay s1b: nobody in squad %d carries smoke" % int(a.id))
		ap._busy = false
		return
	var lead_pos: Vector3 = view.soldiers_by_id[carrier].pos
	# Off to the side of the march (toward C), so the squad does not walk
	# into the minimum range while the tool is up.
	var to_c: Vector3 = ap._point(s, "C").pos - lead_pos
	to_c.y = 0.0
	var aside := Vector3(to_c.z, 0.0, -to_c.x).normalized()
	var spot: Vector3 = ap._ground(lead_pos + aside * 18.0)
	await _look_at(lead_pos.lerp(spot, 0.5), 42.0, -55.0)
	var pick_man := _central_man(int(a.id))
	if pick_man >= 0:
		await _click_soldier(pick_man, true)
	if not view.selection.is_selected(carrier):
		print("autoplay s1b: the double-click did not select the smoke carrier; selecting squad %d directly" % int(a.id))
		view.selection.select_squads(PackedInt32Array([int(a.id)]))
	await _key(KEY_T)
	if view.selection.tool != &"throw":
		print("autoplay s1b: T did not arm the smoke tool")
	view.selection.cursor_override = camera.unproject_position(spot)
	await _snap("smoke_tool", 0.3)
	view.selection.cursor_override = null
	var sent0 := int(ap.source.get_stats().get("commands_sent", 0))
	var in_range: bool = view.selection.throw_in_range("smoke", spot)
	await _click_screen(camera.unproject_position(spot), false)
	print("autoplay s1b: smoke thrown at %s (in range for the view: %s, %d m from the carrier), tool now '%s', commands sent %d" % [
		spot, in_range, roundi(view.soldiers_by_id.get(carrier, {"pos": spot}).pos.distance_to(spot)),
		view.selection.tool, int(ap.source.get_stats().get("commands_sent", 0)) - sent0])
	# The grenade in the air (trail), then the cloud.
	var t0: float = view.state.get("time_s", 0.0)
	var flying := -1.0
	var seen := 0
	while float(view.state.get("time_s", 0.0)) - t0 < 6.0:
		await ap.get_tree().process_frame
		var tnow: float = view.state.get("time_s", 0.0)
		for th in view.state.get("thrown", []):
			seen += 1
			if th.kind == "smoke" and not th.landed and flying < 0.0:
				flying = tnow
		# Snap half a second into the flight, near the top of the lob.
		if flying >= 0.0 and tnow - flying >= 0.5:
			break
	if flying >= 0.0:
		await _snap("throw_flight", 0.0)
		for th in view.state.get("thrown", []):
			print("autoplay s1b:   in flight: %s at %s (%.1f m above the ground, %d m from the camera), landed %s; trails drawn %d" % [
				th.kind, th.pos, th.pos.y - ap.source.height_at(th.pos.x, th.pos.z), roundi(camera.global_position.distance_to(th.pos)),
				th.landed, int(ap.source.get_stats().get("thrown_trails_drawn", -1))])
	else:
		print("autoplay s1b: did not see the smoke grenade in flight (%d thrown entries seen)" % seen)
	var cloud: Variant = await _wait_cloud(spot, 14.0)
	if cloud == null:
		print("autoplay s1b: no smoke cloud near %s" % [spot])
	else:
		await _look_at(cloud.pos, 40.0, -38.0)
	await _snap("smoke", 0.3)

	# A practice frag from the lone man, onto open ground away from
	# everyone: the whole G path and the frag-sized blast, even if the run
	# never gets a man within 30 m of an enemy trench.
	if view.soldiers_by_id.has(_lone) and view.item_count(view.soldiers_by_id[_lone], "frag") > 0:
		var lone_pos: Vector3 = view.soldiers_by_id[_lone].pos
		var away: Vector3 = lone_pos - view.squads_by_id.get(int(a.id), a).pos
		away.y = 0.0
		var blast_at: Vector3 = ap._ground(lone_pos + (away.normalized() if away.length() > 1.0 else Vector3(1, 0, 0)) * 18.0)
		await _look_at(lone_pos.lerp(blast_at, 0.5), 34.0, -50.0)
		await _click_soldier(_lone, false)
		await _key(KEY_G)
		await _click_screen(camera.unproject_position(blast_at), false)
		var tb: float = view.state.get("time_s", 0.0)
		var burst_seen := false
		while not burst_seen and float(view.state.get("time_s", 0.0)) - tb < BURST_WAIT_S:
			await ap.get_tree().process_frame
			for fx in view.state.get("effects", []):
				if fx.kind == "explosion" and fx.get("size", "") == "frag" and fx.pos.distance_to(blast_at) < 10.0:
					burst_seen = true
		if burst_seen:
			await _snap("frag_blast", 0.12)
		else:
			print("autoplay s1b: the practice frag did not go off near %s" % [blast_at])
	view.selection.select_only(PackedInt32Array())
	ap._busy = false


## The closest own / spotted enemy squad pair within `max_d`: [own, foe].
func _contact(s: Dictionary, max_d: float) -> Array:
	var best := []
	var best_d := max_d
	for sq in ap._own(s):
		if sq.type == "mortar":
			continue
		for foe in s.get("squads", []):
			if int(foe.owner) == local:
				continue
			var d: float = sq.pos.distance_to(foe.pos)
			if d < best_d:
				best_d = d
				best = [sq, foe]
	return best


## The full HUD in contact: the own squad of the pair selected with a
## double-click on a man, both squads in view, at 1600x900, then 1280x800.
func _hud_shots(pair: Array) -> void:
	ap._busy = true
	var best: Dictionary = pair[0]
	var foe: Dictionary = pair[1]
	var gap: float = best.pos.distance_to(foe.pos)
	# Both squads in the clear band above the console: centre a little
	# toward the enemy, steep enough that the near squad stays up.
	await _look_at(best.pos.lerp(foe.pos, 0.55), clampf(gap * 0.6, 50.0, 95.0), -62.0)
	var man := _central_man(int(best.id))
	if man < 0:
		# Our squad fell outside the picture: frame it alone.
		await _look_at(best.pos, 60.0, -55.0)
		man = _central_man(int(best.id))
	if man >= 0:
		await _click_soldier(man, true)
	if not view.selection.selected_squads().has(int(best.id)):
		view.selection.select_squads(PackedInt32Array([int(best.id)]))
	await _snap("hud_1600", 0.4)
	var win := ap.get_window()
	if win.mode == Window.MODE_WINDOWED and DisplayServer.get_name() != "headless":
		win.size = Vector2i(1280, 800)
		await _wait_real(0.8)
		await _snap("hud_1280", 0.1, Vector2i(1280, 800))
		win.size = ap._size
		await _wait_real(0.5)
	else:
		print("autoplay s1b: window not resizable here, no 1280x800 shot")
	view.selection.select_only(PackedInt32Array())
	ap._busy = false


## Two own squads (rifle or engineer first) whose leaders stand closest.
func _two_close_squads(s: Dictionary) -> Array:
	var own: Array = ap._own(s).filter(func(sq: Dictionary) -> bool: return int(sq.men) >= 3)
	own.sort_custom(func(x: Dictionary, y: Dictionary) -> bool: return int(x.id) < int(y.id))
	var best := []
	var best_d := INF
	for i in own.size():
		for j in range(i + 1, own.size()):
			var x: Dictionary = own[i]
			var y: Dictionary = own[j]
			var d: float = x.pos.distance_to(y.pos)
			if not x.type in ["rifle", "engineer"]:
				d += 500.0
			if d < best_d:
				best_d = d
				best = [x, y]
	return best


## Screen box around the half of each squad nearer the other.
func _box_between(sa: int, sb: int) -> Rect2:
	var pa := _screens(sa)
	var pb := _screens(sb)
	if pa.is_empty() or pb.is_empty():
		return Rect2()
	var ca := _mean(pa)
	var cb := _mean(pb)
	var pick := PackedVector2Array()
	for side in [[pa, cb], [pb, ca]]:
		var pts: Array = Array(side[0])
		var c: Vector2 = side[1]
		pts.sort_custom(func(p: Vector2, q: Vector2) -> bool: return p.distance_to(c) < q.distance_to(c))
		for k in maxi(pts.size() / 2, 1):
			pick.append(pts[k])
	var r := Rect2(pick[0], Vector2.ZERO)
	for p in pick:
		r = r.expand(p)
	return r.grow(5.0)


## The squad's man drawn nearest the screen centre, inside the clear band
## between the minimap and the console (a click there reaches the world),
## or -1.
func _central_man(squad_id: int) -> int:
	var size := view.get_viewport().get_visible_rect().size
	var mid := size * 0.5
	var clear := Rect2(Vector2(280, 40), size - Vector2(280 + 150, 40 + 180))
	var best := -1
	var best_d := INF
	for id in view.squad_units.get(squad_id, PackedInt32Array()):
		var i := view.pick_ids.find(id)
		if i >= 0 and clear.has_point(view.pick_screen[i]) and view.pick_screen[i].distance_to(mid) < best_d:
			best_d = view.pick_screen[i].distance_to(mid)
			best = id
	return best


func _screens(squad_id: int) -> PackedVector2Array:
	var out := PackedVector2Array()
	for id in view.squad_units.get(squad_id, PackedInt32Array()):
		var i := view.pick_ids.find(id)
		if i >= 0:
			out.append(view.pick_screen[i])
	return out


func _mean(pts: PackedVector2Array) -> Vector2:
	var c := Vector2.ZERO
	for p in pts:
		c += p
	return c / maxf(pts.size(), 1)


# --- frag into an enemy trench ------------------------------------------------

func _frag_assault(s: Dictionary, t: float) -> bool:
	if t >= _next_scan:
		_next_scan = t + 8.0
		_cells = _trench_cells(s)
		# Late in the run, with no trench in reach yet, any spotted enemy
		# in throwing range will do for a frag (saved as s1b_frag_enemy;
		# the trench shot is still wanted).
		if t >= OPEN_FRAG_S and not ap.shots.has(PREFIX + "frag_enemy"):
			for so in s.get("soldiers", []):
				if int(so.owner) != local:
					_cells.append({"pos": so.pos, "foe": int(so.id), "open": true})
	# Any man of ours already in range of an enemy trench throws (the S1
	# script keeps pushing riflemen at the points the AI digs in at).
	if t >= _next_try and not _cells.is_empty():
		_next_try = t + 1.0
		var shot := _thrower_for(_cells)
		if not shot.is_empty():
			_throw_frag(shot)
			return true
	# Else assault the trench spot nearest one of our squads (within
	# ASSAULT_REACH_M): walk to 45 m short, screen the last stretch with
	# the squad's smoke, then walk in to 16 m short (plain moves, so the
	# men do not stop to trade fire at range). The throw itself is the
	# check above.
	if _assault.is_empty():
		for tgt in _cells:
			if tgt.get("open", false):
				continue
			var sq := _assault_squad(s, tgt.pos)
			if sq.is_empty() or sq.pos.distance_to(tgt.pos) > ASSAULT_REACH_M:
				continue
			var limit: float = ASSAULT_S + sq.pos.distance_to(tgt.pos) / ASSAULT_SPEED
			_assault = {"squad": int(sq.id), "pos": tgt.pos, "since": t, "limit": limit, "phase": "approach"}
			reserved[int(sq.id)] = true
			_assault_move(sq, 45.0)
			print("autoplay s1b: %s#%d (%d men) goes for an enemy trench at (%d,%d)%s, %d m away (%d trench spots known)" % [
				sq.type, sq.id, sq.men, roundi(tgt.pos.x), roundi(tgt.pos.z),
				" with enemy %d in it" % tgt.foe if int(tgt.foe) >= 0 else " (nobody seen in it)",
				roundi(sq.pos.distance_to(tgt.pos)), _cells.size()])
			break
		return false
	var sid: int = _assault.squad
	var sq_now: Dictionary = view.squads_by_id.get(sid, {})
	if sq_now.is_empty() or t - float(_assault.since) > float(_assault.limit):
		print("autoplay s1b: assault by squad %d gave up at %s (%s)" % [sid, UiPalette.mmss(t),
			"squad gone" if sq_now.is_empty() else "%d men, %s, still %d m away" % [int(sq_now.men), sq_now.status, roundi(sq_now.pos.distance_to(_assault.pos))]])
		_tried[_key_of(_assault.pos)] = true
		reserved.erase(sid)
		_assault = {}
		return false
	var near := INF
	for id in view.squad_units.get(sid, PackedInt32Array()):
		var p: Vector3 = view.soldiers_by_id[id].pos
		near = minf(near, Vector2(p.x, p.z).distance_to(Vector2(_assault.pos.x, _assault.pos.z)))
	var men: PackedInt32Array = view.squad_units.get(sid, PackedInt32Array())
	if _assault.phase == "approach" and near < 130.0:
		# Within rifle range of the trench: go prone and crawl (harder to
		# spot, and prone men are not pinned flat as fast).
		_assault.phase = "crawl"
		view.emit_intent(&"set_stance", {"units": men, "stance": "prone"})
		print("autoplay s1b:   squad %d crawls from %d m" % [sid, roundi(near)])
	if _assault.phase in ["approach", "crawl"] and (near < 52.0 or sq_now.order == "idle"):
		_assault.phase = "close"
		var to: Vector3 = (_assault.pos - sq_now.pos)
		to.y = 0.0
		var cover: Vector3 = ap._ground(sq_now.pos + to.normalized() * minf(24.0, to.length() * 0.5))
		if int(sq_now.get("items", {}).get("smoke", 0)) > 0:
			view.emit_intent(&"throw", {"units": men, "item": "smoke", "target": cover})
			print("autoplay s1b:   smoke to cover the last %d m" % roundi(near))
		_assault_move(sq_now, 16.0)
	return false


## Orders the assault squad to `short_m` short of the target (plain move).
func _assault_move(sq: Dictionary, short_m: float) -> void:
	var tgt: Vector3 = _assault.pos
	var back: Vector3 = sq.pos - tgt
	back.y = 0.0
	var stop: Vector3 = ap._ground(tgt + back.normalized() * minf(short_m, back.length()))
	ap._intent(&"move", {"squads": PackedInt32Array([int(sq.id)]), "target": stop}, stop)


## The best throw now: an own unpinned man with a frag 8..FRAG_RANGE_M m
## from a trench spot, spots with a seen enemy first, then the nearest.
## {squad, man, pos, foe} or {}.
func _thrower_for(cells: Array) -> Dictionary:
	var best := {}
	var best_score := INF
	for id in view.soldiers_by_id:
		var so: Dictionary = view.soldiers_by_id[id]
		if int(so.owner) != local or so.get("pinned", false) or view.item_count(so, "frag") == 0:
			continue
		var p := Vector2(so.pos.x, so.pos.z)
		for c in cells:
			var d := p.distance_to(Vector2(c.pos.x, c.pos.z))
			if d < 8.0 or d > FRAG_RANGE_M:
				continue
			var score := d + (0.0 if int(c.foe) >= 0 else 100.0) + (1000.0 if c.get("open", false) else 0.0)
			if score < best_score:
				best_score = score
				best = {"squad": int(so.squad), "man": int(id), "pos": c.pos, "foe": int(c.foe), "open": c.get("open", false)}
	return best


func _throw_frag(shot: Dictionary) -> void:
	ap._busy = true
	var sid: int = shot.squad
	var thrower: int = shot.man
	var target: Vector3 = shot.pos
	var foe: int = shot.foe
	var open: bool = shot.get("open", false)
	var thrower_pos: Vector3 = view.soldiers_by_id[thrower].pos
	await _look_at(thrower_pos.lerp(target, 0.5), 42.0, -52.0)
	# Click the thrower (one man), arm G, aim at the man in the trench.
	await _click_soldier(thrower, false)
	if view.selection.selected.is_empty():
		view.selection.select_squads(PackedInt32Array([sid]))
	await _key(KEY_G)
	# Aim at the enemy where he is now if he is still in range, else at
	# the spot found; give up if the thrower is out of range of both.
	var aim: Vector3 = target
	var from := Vector2(view.soldiers_by_id.get(thrower, {"pos": thrower_pos}).pos.x, view.soldiers_by_id.get(thrower, {"pos": thrower_pos}).pos.z)
	var reach := func(p: Vector3) -> bool:
		var d := from.distance_to(Vector2(p.x, p.z))
		return d >= 6.0 and d <= FRAG_RANGE_M + 2.0
	if foe >= 0 and (open or view.soldiers_by_id.get(foe, {}).get("in_trench", false)) and view.soldiers_by_id.has(foe) \
			and reach.call(view.soldiers_by_id[foe].pos):
		aim = view.soldiers_by_id[foe].pos
	if not reach.call(aim):
		print("autoplay s1b: frag target out of the thrower's range by now; skipping it")
		_tried[_key_of(target)] = true
		view.selection.select_only(PackedInt32Array())
		ap._busy = false
		return
	view.selection.cursor_override = camera.unproject_position(aim)
	await _snap("frag_enemy_tool" if open else "frag_tool", 0.25)
	view.selection.cursor_override = null
	var t0: float = view.state.get("time_s", 0.0)
	var frags0 := view.item_count(view.soldiers_by_id.get(thrower, {}), "frag")
	await _click_screen(camera.unproject_position(aim), false)
	print("autoplay s1b: frag thrown by %d at (%d,%d), %d m away, tool now '%s'" % [
		thrower, roundi(aim.x), roundi(aim.z), roundi(Vector2(thrower_pos.x, thrower_pos.z).distance_to(Vector2(aim.x, aim.z))), view.selection.tool])
	var burst: Variant = null
	var reported := false
	while burst == null and float(view.state.get("time_s", 0.0)) - t0 < BURST_WAIT_S:
		await ap.get_tree().process_frame
		var el := float(view.state.get("time_s", 0.0)) - t0
		if not reported and el >= 1.5:
			reported = true
			var so: Dictionary = view.soldiers_by_id.get(thrower, {})
			var frags: Array = view.state.get("thrown", []).filter(func(th: Dictionary) -> bool: return th.kind == "frag")
			print("autoplay s1b:   1.5 s later: thrower %s, order '%s', frags %d (had %d), pinned %s; frags in the air or fused: %d" % [
				"alive" if not so.is_empty() else "dead", so.get("order", ""), view.item_count(so, "frag"), frags0, so.get("pinned", false), frags.size()])
		for fx in view.state.get("effects", []):
			if fx.kind != "explosion":
				continue
			if fx.get("size", "") == "frag" and fx.pos.distance_to(aim) < 10.0:
				burst = fx.pos
			elif fx.pos.distance_to(aim) < 40.0:
				print("autoplay s1b:   %s burst %d m from the aim point" % [fx.get("size", "?"), roundi(fx.pos.distance_to(aim))])
	_tried[_key_of(target)] = true
	_cells = _cells.filter(func(c: Dictionary) -> bool: return not _tried.has(_key_of(c.pos)))
	if burst == null:
		print("autoplay s1b: no frag burst seen near the target; trying again elsewhere")
		view.selection.select_only(PackedInt32Array())
		ap._busy = false
		return
	var shot_name := "frag_enemy" if open else "frag_trench"
	ap.shots[PREFIX + shot_name] = true
	await _wait_real(0.05)
	await Session.save_screenshot(ap.dir.path_join(PREFIX + shot_name + ".png"))
	var depth: float = ap.source.dug_depth_at(burst.x, burst.z) if ap.source.has_method("dug_depth_at") else 0.0
	print("autoplay s1b: frag burst at (%d,%d), ground dug %.1f m there; target enemy %d: %s" % [
		roundi(burst.x), roundi(burst.z), depth, foe,
		"none seen" if foe < 0 else "in trench %s" % view.soldiers_by_id.get(foe, {}).get("in_trench", "(dead or unseen)")])
	if not _assault.is_empty():
		reserved.erase(int(_assault.squad))
	_assault = {}
	view.selection.select_only(PackedInt32Array())
	ap._busy = false


## Enemy trench spots, nearest our squads first: spotted enemy soldiers
## standing in a trench, and dug ground (at least TRENCH_DEPTH_M deep and
## long, see _elongated) within 35 m of a point we do not hold, away from
## our own trench work. Each is {pos, foe} (foe: a spotted enemy right
## there, or -1). Spots already tried are left out.
func _trench_cells(s: Dictionary) -> Array:
	var own: Array = ap._own(s)
	var out: Array = []
	for so in s.get("soldiers", []):
		if int(so.owner) != local and so.get("in_trench", false) and not _tried.has(_key_of(so.pos)):
			out.append({"pos": so.pos, "foe": int(so.id)})
	var src: Node = ap.source
	if src.has_method("dug_depth_at"):
		var ours: Array = []
		if not ap.dig_site.is_empty():
			ours.append(ap.dig_site.center)
		for sq in own:
			if sq.has("dig_line") and sq.dig_line.size() > 0:
				ours.append(sq.dig_line[0])
		var t: float = s.get("time_s", 0.0)
		var log_scan := t >= _next_scan_log
		if log_scan:
			_next_scan_log = t + 60.0
		for p in s.get("points", []):
			if int(p.owner) == local:
				continue
			var c: Vector3 = p.pos
			var deepest := 0.0
			var found := 0
			for i in range(-12, 13):
				for j in range(-12, 13):
					var q := c + Vector3(i * 3.0, 0.0, j * 3.0)
					if Vector2(q.x - c.x, q.z - c.z).length() > 35.0:
						continue
					var dd: float = src.dug_depth_at(q.x, q.z)
					deepest = maxf(deepest, dd)
					if dd < TRENCH_DEPTH_M or _tried.has(_key_of(q)) or not _elongated(src, q):
						continue
					if ours.any(func(o: Vector3) -> bool: return Vector2(o.x - q.x, o.z - q.z).length() < 40.0):
						continue
					var cell := {"pos": ap._ground(q), "foe": -1}
					for so in s.get("soldiers", []):
						if int(so.owner) != local and so.pos.distance_to(q) < 5.0:
							cell.foe = int(so.id)
					out.append(cell)
					found += 1
			if log_scan:
				print("autoplay s1b: point %s (owner %d): deepest dug ground within 35 m %.2f m, %d trench spots" % [p.id, int(p.owner), deepest, found])
	var near_own := func(p: Vector3) -> float:
		var best_d := INF
		for sq in own:
			best_d = minf(best_d, sq.pos.distance_to(p))
		return best_d
	out.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return near_own.call(a.pos) < near_own.call(b.pos))
	return out


## A trench, not a crater: along one of sixteen directions the ground
## stays dug for 10 m (every 2.5 m from -5 to +5). A shell crater is about
## 4 m across, and a field of them rarely lines up that far.
func _elongated(src: Node, q: Vector3) -> bool:
	for k in 16:
		var a := PI * k / 16.0
		var dir := Vector3(cos(a), 0.0, sin(a))
		var ok := true
		for step in [-5.0, -2.5, 2.5, 5.0]:
			var p: Vector3 = q + dir * step
			if float(src.dug_depth_at(p.x, p.z)) < 0.35:
				ok = false
				break
		if ok:
			return true
	return false


## Tried spots, on a 10 m grid.
func _key_of(p: Vector3) -> Vector2i:
	return Vector2i(roundi(p.x / 10.0), roundi(p.z / 10.0))


## Own rifle or engineer squad with frags and at least 4 men, not pinned
## or retreating, nearest `at` (a squad with smoke counts 60 m nearer).
func _assault_squad(s: Dictionary, at: Vector3) -> Dictionary:
	var best := {}
	var best_d := INF
	for sq in ap._own(s):
		if not sq.type in ["rifle", "engineer"] or int(sq.men) < 4 or sq.status in ["retreating", "pinned"]:
			continue
		if int(sq.get("items", {}).get("frag", 0)) == 0:
			continue
		var d: float = sq.pos.distance_to(at) - (60.0 if int(sq.get("items", {}).get("smoke", 0)) > 0 else 0.0)
		if d < best_d:
			best_d = d
			best = sq
	return best


# --- input and camera ---------------------------------------------------------

func _look_at(pos: Vector3, height: float, pitch: float) -> void:
	camera.height = height
	camera.pitch_deg = pitch
	camera.jump_to(pos)
	# Two frames: the camera moves, then the view projects the new picture.
	await ap.get_tree().process_frame
	await ap.get_tree().process_frame


func _snap(name: String, delay: float, size := Vector2i.ZERO) -> void:
	ap.shots[PREFIX + name] = true
	var win := ap.get_window()
	var want: Vector2i = size if size != Vector2i.ZERO else ap._size
	if win.mode == Window.MODE_WINDOWED and win.size != want:
		win.size = want
	await _wait_real(delay)
	await Session.save_screenshot(ap.dir.path_join(PREFIX + name + ".png"))


func _wait_real(s: float) -> void:
	await ap.get_tree().create_timer(s, true, false, true).timeout


## Waits `s` seconds of match time.
func _wait_game(s: float) -> void:
	var t0: float = view.state.get("time_s", 0.0)
	while float(view.state.get("time_s", 0.0)) - t0 < s:
		await ap.get_tree().process_frame


func _wait_cloud(near: Vector3, game_s: float) -> Variant:
	var t0: float = view.state.get("time_s", 0.0)
	while float(view.state.get("time_s", 0.0)) - t0 < game_s:
		await ap.get_tree().process_frame
		for c in view.state.get("smokes", []):
			if c.pos.distance_to(near) < 12.0 and float(c.radius) >= 6.0:
				return c
	return null


func _soldier_screen(id: int) -> Variant:
	var i := view.pick_ids.find(id)
	return view.pick_screen[i] if i >= 0 else null


func _click_soldier(id: int, double: bool) -> void:
	var p: Variant = _soldier_screen(id)
	if p == null:
		print("autoplay s1b: soldier %d is not on screen" % id)
		return
	if double:
		# A double-click is a click, then a press flagged double_click.
		await _click_screen(p, false)
		p = _soldier_screen(id)
		if p == null:
			return
	await _click_screen(p, double)


func _click_screen(p: Vector2, double: bool, button := MOUSE_BUTTON_LEFT) -> void:
	_motion(p, 0)
	_button(p, button, true, double)
	_button(p, button, false, false)
	# Move the (pushed) pointer off the men again: the real mouse is
	# elsewhere, so a hover tooltip would show at the wrong place.
	_motion(Vector2(4, 420), 0)
	await ap.get_tree().process_frame


func _right_click_ground(g: Vector3) -> void:
	await _click_screen(camera.unproject_position(g), false, MOUSE_BUTTON_RIGHT)


## Press, drag and release in one go: the selection controller ends a box
## in _process when Input says the button is up, and pushed events do not
## change Input's button state.
func _drag(a: Vector2, b: Vector2) -> void:
	_motion(a, 0)
	_button(a, MOUSE_BUTTON_LEFT, true, false)
	_motion(a.lerp(b, 0.5), MOUSE_BUTTON_MASK_LEFT)
	_motion(b, MOUSE_BUTTON_MASK_LEFT)
	_button(b, MOUSE_BUTTON_LEFT, false, false)
	await ap.get_tree().process_frame


func _key(code: Key, ctrl := false) -> void:
	for pressed in [true, false]:
		var e := InputEventKey.new()
		e.physical_keycode = code
		e.keycode = code
		e.ctrl_pressed = ctrl
		e.pressed = pressed
		_push(e)
	await ap.get_tree().process_frame


func _motion(p: Vector2, mask: int) -> void:
	var m := InputEventMouseMotion.new()
	m.position = p
	m.global_position = p
	m.button_mask = mask
	_push(m)


func _button(p: Vector2, button: MouseButton, pressed: bool, double: bool) -> void:
	var e := InputEventMouseButton.new()
	e.position = p
	e.global_position = p
	e.button_index = button
	e.pressed = pressed
	e.double_click = double
	e.button_mask = MOUSE_BUTTON_MASK_LEFT if pressed and button == MOUSE_BUTTON_LEFT else 0
	_push(e)


func _push(e: InputEvent) -> void:
	ap.get_viewport().push_input(e, true)
