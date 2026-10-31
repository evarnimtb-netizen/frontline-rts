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
	var saved := [_view.pick_ids, _view.pick_screen, _view.pick_depth, _view.pick_own]
	_view.pick_ids = PackedInt32Array([eng_men[0], eng_men[1]])
	_view.pick_screen = PackedVector2Array([Vector2(500, 500), Vector2(500.4, 500)])
	_view.pick_depth = PackedFloat32Array([90.0, 60.0])
	_view.pick_own = PackedByteArray([1, 1])
	_check(_sel.pick(Vector2(500, 500)) == eng_men[1], "near-tie goes to the man closer to the camera")
	_view.pick_screen = PackedVector2Array([Vector2(500, 500), Vector2(508, 500)])
	_check(_sel.pick(Vector2(500, 500)) == eng_men[0], "clearly nearer on screen wins over depth")
	_check(_sel.pick(Vector2(530, 500)) == -1, "nothing beyond 14 px")
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
	_check(absf(hs - mm.size.x * 0.5 / sqrt(2.0)) < 0.5, "rotated: whole map fits the disc (side = diameter / sqrt 2)")
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
	_check(not mm._inside(Vector2(1, 1)), "rotated: corners outside the disc pass through")
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
	# Call-in: tile click → intent; Tab focuses the column.
	_intents.clear()
	_view.call_in.call_in_index(_view.call_in.index_of("rifle"))
	await _frames(2)
	_check(_last(&"call_in") != null and _last(&"call_in").type == "rifle", "call-in intent")
	_check(_view.state.callin.queue.size() >= 1, "mock queued the call-in")
	_key(KEY_TAB)
	_check(_view.call_in.focused, "Tab focuses the call-in column")
	_view.call_in.toggle_focus()
	_check(not _view.call_in.focused, "focus leaves the column again")
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
	_check(absf(ci.size.x - 120.0) < 0.5 and absf(ci.end.x - (sz.x - 12.0)) < 0.5 and ci.end.y <= top - 8.0 + 0.5, "%s: call-in column 120 px, right, above the orders" % label)
	var fe: Rect2 = hud.rect_of(_view.event_feed)
	_check(absf(fe.end.x - (sz.x - 12.0)) < 0.5 and fe.position.y == tb.size.y + 8.0 and not fe.intersects(mm), "%s: feed top-right" % label)
	_check(ci.position.y > fe.end.y and not ci.intersects(orr), "%s: no overlaps on the right" % label)


func _test_end() -> void:
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


func _key(code: Key, ctrl := false, shift := false) -> void:
	var e := InputEventKey.new()
	e.physical_keycode = code
	e.keycode = code
	e.pressed = true
	e.ctrl_pressed = ctrl
	e.shift_pressed = shift
	_sel._unhandled_input(e)
