extends SceneTree
## Headless test of MatchView input and intents on the mock source (S1b:
## soldier-level selection, HUD v2): soldier picking and its tie rule,
## double-click and Ctrl+double-click, box-select across two squads, Shift
## toggling, control groups (exact sets, double-tap centring, dead men
## dropping out), order mapping (whole squads get squad intents, the rest
## *_units), stance keys and the active-again-means-Auto rule, throw tools
## (range, Shift = throw_each, empty item refused), minimap click mapping in
## rotated and north-up mode, the HUD v2 layout at 1600x900 and 1280x800,
## roster and detail panel clicks, the call-in column, pause and
## surrender → end screen.
##
## S3 (vehicles on the mock): state flags per staged state, the MultiMesh
## buffers per (type, part), vehicle picking (footprint and 14 px),
## rings, right-click mapping (attack_unit, follow_units, Alt reverse),
## O / N / J, the roster card and detail panel, the ammo toggle, the
## call-in lock countdown and the vehicle limit, hit markers and feed
## lines, the 16-vehicle CPU cost (< 0.2 ms) and the 1280x800 roster fit
## with 4 vehicles and 8 squads.
##
##   Godot --headless --path godot -s res://tools/test_match_view.gd
##
## Exits 0 when every check passes. Events are fed straight to the
## selection controller and the HUD pieces (no OS input needed).

var _match: Node
## Untyped on purpose: a static MatchView reference would compile the view
## before the autoloads exist (-s scripts load first).
var _view: Node
var _sel: Node
var _source: Node
var _cam: Camera3D
var _intents: Array = []
var _fails := 0
var _checks := 0


func _initialize() -> void:
	_run.call_deferred()
	# Guard: a script error inside _run would otherwise hang the run.
	create_timer(180.0).timeout.connect(func() -> void:
		printerr("test_match_view: timed out")
		quit(2))


func _run() -> void:
	_match = load("res://match/match_mock.tscn").instantiate()
	root.add_child(_match)
	await _frames(5)
	_view = _match.view
	_sel = _view.selection
	_source = _match.source
	_cam = _match.camera
	# The mock's own fights would move and kill men under the test.
	_source.time_scale = 0.0
	_view.intent.connect(func(k: StringName, d: Dictionary) -> void: _intents.append([k, d]))
	var eng: int = _source.squad_ids(0, "engineer")[0]
	var rifle: int = _source.squad_ids(0, "rifle")[0]
	var mg: int = _source.squad_ids(0, "mg")[0]
	var mortar: int = _source.squad_ids(0, "mortar")[0]
	var eng_men: PackedInt32Array = _source.unit_ids(eng)
	var rifle_men: PackedInt32Array = _source.unit_ids(rifle)
	_cam.height = 120.0
	_cam.jump_to(_source.squad_pos(eng).lerp(_source.squad_pos(rifle), 0.5))
	await _frames(3)

	await _test_picking(eng, eng_men, rifle, rifle_men)
	await _test_groups(eng_men, rifle_men)
	await _test_order_mapping(eng, eng_men, rifle, rifle_men)
	await _test_stance(rifle_men)
	await _test_throw(eng, eng_men, mg)
	await _test_tools(eng, mortar)
	await _test_minimap()
	await _test_hud(eng, eng_men, rifle)
	await _test_vehicles()
	_test_cost()
	await _test_callin_need()
	await _test_end()

	print("test_match_view: %d/%d checks passed" % [_checks - _fails, _checks])
	quit(0 if _fails == 0 else 1)


# --- selection ---------------------------------------------------------------

func _test_picking(eng: int, eng_men: PackedInt32Array, rifle: int, rifle_men: PackedInt32Array) -> void:
	_check(_view.soldiers_by_id.size() > 0 and not _view.proxy_soldiers, "state has soldiers")
	# Click on a soldier selects that one man.
	var man := eng_men[1]
	_click(_screen(man))
	_check(_sel.selected == PackedInt32Array([man]), "click selects one soldier")
	var sel_i: Variant = _last(&"select")
	_check(sel_i != null and sel_i.units == PackedInt32Array([man]) and sel_i.squads == PackedInt32Array([eng]), "select intent carries units and squads")
	_click(Vector2(5, 450))
	_check(_sel.selected.is_empty(), "click on empty ground clears")
	# Within 14 px picks; beyond it does not.
	_click(_screen(man) + Vector2(9, 0))
	_check(_sel.selected.size() == 1, "click within 14 px picks a soldier")
	_click(Vector2(5, 450))

	# Tie rule: two candidates at the same screen distance → closer to the
	# camera wins (checked on injected pick arrays).
	var saved := [_view.pick_ids, _view.pick_screen, _view.pick_depth, _view.pick_own, _view.pick_reach]
	_view.pick_reach = PackedFloat32Array([0.0, 0.0])
	_view.pick_ids = PackedInt32Array([eng_men[0], eng_men[1]])
	_view.pick_screen = PackedVector2Array([Vector2(500, 500), Vector2(500.4, 500)])
	_view.pick_depth = PackedFloat32Array([90.0, 60.0])
	_view.pick_own = PackedByteArray([1, 1])
	_check(_sel.pick(Vector2(500, 500)) == eng_men[1], "near-tie goes to the man closer to the camera")
	_view.pick_screen = PackedVector2Array([Vector2(500, 500), Vector2(508, 500)])
	_check(_sel.pick(Vector2(500, 500)) == eng_men[0], "clearly nearer on screen wins over depth")
	_check(_sel.pick(Vector2(530, 500)) == -1, "nothing beyond 14 px")
	# Close up a man is tall on screen: a click on his head (40 px above
	# the pick point) still picks him.
	_view.pick_reach = PackedFloat32Array([50.0, 0.0])
	_check(_sel.pick(Vector2(500, 460)) == eng_men[0], "close up, a click on his head picks him")
	_view.pick_reach = saved[4]
	_view.pick_ids = saved[0]
	_view.pick_screen = saved[1]
	_view.pick_depth = saved[2]
	_view.pick_own = saved[3]

	# Double-click selects his squad; Shift+double-click adds a squad.
	_double(_screen(man))
	_check(_sel.selected == eng_men, "double-click selects the whole squad")
	_double(_screen(rifle_men[0]), false, true)
	var both := eng_men.duplicate()
	both.append_array(rifle_men)
	both.sort()
	_check(_sel.selected == both, "Shift+double-click adds a squad")
	# Ctrl+double-click: every own squad of that type with a man on screen.
	_click(Vector2(5, 450))
	_double(_screen(rifle_men[0]), true)
	var all_rifle := true
	for id in _sel.selected:
		all_rifle = all_rifle and _view.squad_type(_view.soldier_squad(id)) == "rifle"
	_check(_sel.squad_fully_selected(rifle) and all_rifle, "Ctrl+double-click selects that squad type on screen")

	# Box-select across two squads: a box around the engineers and the
	# rifle squad's first men.
	var box := Rect2(_screen(eng_men[0]), Vector2.ZERO)
	for id in eng_men:
		box = box.expand(_screen(id))
	box = box.expand(_screen(rifle_men[0])).expand(_screen(rifle_men[1])).grow(4.0)
	_drag(box.position, box.end)
	var want := PackedInt32Array()
	for i in _view.pick_ids.size():
		if _view.pick_own[i] == 1 and box.has_point(_view.pick_screen[i]):
			want.append(_view.pick_ids[i])
	want.sort()
	var squads_hit: PackedInt32Array = _sel.selected_squads()
	_check(_sel.selected == want and squads_hit.has(eng) and squads_hit.has(rifle), "box-select picks soldiers from two squads (%d men)" % want.size())
	var any_enemy := false
	for id in _sel.selected:
		any_enemy = any_enemy or not _view.own_soldier(id)
	_check(not any_enemy, "box-select never takes enemies")

	# Shift-click toggles one man out and back in; Shift-box adds.
	var t := eng_men[2]
	_click(_screen(t), true)
	_check(not _sel.is_selected(t), "Shift-click removes a selected man")
	_click(_screen(t), true)
	_check(_sel.is_selected(t), "Shift-click adds him back")
	_click(Vector2(5, 450))
	_click(_screen(eng_men[0]))
	var b2 := Rect2(_screen(rifle_men[2]), Vector2.ZERO).grow(5.0)
	_drag(b2.position, b2.end, true)
	_check(_sel.is_selected(eng_men[0]) and _sel.is_selected(rifle_men[2]), "Shift-box adds to the selection")
	_drag(b2.position, b2.end, true)
	_check(_sel.is_selected(eng_men[0]) and not _sel.is_selected(rifle_men[2]), "Shift-box over selected men only takes them out")


func _test_groups(eng_men: PackedInt32Array, rifle_men: PackedInt32Array) -> void:
	var grp := PackedInt32Array([eng_men[0], eng_men[3], rifle_men[1]])
	grp.sort()
	_sel.select_only(grp)
	_key(KEY_2, true)
	_click(Vector2(5, 450))
	_key(KEY_2)
	_check(_sel.selected == grp, "Ctrl+2 / 2 restores the exact soldier set")
	await _frames(1)
	_check(_view.roster.tabs.any(func(tb: Dictionary) -> bool: return tb.group == 2), "group 2 shows a roster tab")
	# Double-tap centres the camera on the group.
	_cam.jump_to(Vector3(300, 0, -300))
	_key(KEY_2)
	_key(KEY_2)
	var c := Vector3.ZERO
	for id in grp:
		c += _view.soldiers_by_id[id].pos
	c /= grp.size()
	_check(Vector2(_cam.focus.x, _cam.focus.z).distance_to(Vector2(c.x, c.z)) < 1.0, "pressing 2 twice centres the camera on the group")
	# Dead men drop out.
	_source.kill_soldier(eng_men[3])
	await _frames(2)
	_click(Vector2(5, 450))
	_key(KEY_2)
	_check(_sel.selected.size() == 2 and not _sel.is_selected(eng_men[3]), "dead men drop out of a group")
	_cam.jump_to(_source.squad_pos(_source.squad_ids(0, "engineer")[0]))
	await _frames(2)


# --- orders ------------------------------------------------------------------

func _test_order_mapping(eng: int, _eng_men: PackedInt32Array, rifle: int, rifle_men: PackedInt32Array) -> void:
	var eng_now: PackedInt32Array = _source.unit_ids(eng)
	var part := PackedInt32Array([rifle_men[0], rifle_men[3]])
	var sel := eng_now.duplicate()
	sel.append_array(part)
	_sel.select_only(sel)
	_intents.clear()
	var g := _ground_screen(_source.squad_pos(eng) + Vector3(0, 0, -30))
	_rclick(g)
	var mv: Variant = _last(&"move")
	var mu: Variant = _last(&"move_units")
	_check(mv != null and mv.squads == PackedInt32Array([eng]), "fully selected squad gets the squad move")
	_check(mu != null and mu.units == part and mu.target is Vector3, "partly selected men get move_units")
	await _frames(2)
	_check(_view.soldiers_by_id[rifle_men[0]].get("detached", false) and _view.soldiers_by_id[rifle_men[0]].has("target"), "mock detaches the moved men and reports their target")
	_check(not _view.soldiers_by_id[rifle_men[1]].get("detached", true), "unselected squad mates stay with the squad")
	# Ctrl+right-click: attack-move, same split.
	_intents.clear()
	_rclick(g, true)
	_check(_last(&"attack_move") != null and _last(&"attack_move_units") != null and _last(&"move") == null, "Ctrl+right-click attack-moves (squad + units)")
	# H stop, R retreat: same split.
	_intents.clear()
	_key(KEY_H)
	_check(_last(&"stop") != null and _last(&"stop").squads == PackedInt32Array([eng]) and _last(&"stop_units").units == part, "H: stop + stop_units")
	_key(KEY_R)
	_check(_last(&"retreat") != null and _last(&"retreat_units") != null, "R: retreat + retreat_units")
	# L regroup: every squad with a selected man.
	_key(KEY_L)
	var rg: Variant = _last(&"regroup")
	var want := PackedInt32Array([eng, rifle])
	want.sort()
	_check(rg != null and rg.squads == want, "L: regroup the selected men's squads")
	# A unit order that covers every living man is a squad order.
	_sel.select_only(_source.unit_ids(rifle))
	_intents.clear()
	_rclick(g)
	_check(_last(&"move") != null and _last(&"move_units") == null, "all men of a squad selected → squad intent only")
	# Barrage is a squad order: one selected mortar man → his squad.
	var mortar: int = _source.squad_ids(0, "mortar")[0]
	_sel.select_only(PackedInt32Array([_source.unit_ids(mortar)[1]]))
	_cam.jump_to(_source.squad_pos(mortar) + Vector3(0, 0, -150))
	await _frames(2)
	_intents.clear()
	_rclick(_cam.unproject_position(_source.squad_pos(mortar) + Vector3(0, 0, -150)))
	var br: Variant = _last(&"barrage")
	_check(br != null and br.squads == PackedInt32Array([mortar]), "barrage for a selected mortar man goes to his squad")
	_cam.jump_to(_source.squad_pos(eng))
	await _frames(2)


func _test_stance(rifle_men: PackedInt32Array) -> void:
	# S3b Fix: a squad half ducked below a trench lip reads "Heads down".
	_check(_view.squad_status({"status": "heads_down"}) == ["Heads down", true], "heads_down status reads Heads down")
	var two := PackedInt32Array([rifle_men[1], rifle_men[2]])
	_sel.select_only(two)
	_intents.clear()
	_key(KEY_X)
	var st: Variant = _last(&"set_stance")
	_check(st != null and st.stance == "stand" and st.units == two, "X sets stand on the selected men")
	await _frames(2)
	_check(_view.soldiers_by_id[two[0]].stance_override == "stand", "mock applies the stance override")
	_check(_view.orders.active(&"stand"), "the stance strip shows Stand as active")
	_key(KEY_X)
	_check(_last(&"set_stance").stance == "auto", "X again returns to Auto")
	# Same rule before the state has caught up (pending overrides).
	_key(KEY_C)
	_key(KEY_C)
	_check(_last(&"set_stance").stance == "auto", "C twice in one frame: crouch, then Auto")
	_key(KEY_Z)
	_check(_last(&"set_stance").stance == "prone", "Z sets prone")
	# Mixed overrides: the key sets, it does not toggle.
	await _frames(2)
	_sel.select_only(PackedInt32Array([rifle_men[1], rifle_men[4]]))
	_key(KEY_Z)
	_check(_last(&"set_stance").stance == "prone", "Z on a mixed selection sets prone")
	_view.orders.press(&"auto")
	_check(_last(&"set_stance").stance == "auto", "the Auto segment sets Auto")


func _test_throw(eng: int, _eng_men: PackedInt32Array, mg: int) -> void:
	# Earlier tests left the engineers retreating; hold them in place.
	_source.handle_intent(&"stop", {"squads": PackedInt32Array([eng])})
	var men: PackedInt32Array = _source.unit_ids(eng)
	_sel.select_only(men)
	await _frames(1)
	var frags: int = _sel.item_count("frag")
	_check(frags > 0, "engineers carry frags (%d)" % frags)
	_key(KEY_G)
	_check(_sel.tool == &"throw" and _sel.throw_item == "frag", "G arms the frag tool")
	_check(_view.orders.active(&"frag"), "the Frag button shows the armed tool")
	var p: Vector3 = _view.soldiers_by_id[men[0]].pos
	_intents.clear()
	# Out of range (about 120 m): refused, the tool stays armed.
	var far := _ground_screen(p + Vector3(0, 0, -120))
	_click(far)
	_check(_last(&"throw") == null and _sel.tool == &"throw", "out-of-range click does not throw")
	_check(not _sel.throw_in_range("frag", _view.ground_pick(far)), "far target reads as out of range (red cursor)")
	var near := _ground_screen(p + Vector3(12, 0, -12))
	_check(_sel.throw_in_range("frag", _view.ground_pick(near)), "a 17 m target is in range")
	_click(near)
	var th: Variant = _last(&"throw")
	_check(th != null and th.item == "frag" and th.units == men and th.target is Vector3, "click throws one frag (units = selection)")
	_check(_sel.tool == &"", "tool clears after the throw")
	_source.prewarm(1.5)
	await _frames(2)
	_check(_sel.item_count("frag") == frags - 1, "mock: exactly one frag thrown")
	# Shift+click: every eligible man throws.
	_key(KEY_G)
	_intents.clear()
	_click(_ground_screen(p + Vector3(-10, 0, -14)), true)
	_check(_last(&"throw_each") != null and _last(&"throw") == null, "Shift+click sends throw_each")
	# Satchel: 3–8 m only.
	_key(KEY_F)
	_check(_sel.tool == &"throw" and _sel.throw_item == "satchel", "F arms the satchel tool")
	_intents.clear()
	_click(_ground_screen(p + Vector3(0, 0, -20)))
	_check(_last(&"throw") == null, "satchel at 20 m is refused")
	_key(KEY_ESCAPE)
	_check(_sel.tool == &"" and not _view.pause_menu.visible, "Esc cancels the throw tool")
	# A team without items cannot arm it.
	_sel.select_only(_source.unit_ids(mg))
	_key(KEY_G)
	_check(_sel.tool == &"" and not _view.orders.enabled(&"frag"), "frag greyed and refused for an MG team (count 0)")
	_source.prewarm(6.0)
	await _frames(2)


func _test_tools(eng: int, mortar: int) -> void:
	_cam.jump_to(_source.squad_pos(eng))
	await _frames(2)
	_sel.select_squads(PackedInt32Array([eng]))
	# Dig tool: points along the ground, capped at 60 m, right-click digs.
	_key(KEY_Y)
	_check(_sel.tool == &"dig", "Y arms the dig tool")
	for x in [300, 500, 700, 900, 1100, 1300, 1500]:
		_click(Vector2(x, 450))
	_check(_sel.dig_length() <= 60.01, "dig polyline capped at 60 m (%.1f)" % _sel.dig_length())
	_check(_sel.dig_points.size() >= 2, "dig points placed")
	_key(KEY_BACKSPACE)
	var n_after_undo: int = _sel.dig_points.size()
	_intents.clear()
	_rclick(Vector2(800, 450))
	var dig: Variant = _last(&"dig")
	_check(dig != null and dig.polyline.size() == n_after_undo and dig.squads == PackedInt32Array([eng]), "right-click emits dig with the polyline")
	# Dig needs a digger: a mortar team alone cannot.
	_sel.select_squads(PackedInt32Array([mortar]))
	_key(KEY_Y)
	_check(_sel.tool == &"", "dig refused for a mortar team")
	_key(KEY_B)
	_check(_sel.tool == &"barrage", "B arms barrage for mortars")
	_key(KEY_ESCAPE)
	_check(_sel.tool == &"" and not _view.pause_menu.visible, "Esc cancels the tool first")
	# Orders grid buttons.
	_sel.select_squads(PackedInt32Array([eng]))
	_intents.clear()
	_view.orders.press(&"stop")
	_check(_last(&"stop") != null, "Stop button emits stop")
	_view.orders.press(&"attack_move")
	_check(_sel.tool == &"attack_move", "Attack button arms the attack-move tool")
	_click(Vector2(800, 400))
	_check(_last(&"attack_move") != null and _sel.tool == &"", "attack-move tool places and clears")


func _test_minimap() -> void:
	var mm: Control = _view.minimap
	var half: float = _view.map_half
	# North-up: the map's north-west corner is the box's top-left.
	mm.rotate_with_camera = false
	_cam.yaw_deg = 0.0
	_cam.jump_to(Vector3.ZERO)
	await _frames(1)
	var nw: Vector2 = mm.world_to_map(Vector3(-half, 0, -half))
	_check(nw.distance_to(Vector2.ZERO) < 1.0, "north-up: north-west corner at top-left")
	_mm_click(mm, mm.world_to_map(Vector3(200, 0, -100)))
	_check(Vector2(_cam.focus.x, _cam.focus.z).distance_to(Vector2(200, -100)) < 2.0, "north-up click moves the camera there")
	# Rotated with the camera: forward is up, clicks go through the inverse.
	mm.rotate_with_camera = true
	_cam.yaw_deg = 90.0  # looking west
	_cam.jump_to(Vector3.ZERO)
	await _frames(1)
	var hs: float = mm.map_half_px()
	_check(absf(hs - mm.size.x * 0.5 / sqrt(2.0)) < 0.5, "rotated: whole map fits the box at any yaw (side = box / sqrt 2)")
	var up: Vector2 = mm.centre() + Vector2(0, -hs * 0.5)
	var w: Vector3 = mm.map_to_world(up)
	_check(Vector2(w.x, w.z).distance_to(Vector2(-half * 0.5, 0)) < 1.0, "rotated: up on the minimap is camera forward (west)")
	var target := Vector3(200, 0, -100)
	var at: Vector2 = mm.world_to_map(target)
	var want: Vector2 = mm.centre() + Vector2(100, 200) / half * hs
	_check(at.distance_to(want) < 0.5, "rotated: world point lands where the rotation puts it")
	_mm_click(mm, at)
	_check(Vector2(_cam.focus.x, _cam.focus.z).distance_to(Vector2(200, -100)) < 2.0, "rotated click moves the camera to the right world point")
	# Right-click orders through the same mapping.
	_sel.select_squads(_source.squad_ids(0, "rifle").slice(0, 1))
	_intents.clear()
	var ev := InputEventMouseButton.new()
	ev.button_index = MOUSE_BUTTON_RIGHT
	ev.pressed = true
	ev.position = mm.world_to_map(Vector3(-150, 0, 50))
	mm._gui_input(ev)
	var mv: Variant = _last(&"move")
	_check(mv != null and Vector2(mv.target.x, mv.target.z).distance_to(Vector2(-150, 50)) < 2.0, "rotated right-click orders a move to the right world point")
	_check(not mm._inside(Vector2(1, 1)), "rotated: box corners pass through")
	# No disc: off the turned square (here axis-aligned, yaw 90) the box
	# passes clicks through, except on the lock button.
	_check(not mm._has_point(mm.centre() + Vector2(hs + 8.0, 0)), "rotated: just off the map edge passes through")
	_check(mm._has_point(mm.lock_centre()) and not mm._inside(mm.lock_centre()), "rotated: lock button clickable, off the map")
	# Camera footprint bigger than the map: clipped to the map square.
	var big := []
	for u in [Vector2(-2, -2), Vector2(2, -2), Vector2(2, 2), Vector2(-2, 2)]:
		big.append(Vector3(u.x * half, 0, u.y * half))
	var fp: PackedVector2Array = mm._footprint(big)
	var fp_in := fp.size() >= 4
	for p in fp:
		var u: Vector2 = mm._map_units(p)
		fp_in = fp_in and absf(u.x) <= 1.001 and absf(u.y) <= 1.001
	_check(fp_in, "rotated: camera footprint clipped to the map square")
	# Turned 45°: the diamond's corners reach the box edge; the lock stays off it.
	_cam.yaw_deg = 45.0
	await _frames(1)
	_check(mm._inside(mm.centre() + Vector2(mm.size.x * 0.5 - 3.0, 0)), "rotated 45: diamond corner reaches the box edge")
	# Probe the corner beyond the lock button (it sits at the north
	# marker's offset, about 0.32 of the box along the diagonal).
	_check(not mm._has_point(mm.centre() + Vector2(mm.size.x * 0.45, mm.size.y * 0.45)) and not mm._inside(mm.lock_centre()), "rotated 45: empty corner passes through, lock off the map")
	# The lock button follows the turned map: whole button inside the box
	# and off the map (its rim and click radius too) at each yaw, at this
	# box size and the 1280 one (200 px).
	var box_size: Vector2 = mm.size
	for side in [box_size.x, 200.0]:
		mm.size = Vector2(side, side)
		var lock_ok := true
		for yaw in [0.0, 30.0, 45.0, 60.0, 90.0, 135.0, 200.0, 315.0]:
			_cam.yaw_deg = yaw
			var lc: Vector2 = mm.lock_centre()
			var r: float = mm.LOCK_R
			lock_ok = lock_ok and Rect2(Vector2(r, r), mm.size - Vector2(2 * r, 2 * r)).has_point(lc)
			for k in 16:
				lock_ok = lock_ok and not mm._inside(lc + Vector2.RIGHT.rotated(TAU * k / 16.0) * (r + 2.0))
		_check(lock_ok, "rotated %d px: lock inside the box and off the map at 0..315°" % side)
	mm.size = box_size
	# No jumps: it slides smoothly through 45° and over a full turn.
	_cam.yaw_deg = 44.5
	var l_a: Vector2 = mm.lock_centre()
	_cam.yaw_deg = 45.5
	_check(l_a.distance_to(mm.lock_centre()) < 2.0, "rotated: lock steady through 45°")
	var worst := 0.0
	_cam.yaw_deg = 0.0
	var prev: Vector2 = mm.lock_centre()
	for i in range(1, 361):
		_cam.yaw_deg = float(i)
		var cur: Vector2 = mm.lock_centre()
		worst = maxf(worst, cur.distance_to(prev))
		prev = cur
	_check(worst < 2.0, "rotated: lock moves under 2 px per degree (%.2f)" % worst)
	# It follows the map: not fixed in the box corner.
	_cam.yaw_deg = 0.0
	var l0: Vector2 = mm.lock_centre()
	_cam.yaw_deg = 45.0
	_check(l0.distance_to(mm.lock_centre()) > 10.0, "rotated: lock follows the map as it turns")
	# Clicking the lock closes it (north-up) and clicking again opens it
	# (rotating); the camera stays put. Settings is detached so the test
	# never writes the player's settings file.
	var st: Node = get_root().get_node_or_null(^"Settings")
	if st != null:
		get_root().remove_child(st)
	var focus0: Vector3 = _cam.focus
	_mm_click(mm, mm.lock_centre())
	await _frames(1)
	_check(not mm.rotate_with_camera and mm._get_tooltip(mm.lock_centre()) == "Rotate with the camera", "lock click: closed, north-up")
	_mm_click(mm, mm.lock_centre())
	await _frames(1)
	_check(mm.rotate_with_camera and mm._get_tooltip(mm.lock_centre()) == "Lock north-up", "lock click again: open, rotating")
	_check(Vector3(_cam.focus).distance_to(focus0) < 0.01, "lock clicks leave the camera where it was")
	if st != null:
		get_root().add_child(st)
	mm.rotate_with_camera = false
	_cam.yaw_deg = 0.0
	_cam.jump_to(_source.squad_pos(_source.squad_ids(0, "engineer")[0]))
	await _frames(2)


# --- HUD -----------------------------------------------------------------------

func _test_hud(eng: int, eng_men: PackedInt32Array, rifle: int) -> void:
	var hud: Control = _view.hud
	# Headless viewports are 1600x1600; lay the HUD out for each target size.
	hud.force_size = Vector2(1600, 900)
	await _frames(2)
	_check(not hud.compact, "1600x900 uses the normal metrics")
	_check_layout(hud, Vector2(1600, 900), "1600x900")
	hud.force_size = Vector2(1280, 800)
	await _frames(2)
	_check(hud.compact, "1280x800 uses the compact metrics")
	_check_layout(hud, Vector2(1280, 800), "1280x800")
	_check(_view.call_in.position.y >= 460.0 - 0.5, "1280: call-in column top at or below y460 (%.0f)" % _view.call_in.position.y)
	_check(absf(_view.minimap.size.x - 200.0) < 0.5 and absf(_view.detail.size.x - 262.0) < 0.5, "1280: 200 px minimap, 262 px detail")
	hud.force_size = Vector2.ZERO
	hud.relayout()
	await _frames(2)

	# Roster: card click selects the squad, pip click one man.
	var roster: Control = _view.roster
	var r: Rect2 = roster.card_rect(eng)
	_check(r.size.x > 0.0, "engineers have a roster card")
	_gui_click(roster, r.position + Vector2(r.size.x - 20.0, 10.0))
	_check(_sel.selected == _source.unit_ids(eng), "card click selects the whole squad")
	var pip_man := -1
	var pip_rect := Rect2()
	for c in roster.cards:
		if c.squad == eng:
			for pp in c.pips:
				if pp[1] != -1 and pip_man == -1:
					pip_man = pp[1]
					pip_rect = pp[0]
	_gui_click(roster, pip_rect.get_center())
	_check(pip_man != -1 and _sel.selected == PackedInt32Array([pip_man]), "pip click selects that man")
	_gui_click(roster, roster.card_rect(rifle).position + Vector2(20, 10), true)
	_check(_sel.squad_fully_selected(rifle) and _sel.is_selected(pip_man), "Shift+card click adds the squad")
	# Cards fit their text.
	var font: Font = roster.get_theme_default_font()
	var fits := true
	for c in roster.cards:
		fits = fits and c.rect.size.x + 0.5 >= roster.card_width(c.squad, font, HudStyle.bold())
	_check(fits, "every card is at least as wide as its text")
	# Detail panel: squad rows in pip order; a row click selects that man.
	_sel.select_squads(PackedInt32Array([eng]))
	await _frames(1)
	var detail: Control = _view.detail
	_check(detail.mode() == "squad" and detail.rows.size() == int(_view.squads_by_id[eng].men_max), "detail lists the focused squad's men (one row per slot)")
	var row: Dictionary = detail.rows[1]
	_gui_click(detail, row.rect.get_center())
	_check(_sel.selected == PackedInt32Array([row.id]) and detail.mode() == "soldier", "row click selects that man; detail shows his readout")
	await _frames(1)
	# Event feed: at most 3 lines, click-through.
	for i in 5:
		_view.event_feed.push({"kind": "info", "text": "Test %d" % i})
	_check(_view.event_feed.lines.size() == 3 and _view.event_feed.mouse_filter == Control.MOUSE_FILTER_IGNORE, "feed keeps 3 lines and ignores the mouse")
	# Call-in (S3b, owner 2026-11-01): no queue, no delivery time. A click
	# buys at once, the squad stands in the base, the tile's cooldown runs.
	var ci: Control = _view.call_in
	var ri: int = ci.index_of("rifle")
	_source.players[0].manpower = 1000.0
	await _frames(1)
	_check(ci.is_ready(ri) and ci.blocked_reason(_view.state.callin.catalog[ri]) == "", "rifle tile ready (lit)")
	_check(not _view.state.callin.has("queue") and not _view.state.callin.has("queue_max"), "no call-in queue in the state")
	var rifles_before: int = _source.squad_ids(0, "rifle").size()
	_intents.clear()
	ci.call_in_index(ri)
	await _frames(2)
	_check(_last(&"call_in") != null and _last(&"call_in").type == "rifle", "call-in intent")
	_check(_source.squad_ids(0, "rifle").size() == rifles_before + 1, "the mock deployed the squad at once")
	var fed := false
	for l in _view.event_feed.lines:
		fed = fed or (l.kind == "squad_arrived" and String(l.text).ends_with("deployed at base."))
	_check(fed, "feed: <squad> deployed at base.")
	var rtile: Dictionary = _view.state.callin.catalog[ri]
	_check(float(rtile.get("cooldown_s", 0.0)) > 20.0 and float(rtile.get("cooldown_total_s", 0.0)) == 24.0 and ci.blocked_reason(rtile) == "cooldown" and not ci.is_ready(ri), "the buy starts the rifle cooldown (24 s)")
	# Clicked again on cooldown: no intent, the feed says why.
	_intents.clear()
	_check(not ci.call_in_index(ri) and _last(&"call_in") == null, "on cooldown: no call-in sent")
	_check(_view.event_feed.lines[0].kind == "call_in_refused" and String(_view.event_feed.lines[0].text).contains("not called in: ready again in"), "feed: refused, ready again in N s (%s)" % _view.event_feed.lines[0].text)
	# Short of manpower: the reason names the shortfall.
	_source.players[0].manpower = 10.0
	await _frames(1)
	var mgi: int = ci.index_of("mg")
	var mg_cost := int(_view.state.callin.catalog[mgi].cost)
	var short_why: String = ci.refusal_text(_view.state.callin.catalog[mgi])
	var short_n := int(short_why.trim_prefix("need ").get_slice(" ", 0))
	_check(short_why.ends_with("more manpower") and short_n > 0 and short_n <= mg_cost - 10, "short: need N more manpower (%s)" % short_why)
	# Tab focuses the column; then a number key buys that tile at once.
	_source.players[0].manpower = 1000.0
	await _frames(1)
	_key(KEY_TAB)
	_check(ci.focused, "Tab focuses the call-in column")
	_intents.clear()
	_panel_key(KEY_1 + mgi)
	await _frames(2)
	_check(_last(&"call_in") != null and _last(&"call_in").type == "mg", "Tab, then %d buys the MG team" % (mgi + 1))
	_check(float(_view.state.callin.catalog[mgi].get("cooldown_s", 0.0)) > 0.0, "the MG cooldown starts")
	ci.toggle_focus()
	_check(not ci.focused, "focus leaves the column again")
	_check(eng_men.size() > 0, "engineer roster sane")


func _check_layout(hud: Control, sz: Vector2, label: String) -> void:
	var tb: Rect2 = hud.rect_of(_view.top_bar)
	_check(tb.size.y >= 28.0 and tb.size.y <= 30.0 and absf(tb.size.x - sz.x) < 0.5, "%s: top bar full width, 28–30 px" % label)
	var mm: Rect2 = hud.rect_of(_view.minimap)
	_check(mm.position == Vector2(12, tb.size.y + 8.0), "%s: minimap top-left under the bar" % label)
	var ro: Rect2 = hud.rect_of(_view.roster)
	var de: Rect2 = hud.rect_of(_view.detail)
	var orr: Rect2 = hud.rect_of(_view.orders)
	var top := sz.y - 10.0 - float(hud.m.hc)
	_check(absf(ro.position.y - top) < 0.5 and absf(de.position.y - top) < 0.5 and absf(orr.position.y - top) < 0.5, "%s: console shares one top" % label)
	_check(absf(ro.end.y - (sz.y - 10.0)) < 0.5 and absf(de.end.y - (sz.y - 10.0)) < 0.5 and absf(orr.end.y - (sz.y - 10.0)) < 0.5, "%s: console shares one baseline" % label)
	_check(ro.end.x <= de.position.x and de.end.x <= orr.position.x and absf(orr.end.x - (sz.x - 12.0)) < 0.5, "%s: roster | detail | orders, right-aligned" % label)
	var ci: Rect2 = hud.rect_of(_view.call_in)
	# One column of 120 px tiles, or two (S3: 6+ tiles would rise too high).
	var cols: int = _view.call_in.columns
	_check(absf(ci.size.x - (120.0 * cols + 4.0 * (cols - 1))) < 0.5 and absf(ci.end.x - (sz.x - 12.0)) < 0.5 and ci.end.y <= top - 8.0 + 0.5, "%s: call-in %d column(s) of 120 px tiles, right, above the orders" % [label, cols])
	_check(ci.position.y >= sz.y * 0.575 - 0.5, "%s: call-in top at or below 57.5 %% of the height (%.0f)" % [label, ci.position.y])
	var fe: Rect2 = hud.rect_of(_view.event_feed)
	_check(absf(fe.end.x - (sz.x - 12.0)) < 0.5 and fe.position.y == tb.size.y + 8.0 and not fe.intersects(mm), "%s: feed top-right" % label)
	_check(ci.position.y > fe.end.y and not ci.intersects(orr), "%s: no overlaps on the right" % label)


# --- vehicles (S3) -------------------------------------------------------------

func _vehicle_tile() -> int:
	var cat: Array = _view.state.get("callin", {}).get("catalog", [])
	for i in cat.size():
		if cat[i].get("type", "") == "vehicle":
			return i
	return -1


func _vstate(id: int) -> Dictionary:
	return _view.vehicles_by_id.get(id, {})


func _test_vehicles() -> void:
	var ci: Control = _view.call_in
	await _frames(1)
	# Before the unlock: the vehicle tile is locked with a countdown.
	var vi := _vehicle_tile()
	var tile: Dictionary = _view.state.callin.catalog[vi] if vi >= 0 else {}
	_check(vi >= 0 and float(tile.get("locked_s", 0.0)) > 200.0, "vehicle tile locked before 4:00 (%.0f s left)" % float(tile.get("locked_s", 0.0)))
	_check(ci.blocked_reason(tile) == "locked" and tile.has("id") and int(tile.get("pop", 0)) == 12, "locked tile: reason, id and pop")
	_intents.clear()
	_check(not ci.call_in_index(vi) and _last(&"call_in") == null, "a locked tile sends no call-in")

	# Stage four own tanks, two seen enemies and two wrecks near the rifles.
	var rifles: PackedInt32Array = _source.squad_ids(0, "rifle")
	var escort_sq: int = rifles[1]
	var anchor: Vector3 = _source.squad_pos(escort_sq)
	var tanks: PackedInt32Array = _source.stage_vehicles(Vector2(anchor.x + 20.0, anchor.z + 10.0), Vector2(1, -1), escort_sq)
	var at_sq: int = _source._spawn(0, "at", Vector2(anchor.x + 8.0, anchor.z + 24.0))
	_source.prewarm(0.5)
	_sel.select_only(PackedInt32Array())
	# Clear the ground around the tanks of other men (picking tests).
	_source.place_squad(rifles[0], Vector2(anchor.x - 60.0, anchor.z + 40.0))
	_source.place_squad(at_sq, Vector2(anchor.x - 20.0, anchor.z + 50.0))
	_cam.yaw_deg = -45.0
	_cam.height = 75.0
	var c := Vector3.ZERO
	for id in tanks:
		c += _source.veh.vehicles[id].pos.x * Vector3.RIGHT + _source.veh.vehicles[id].pos.y * Vector3.BACK
	_cam.jump_to(c / tanks.size() + Vector3(1, 0, -1).normalized() * 25.0)
	await _frames(3)
	_check(tanks.size() == 4 and _view.vehicles_by_id.size() >= 8, "state has vehicles (%d)" % _view.vehicles_by_id.size())
	var bog := _vstate(tanks[1])
	_check(Vehicles.has(bog, Vehicles.BOGGED) and Vehicles.has(bog, Vehicles.CROSSING) and bog.state == "bogged", "bogged tank: BOGGED | CROSSING, badge Bogged")
	_check(Vehicles.has(_vstate(tanks[2]), Vehicles.BURNING) and _vstate(tanks[2]).state == "burning", "burning tank flagged")
	var rep := _vstate(tanks[3])
	_check(Vehicles.has(rep, Vehicles.REPAIRING) and rep.state == "repairing" and rep.repairing == "track_l" and rep.crew[1] == "swapping", "repairing tank: REPAIRING, track_l, gunner swapping")
	var enemy := -1
	var fresh := -1
	var old := -1
	for id in _view.vehicles_by_id:
		var v: Dictionary = _view.vehicles_by_id[id]
		if Vehicles.is_wreck(v):
			if Vehicles.has(v, Vehicles.KNOCKED_OUT):
				fresh = id
			else:
				old = id
		elif int(v.owner) == 1 and enemy == -1:
			enemy = id
	_check(fresh != -1 and old != -1 and int(_vstate(fresh).owner) == -1 and int(_vstate(fresh).squad) == -1, "wrecks: owner -1, a fresh one KNOCKED_OUT, an old one not")
	var ev := _vstate(enemy)
	_check(enemy != -1 and not ev.has("crew") and not ev.has("ammo") and (int(ev.flags) & ~(Vehicles.BURNING | Vehicles.WRECK | Vehicles.KNOCKED_OUT)) == 0, "enemy vehicle: no own-only keys, flags masked")

	# Renderer: one MultiMesh per (type, part), one instance per vehicle.
	var vv: VehicleView = _match.vehicle_view
	var mine := String(_vstate(tanks[0]).type)
	var n_mine := 0
	for id in _view.vehicles_by_id:
		if String(_view.vehicles_by_id[id].type) == mine:
			n_mine += 1
	var hull: MultiMesh = vv.batch(mine, 0)
	_check(hull != null and int(vv.counts.get(mine, -1)) == n_mine and hull.visible_instance_count == n_mine and vv.batch(mine, 2).visible_instance_count == n_mine, "one hull/turret/gun instance per %s (%d)" % [mine, n_mine])
	_check(hull != null and hull.buffer.size() == VehicleView.CAP * 16 and hull.use_custom_data, "buffer: CAP x 16 floats with custom data")
	var first: Dictionary = {}
	for v in _view.state.vehicles:
		if String(v.type) == mine:
			first = v
			break
	var b := hull.buffer
	var p0: Vector3 = first.pos
	_check(absf(b[3] - p0.x) < 0.01 and absf(b[7] - p0.y) < 0.01 and absf(b[11] - p0.z) < 0.01, "hull instance 0 sits at the first vehicle's pos")
	var tint := VehicleView.tint(String(MockVehicles.TYPES[mine].faction), 0, 0)
	_check(Vehicles.is_wreck(first) or (absf(b[12] - tint.r) < 0.01 and absf(b[15] - 1.0) < 0.01), "custom data: team tint and full brightness")
	_check(_match.vehicle_fx.counts.fires >= 1 and _match.vehicle_fx.counts.smokes >= 1, "fire on the burning tank, smoke over the fresh wreck")

	# Picking: centre, inside the footprint, outside. The escort file
	# behind the first tank overlaps it on screen, so test on a tank with
	# nobody in front of it (soldiers under the cursor win by design).
	var tp := -1
	for t in tanks:
		var k: int = _view.veh_pick_ids.find(t)
		if k >= 0 and _sel.pick_soldier(_view.veh_pick_screen[k], -1) == -1:
			tp = t
			break
	_check(tp != -1, "a tank with no soldier over it on screen (test setup)")
	var centre: Vector2 = _view.veh_pick_screen[_view.veh_pick_ids.find(tp)]
	_click(centre)
	_check(_sel.selected == PackedInt32Array([tp]) and _last(&"select").units == PackedInt32Array([tp]), "click on a tank selects it (units carry the vehicle id)")
	_check(_view.detail.mode() == "vehicle", "detail panel shows the vehicle")
	var v0 := _vstate(tp)
	var vt0: Dictionary = _view.vehicle_type(tp)
	var fwd := Vector3(-sin(v0.yaw), 0, -cos(v0.yaw))
	var nose := _cam.unproject_position(v0.pos + fwd * float(vt0.hull_length) * 0.42 + Vector3(0, 0.4, 0))
	_click(Vector2(5, 450))
	_check(nose.distance_to(centre) > 14.0 and _sel.pick_vehicle(nose, 1) == tp, "inside the footprint, %.0f px from the centre, picks the tank" % nose.distance_to(centre))
	var off := _cam.unproject_position(v0.pos + fwd * (float(vt0.hull_length) * 0.5 + 4.0))
	_check(_sel.pick_vehicle(off, 1) == -1, "4 m off the nose picks nothing")
	_sel.select_only(PackedInt32Array([tanks[0]]))
	_sel.hover_id = -1
	await _frames(1)
	var rings: MultiMesh = _match.get_node("GroundOverlay/VehicleRings").multimesh
	_check(rings.visible_instance_count == 1, "one vehicle ring for one selected tank")
	_check(_view.orders.grid_mode == "vehicle", "orders grid switches to the vehicle layout")

	# Orders.
	var g := _ground_screen(_view.unit_pos(tanks[0]) + Vector3(-30, 0, 30))
	_intents.clear()
	_rclick(g)
	_check(_last(&"move_units") != null and _last(&"move_units").units == PackedInt32Array([tanks[0]]) and _last(&"move") == null, "right-click: move_units with the tank")
	_intents.clear()
	var e := InputEventMouseButton.new()
	e.button_index = MOUSE_BUTTON_RIGHT
	e.pressed = true
	e.alt_pressed = true
	e.position = g
	_sel._unhandled_input(e)
	_check(_last(&"reverse_units") != null and _last(&"reverse_units").units == PackedInt32Array([tanks[0]]), "Alt+right-click: reverse_units")
	# Tank + AT team on an enemy tank: attack_unit for both, nobody moves.
	await _frames(1)
	var ie: int = _view.veh_pick_ids.find(enemy)
	var at_men: PackedInt32Array = _source.unit_ids(at_sq)
	var focus := at_men.duplicate()
	focus.append(tanks[0])
	focus.sort()
	var with_rifles := focus.duplicate()
	with_rifles.append_array(_source.unit_ids(rifles[0]))
	_sel.select_only(with_rifles)
	_intents.clear()
	if ie >= 0:
		_rclick(_view.veh_pick_screen[ie])
	var au: Variant = _last(&"attack_unit")
	_check(ie >= 0 and au != null and au.units == focus and int(au.target) == enemy, "right-click an enemy tank: attack_unit (tank + AT men)")
	_check(_last(&"attack_move") != null and _last(&"attack_move").squads == PackedInt32Array([rifles[0]]), "the rifles attack-move to it")
	# Infantry on an own tank: escort.
	await _frames(1)
	_sel.select_squads(PackedInt32Array([rifles[0]]))
	_intents.clear()
	var i1: int = _view.veh_pick_ids.find(tanks[1])
	_rclick(_view.veh_pick_screen[i1])
	var fu: Variant = _last(&"follow_units")
	_check(fu != null and int(fu.vehicle) == tanks[1] and fu.units == _source.unit_ids(rifles[0]) and _last(&"move") == null, "infantry right-click an own tank: follow_units")
	await _frames(2)
	_check(int(_vstate(tanks[1]).get("followers", 0)) > 0 and int(_view.squads_by_id[rifles[0]].get("follow", -1)) == tanks[1], "mock: escort counted, the squad reports `follow`")
	# Keys: O face tool, N cycles ammo, J repair.
	_sel.select_only(PackedInt32Array([tanks[0]]))
	_key(KEY_O)
	_check(_sel.tool == &"face", "O arms the face tool")
	_intents.clear()
	_click(g)
	_check(_last(&"face_units") != null and _last(&"face_units").units == PackedInt32Array([tanks[0]]) and _sel.tool == &"", "click: face_units")
	await _frames(1)
	var before: String = _vstate(tanks[0]).ammo_choice
	_key(KEY_N)
	var a1: String = _last(&"set_ammo").ammo
	_key(KEY_N)
	var a2: String = _last(&"set_ammo").ammo
	_check(a1 == Vehicles.next_ammo(before) and a2 == Vehicles.next_ammo(a1), "N twice in one frame: %s -> %s -> %s" % [before, a1, a2])
	_key(KEY_J)
	_check(_last(&"repair_units") != null and _last(&"repair_units").units == PackedInt32Array([tanks[0]]), "J: repair_units")
	# S3b: P arms the suppress tool; a drag sends a line capped at
	# rules.suppress_line_max_m, a click a point.
	_key(KEY_P)
	_check(_sel.tool == &"suppress", "P arms the suppress tool")
	_check(_view.orders.button_rect(&"suppress").size.x > 0.0 and _view.orders.enabled(&"suppress"), "the vehicle grid has a Suppress button, enabled")
	var sp0 := _ground_screen(_view.unit_pos(tanks[0]) + Vector3(-50, 0, -10))
	var sp1 := _ground_screen(_view.unit_pos(tanks[0]) + Vector3(-50, 0, 70))
	_intents.clear()
	_button(sp0, MOUSE_BUTTON_LEFT, true)
	_check(_sel.suppress_from != null, "press: the line starts on the ground")
	_button(sp1, MOUSE_BUTTON_LEFT, false)
	var su: Variant = _last(&"suppress_units")
	var su_ok: bool = su != null and su.has("polyline") and su.polyline.size() == 2 and su.units == PackedInt32Array([tanks[0]])
	if su_ok:
		var pl: PackedVector3Array = su.polyline
		su_ok = Vector2(pl[0].x, pl[0].z).distance_to(Vector2(pl[1].x, pl[1].z)) <= 40.5
	_check(su_ok and _sel.tool == &"", "drag: suppress_units with a 2-point line capped at 40 m, the tool disarms")
	_key(KEY_P)
	_intents.clear()
	_click(sp0)
	su = _last(&"suppress_units")
	_check(su != null and su.has("target") and not su.has("polyline"), "click: suppress_units at a point")
	_sel.select_squads(PackedInt32Array([rifles[0]]))
	await _frames(1)
	_check(_view.orders.button_rect(&"suppress").size.x > 0.0 and _view.orders.enabled(&"suppress"), "the soldier grid has Suppress too")
	_sel.select_only(PackedInt32Array([tanks[0]]))
	# Ammo toggle on the grid; the state follows.
	_view.orders.press(&"ammo_he")
	_check(_last(&"set_ammo").ammo == "he", "ammo strip HE sends set_ammo he")
	await _frames(2)
	_check(_vstate(tanks[0]).ammo_choice == "he" and _view.orders.active(&"ammo_he") and not _view.orders.active(&"ammo_ap"), "the strip shows HE once the state has it")
	# S3b: APCR (the special round) only on a gun that carries it.
	_check(Vehicles.next_ammo("ap", true) == "apcr" and Vehicles.next_ammo("apcr", true) == "he" and Vehicles.next_ammo("ap") == "he", "N cycles Auto, AP, APCR (where carried), HE")
	_check(_view.orders.enabled(&"ammo_apcr") == Vehicles.has_apcr(_vstate(tanks[0])), "the APCR button lights only for a gun that carries it")
	# Roster card: shown, wide enough, a click selects the tank.
	var roster: Control = _view.roster
	var r: Rect2 = roster.vehicle_card_rect(tanks[0])
	_check(r.size.x > 0.0 and r.size.x + 0.5 >= roster.vehicle_card_width(tanks[0], roster.get_theme_default_font(), HudStyle.bold()), "the selected tank has a card wide enough for its text (%.0f px)" % r.size.x)
	_sel.select_only(PackedInt32Array())
	_gui_click(roster, r.get_center())
	_check(_sel.selected == PackedInt32Array([tanks[0]]) and _sel.focused_vehicle() == tanks[0], "card click selects the tank and focuses it")
	# Vehicle limit: 4 alive.
	await _frames(1)
	tile = _view.state.callin.catalog[_vehicle_tile()]
	_check(ci.blocked_reason(tile) == "limit" and int(_view.state.callin.vehicles) == 4, "4 tanks: the tile says limit (4/4)")
	# Feed and hit markers.
	_view.event_feed.push({"kind": "vehicle_knocked_out", "text": "Enemy Sherman knocked out.", "player": 1})
	_check(_view.event_feed.lines[0].get("good", false), "enemy knock-out: own-colour stripe")
	var evs: Array = []
	for k in 8:
		evs.append({"kind": "pen" if k % 2 == 0 else "no_pen", "pos": _view.unit_pos(enemy), "owner": 1})
	_view._note_hits(evs)
	_check(_view.hit_markers.size() == _view.HIT_MAX and _view.hit_markers[-1].word == "No pen", "hit markers: at most 6, newest kept")
	# Shells: a volley shows tracers.
	_source.veh.volley(0)
	await _frames(1)
	_check(_match.vehicle_fx.counts.streaks >= 1, "shells in flight draw tracers (%d)" % _match.vehicle_fx.counts.streaks)
	await _test_vehicle_cost()
	await _test_fit_1280(tanks)
	_sel.select_only(PackedInt32Array())
	_sel.cancel_tool()


## Done-when 9: CPU time to build the vehicle buffers, the picking index,
## one pick and the vehicle rings for 16 vehicles, under 0.2 ms.
func _test_vehicle_cost() -> void:
	var s: Dictionary = _view.state.duplicate()
	var list: Array = []
	var f: Vector3 = _cam.focus
	var keys := ["ger_pz4_h", "usa_m4a3_75"]
	for i in 16:
		list.append({
			"id": 9000 + i, "squad": 900 + i, "owner": i % 2, "type": keys[i % 2], "name": "T",
			"pos": f + Vector3((i % 4) * 12.0 - 18.0, 0, floorf(i / 4.0) * 12.0 - 18.0),
			"yaw": i * 0.4, "pitch": 0.02, "roll": -0.01, "turret_yaw": i * 0.3, "gun_pitch": 0.05,
			"speed": 3.0, "flags": 1, "state": "moving",
		})
	s.vehicles = list
	var vv: VehicleView = _match.vehicle_view
	var rings: SelectionRings = _match.get_node("GroundOverlay/VehicleRings")
	var pts := PackedVector3Array()
	var yaws := PackedFloat32Array()
	var half := PackedVector2Array()
	var cols := PackedColorArray()
	var rect: Rect2 = _view.get_viewport().get_visible_rect()
	var n := 100
	var t_buf := 0
	var t_pick := 0
	var t_ring := 0
	for k in n:
		var t0 := Time.get_ticks_usec()
		vv.update_state(s)
		var t1 := Time.get_ticks_usec()
		_view._index_vehicles(s, rect)
		_sel.pick_vehicle(Vector2(800, 450), -1)
		var t2 := Time.get_ticks_usec()
		pts.clear()
		yaws.clear()
		half.clear()
		cols.clear()
		for v in list:
			pts.append(v.pos)
			yaws.append(v.yaw)
			half.append(Vector2(2.3, 3.8))
			cols.append(Color.WHITE)
		rings.set_oriented(pts, yaws, half, cols)
		var t3 := Time.get_ticks_usec()
		t_buf += t1 - t0
		t_pick += t2 - t1
		t_ring += t3 - t2
	var per := (t_buf + t_pick + t_ring) / float(n) / 1000.0
	print("cost: 16 vehicles: buffers %.3f + picking %.3f + rings %.3f = %.3f ms/frame" % [t_buf / float(n) / 1000.0, t_pick / float(n) / 1000.0, t_ring / float(n) / 1000.0, per])
	_check(vv.counts.get("ger_pz4_h", 0) == 8 and _view.veh_pick_ids.size() > 8, "16-vehicle state drawn and indexed")
	_check(per < 0.2, "16 vehicles: buffers + picking + rings under 0.2 ms (%.3f ms, headless debug)" % per)
	_view.update_state(_source.get_state())
	await _frames(1)


## Done-when 10: at 1280x800 with 4 vehicles and 8 squads no roster cards
## overlap; prints the visible card count.
func _test_fit_1280(tanks: PackedInt32Array) -> void:
	var hud: Control = _view.hud
	var own: PackedInt32Array = _source.squad_ids(0)
	var k := 0
	while own.size() < 8:
		_source._spawn(0, "rifle", Vector2(-300 + k * 12.0, 300))
		own = _source.squad_ids(0)
		k += 1
	while own.size() > 8:
		for man in _source.unit_ids(own[own.size() - 1]):
			_source.kill_soldier(man)
		own = _source.squad_ids(0)
	hud.force_size = Vector2(1280, 800)
	_sel.select_only(PackedInt32Array())
	await _frames(3)
	var roster: Control = _view.roster
	var ok := true
	for i in roster.cards.size():
		var a: Rect2 = roster.cards[i].rect
		ok = ok and a.position.x >= -0.5 and a.end.x <= roster.size.x + 0.5 and a.end.y <= roster.size.y + 0.5
		for j in range(i + 1, roster.cards.size()):
			ok = ok and not a.intersects(roster.cards[j].rect)
	var vehicles_shown := 0
	for c in roster.cards:
		if c.vehicle != -1:
			vehicles_shown += 1
	print("fit 1280x800: %d of %d cards shown (%d vehicle cards), +%d tile" % [roster.cards.size(), roster.cards.size() + roster.hidden_count, vehicles_shown, roster.hidden_count])
	_check(own.size() == 8 and tanks.size() == 4 and roster.cards.size() + roster.hidden_count == 12, "1280x800: 8 squads + 4 vehicles in the roster")
	_check(ok, "1280x800: no roster cards overlap or leave the console (%d shown)" % roster.cards.size())
	_check(vehicles_shown == mini(tanks.size(), roster.cards.size()), "1280x800: with nothing selected the vehicle cards are kept first (%d shown)" % vehicles_shown)
	# Selected tanks keep their cards.
	_sel.select_only(tanks)
	await _frames(1)
	var all_shown := true
	for t in tanks:
		all_shown = all_shown and roster.vehicle_card_rect(t).size.x > 0.0
	_check(all_shown, "1280x800: all four selected tanks keep their cards")
	hud.force_size = Vector2.ZERO
	hud.relayout()
	await _frames(1)


## S1b budget: picking 240 soldiers and their rings under 0.3 ms per frame
## on the M3. Measured here on a synthetic 240-man state (index + project +
## one pick + one ring upload); the check is loose (headless, debug build),
## the number is printed for the report.
func _test_cost() -> void:
	var s: Dictionary = _view.state.duplicate()
	var list: Array = []
	var c: Vector3 = _cam.focus
	for i in 240:
		list.append({"id": 5000 + i, "squad": 1, "owner": 0, "slot": i % 8, "pos": c + Vector3((i % 20) * 2.0 - 20.0, 0, floorf(i / 20.0) * 2.0 - 12.0), "hp": 100.0, "stance": "crouch"})
	s.soldiers = list
	var ids := PackedInt32Array()
	for so in list:
		ids.append(so.id)
	var rings: SelectionRings = _match.get_node("GroundOverlay/SelectionRings")
	var pts := PackedVector3Array()
	var cols := PackedColorArray()
	for so in list:
		pts.append(so.pos)
		cols.append(Color.WHITE)
	var t0 := Time.get_ticks_usec()
	var n := 50
	for k in n:
		_view.state = s
		_view._index_soldiers(s, _view.get_viewport().get_visible_rect())
		_sel.pick(Vector2(800, 450), -1)
		rings.set_rings(pts, cols)
	var per := (Time.get_ticks_usec() - t0) / float(n) / 1000.0
	print("cost: pick index + rings for 240 soldiers: %.3f ms/frame" % per)
	_check(_view.pick_ids.size() > 200 and per < 2.0, "240-soldier picking + rings stay cheap (%.3f ms, headless)" % per)
	_view.update_state(_source.get_state())


## The call-in "need N" reasons: steady over many frames at a constant
## manpower, N always the shortfall, on every tile including the dearest,
## and following manpower as it rises (the owner saw "need N" blink, and
## none on the Mortar tile: the fit was judged on proportional digits).
func _test_callin_need() -> void:
	var ci: Control = _view.call_in
	# Live mock frames at a constant manpower.
	var me: Dictionary = _source.players[_view.local_player]
	var saved_mp: float = me.manpower
	me.manpower = 58.0
	await _frames(2)
	var first: Array = _reasons(ci)
	var steady := true
	for f in 60:
		await _frames(1)
		steady = steady and _reasons(ci) == first
	var cat: Array = _view.state.callin.catalog
	var right := true
	for i in cat.size():
		if not ci.is_vehicle(cat[i]) and float(cat[i].get("cooldown_s", 0.0)) <= 0.0 and int(cat[i].cost) > 58:
			right = right and first[i] == "need %d" % (int(cat[i].cost) - 58)
	_check(steady, "call-in reasons steady over 60 frames at 58 MP %s" % str(first))
	_check(right, "every squad tile says need <cost - 58> at 58 MP")
	me.manpower = saved_mp

	# The owner's frame: the S2/S3 costs at 58 MP, fed 120 times.
	var s: Dictionary = _view.state.duplicate(true)
	s.callin.catalog = [
		{"type": "rifle", "id": "ww2:usa_rifle", "name": "Rifle squad", "cost": 250, "recall_s": 24.0, "men": 8, "pop": 8},
		{"type": "mg", "id": "ww2:usa_mg", "name": "MG team", "cost": 190, "recall_s": 36.0, "men": 3, "pop": 3},
		{"type": "engineer", "id": "ww2:usa_engineer", "name": "Engineer squad", "cost": 220, "recall_s": 36.0, "men": 5, "pop": 5},
		{"type": "mortar", "id": "ww2:usa_mortar", "name": "Mortar team", "cost": 280, "recall_s": 60.0, "men": 3, "pop": 3},
	]
	s.players[_view.local_player].pop = 0
	s.players[_view.local_player].manpower = 58.0
	var want := ["need 192", "need 132", "need 162", "need 222"]
	steady = true
	for f in 120:
		_view.update_state(s)
		steady = steady and _reasons(ci) == want
	_check(steady, "owner's frame: need 192 / 132 / 162 / 222 on all four tiles, 120 updates (%s)" % str(_reasons(ci)))
	# Rising manpower: every update shows exactly the shortfall until the
	# tile is affordable, then nothing.
	var bad := ""
	var mp := 0.0
	while mp <= 300.0 and bad == "":
		s.players[_view.local_player].manpower = mp
		_view.update_state(s)
		var got: Array = _reasons(ci)
		for i in s.callin.catalog.size():
			var short: int = int(s.callin.catalog[i].cost) - int(mp)
			var expect := "need %d" % short if short > 0 else ""
			if got[i] != expect:
				bad = "(%.2f MP tile %d: %s, want %s)" % [mp, i, got[i], expect]
		mp += 0.25
	_check(bad == "", "rising 0 → 300 MP: need N tracks the shortfall on every tile %s" % bad)
	_view.update_state(_source.get_state())
	await _frames(2)


func _reasons(ci: Control) -> Array:
	var out: Array = []
	for i in _view.state.callin.catalog.size():
		out.append(ci.reason_text(i))
	return out


func _test_end() -> void:
	# S3b debug data reload: F5 asks for it (debug builds only), the pause
	# menu offers it, and a refused reload shows a readable banner.
	var debug := OS.is_debug_build()
	_key(KEY_F5)
	_check((_last(&"reload_data") != null) == debug, "F5 asks to reload the data in debug builds only")
	_view.pause_menu.open()
	_check(_view.pause_menu.reload_button.visible == debug, "the pause menu offers Reload data & restart in debug builds")
	_view.pause_menu.close()
	_view.data_error.show_error("defs/rules/skirmish.toml: combat.near_miss_cm: must be in 0..=2000, got 9000")
	_check(_view.data_error.visible and _view.data_error._text.text.contains("near_miss_cm"), "a refused reload shows the loader's message")
	_view.data_error.hide_error()
	_check(not _view.data_error.visible, "the banner dismisses")
	# Pause via Esc, resume via Esc.
	_key(KEY_ESCAPE)
	_check(_view.pause_menu.visible and _last(&"pause") != null and _last(&"pause").paused, "Esc opens the menu and pauses")
	_key(KEY_ESCAPE)
	_check(not _view.pause_menu.visible and not _last(&"pause").paused, "Esc again resumes")
	# Surrender → result lost → end screen.
	_view.emit_intent(&"surrender", {})
	await _frames(3)
	_check(_view.end_screen.visible and _view.ended, "surrender shows the end screen")
	_check(_view.state.result.status == "lost", "surrender is a loss")


# --- helpers -------------------------------------------------------------------

func _frames(n: int) -> void:
	for i in n:
		await process_frame


func _check(ok: bool, what: String) -> void:
	_checks += 1
	if not ok:
		_fails += 1
		printerr("FAIL: " + what)
	else:
		print("ok: " + what)


func _last(kind: StringName) -> Variant:
	for i in range(_intents.size() - 1, -1, -1):
		if _intents[i][0] == kind:
			return _intents[i][1]
	return null


## Screen position the view picks a soldier at.
func _screen(id: int) -> Vector2:
	var i: int = _view.pick_ids.find(id)
	if i < 0:
		printerr("soldier %d not on screen" % id)
		return Vector2(-100, -100)
	return _view.pick_screen[i]


func _ground_screen(p: Vector3) -> Vector2:
	p.y = _source.height_at(p.x, p.z)
	return _cam.unproject_position(p)


func _button(pos: Vector2, index: int, pressed: bool, shift := false, ctrl := false, double := false) -> void:
	var e := InputEventMouseButton.new()
	e.button_index = index
	e.pressed = pressed
	e.position = pos
	e.shift_pressed = shift
	e.ctrl_pressed = ctrl
	e.double_click = double
	_sel._unhandled_input(e)


func _click(pos: Vector2, shift := false) -> void:
	_button(pos, MOUSE_BUTTON_LEFT, true, shift)
	_button(pos, MOUSE_BUTTON_LEFT, false, shift)


## The second click of a double-click (the first one already selected).
func _double(pos: Vector2, ctrl := false, shift := false) -> void:
	_button(pos, MOUSE_BUTTON_LEFT, true, shift, ctrl)
	_button(pos, MOUSE_BUTTON_LEFT, false, shift, ctrl)
	_button(pos, MOUSE_BUTTON_LEFT, true, shift, ctrl, true)
	_button(pos, MOUSE_BUTTON_LEFT, false, shift, ctrl)


func _rclick(pos: Vector2, ctrl := false) -> void:
	_button(pos, MOUSE_BUTTON_RIGHT, true, false, ctrl)
	_button(pos, MOUSE_BUTTON_RIGHT, false, false, ctrl)


func _drag(a: Vector2, b: Vector2, shift := false) -> void:
	_button(a, MOUSE_BUTTON_LEFT, true, shift)
	var m := InputEventMouseMotion.new()
	m.position = b
	_sel._unhandled_input(m)
	_button(b, MOUSE_BUTTON_LEFT, false, shift)


func _gui_click(c: Control, local: Vector2, shift := false) -> void:
	var e := InputEventMouseButton.new()
	e.button_index = MOUSE_BUTTON_LEFT
	e.pressed = true
	e.position = local
	e.shift_pressed = shift
	c._gui_input(e)


func _mm_click(mm: Control, local: Vector2) -> void:
	_gui_click(mm, local)
	var up := InputEventMouseButton.new()
	up.button_index = MOUSE_BUTTON_LEFT
	up.pressed = false
	up.position = local
	mm._gui_input(up)


## A key press straight to the focused call-in column (its own input).
func _panel_key(code: int) -> void:
	var e := InputEventKey.new()
	e.physical_keycode = code as Key
	e.keycode = code as Key
	e.pressed = true
	_view.call_in._gui_input(e)


func _key(code: Key, ctrl := false, shift := false) -> void:
	var e := InputEventKey.new()
	e.physical_keycode = code
	e.keycode = code
	e.pressed = true
	e.ctrl_pressed = ctrl
	e.shift_pressed = shift
	_sel._unhandled_input(e)
