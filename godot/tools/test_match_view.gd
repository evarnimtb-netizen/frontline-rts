extends SceneTree
## Headless test of MatchView input and intents on the mock source: box
## and click selection, groups, double-click, order tools, the dig tool's
## length cap, the call-in panel, pause and surrender → end screen.
##
##   Godot --headless --path godot -s res://tools/test_match_view.gd
##
## Exits 0 when every check passes. Events are fed straight to the
## selection controller (no OS input needed).

var _match: Node
## Untyped on purpose: a static MatchView reference would compile the view
## before the autoloads exist (-s scripts load first).
var _view: Node
var _sel: Node
var _intents: Array = []
var _fails := 0
var _checks := 0


func _initialize() -> void:
	_run.call_deferred()
	# Guard: a script error inside _run would otherwise hang the run.
	create_timer(120.0).timeout.connect(func() -> void:
		printerr("test_match_view: timed out")
		quit(2))


func _run() -> void:
	_match = load("res://match/match.tscn").instantiate()
	root.add_child(_match)
	await _frames(5)
	_view = _match.view
	_sel = _view.selection
	_view.intent.connect(func(k: StringName, d: Dictionary) -> void: _intents.append([k, d]))
	var source: Node = _match.source
	var cam: Camera3D = _match.camera
	# Look at our engineers and the rifle squad next to them.
	var own: PackedInt32Array = source.squad_ids(0)
	var eng: int = source.squad_ids(0, "engineer")[0]
	var mortar: int = source.squad_ids(0, "mortar")[0]
	cam.height = 120.0
	cam.jump_to(source.squad_pos(eng))
	await _frames(3)

	# Click on a squad selects it; click on empty ground clears.
	var sp: Vector2 = _view.squad_screen[eng]
	_click(sp)
	_check(_sel.selected == PackedInt32Array([eng]), "click selects the engineer")
	_check(_last(&"select") != null, "select intent emitted")
	_click(Vector2(5, 450))
	_check(_sel.selected.is_empty(), "click on empty ground clears")

	# Box select everything on screen that is ours.
	_drag(Vector2(2, 60), Vector2(1598, 898))
	var on_screen := PackedInt32Array()
	for id in _view.squad_screen:
		if _view.own_squad(id) and Rect2(2, 60, 1596, 838).has_point(_view.squad_screen[id]):
			on_screen.append(id)
	on_screen.sort()
	_check(not on_screen.is_empty() and _sel.selected == on_screen, "box select picks own squads on screen")

	# Groups: Ctrl+1 stores, clear, 1 recalls.
	var stored: PackedInt32Array = _sel.selected.duplicate()
	_key(KEY_1, true)
	_click(Vector2(5, 450))
	_key(KEY_1)
	_check(_sel.selected == stored, "Ctrl+1 / 1 group recall")

	# Shift-click toggles one squad out.
	_click(_view.squad_screen[eng], true)
	_check(not _sel.selected.has(eng), "shift-click removes from selection")

	# Attack-move tool: A then left click.
	_sel.select_only(PackedInt32Array([eng]))
	_intents.clear()
	_key(KEY_A)
	_check(_sel.tool == &"attack_move", "A arms attack-move")
	_click(Vector2(800, 400))
	var am: Variant = _last(&"attack_move")
	_check(am != null and am.squads == PackedInt32Array([eng]) and am.target is Vector3, "attack_move intent with target")
	_check(_sel.tool == &"", "tool clears after use")

	# Stop / retreat are instant.
	_key(KEY_S)
	_check(_last(&"stop") != null, "S emits stop")
	_key(KEY_R)
	_check(_last(&"retreat") != null, "R emits retreat")

	# Dig tool: points along the ground, capped at 60 m, right-click digs.
	_key(KEY_D)
	_check(_sel.tool == &"dig", "D arms the dig tool")
	for x in [300, 500, 700, 900, 1100, 1300, 1500]:
		_click(Vector2(x, 450))
	_check(_sel.dig_length() <= 60.01, "dig polyline capped at 60 m (%.1f)" % _sel.dig_length())
	_check(_sel.dig_points.size() >= 2, "dig points placed")
	_key(KEY_BACKSPACE)
	var n_after_undo: int = _sel.dig_points.size()
	_rclick(Vector2(800, 450))
	var dig: Variant = _last(&"dig")
	_check(dig != null and dig.polyline.size() == n_after_undo and dig.squads == PackedInt32Array([eng]), "right-click emits dig with the polyline")

	# Dig needs a digger: mortar alone cannot.
	_sel.select_only(PackedInt32Array([mortar]))
	_key(KEY_D)
	_check(_sel.tool == &"", "dig refused for a mortar team")
	_key(KEY_B)
	_check(_sel.tool == &"barrage", "B arms barrage for mortars")
	_key(KEY_ESCAPE)
	_check(_sel.tool == &"" and not _view.pause_menu.visible, "Esc cancels the tool first")

	# Right-click with mortar selected and target in range → barrage.
	_intents.clear()
	var mp: Vector3 = source.squad_pos(mortar)
	cam.jump_to(mp + Vector3(0, 0, -150))
	await _frames(2)
	var tgt := cam.unproject_position(mp + Vector3(0, 0, -150))
	_rclick(tgt)
	_check(_last(&"barrage") != null, "right-click in range gives barrage")

	# Double-click selects every own squad of that type on screen.
	cam.height = 150.0
	cam.jump_to(source.squad_pos(own[0]))
	await _frames(2)
	var rifle: int = source.squad_ids(0, "rifle")[0]
	if _view.squad_screen.has(rifle):
		var dc := InputEventMouseButton.new()
		dc.button_index = MOUSE_BUTTON_LEFT
		dc.pressed = true
		dc.double_click = true
		dc.position = _view.squad_screen[rifle]
		_sel._unhandled_input(dc)
		var all_rifles := true
		for id in _sel.selected:
			all_rifles = all_rifles and _view.squads_by_id[id].type == "rifle"
		_check(_sel.selected.has(rifle) and all_rifles, "double-click selects same type")

	# Call-in through the panel button.
	_intents.clear()
	var btn: Button = _view.call_in._buttons["rifle"]
	btn.pressed.emit()
	await _frames(2)
	_check(_last(&"call_in") != null and _last(&"call_in").type == "rifle", "call-in intent")
	await _frames(2)
	_check(_view.state.callin.queue.size() >= 1, "mock queued the call-in")

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

	print("test_match_view: %d/%d checks passed" % [_checks - _fails, _checks])
	quit(0 if _fails == 0 else 1)


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


func _button(pos: Vector2, index: int, pressed: bool, shift := false) -> void:
	var e := InputEventMouseButton.new()
	e.button_index = index
	e.pressed = pressed
	e.position = pos
	e.shift_pressed = shift
	_sel._unhandled_input(e)


func _click(pos: Vector2, shift := false) -> void:
	_button(pos, MOUSE_BUTTON_LEFT, true, shift)
	_button(pos, MOUSE_BUTTON_LEFT, false, shift)


func _rclick(pos: Vector2) -> void:
	_button(pos, MOUSE_BUTTON_RIGHT, true)
	_button(pos, MOUSE_BUTTON_RIGHT, false)


func _drag(a: Vector2, b: Vector2) -> void:
	_button(a, MOUSE_BUTTON_LEFT, true)
	var m := InputEventMouseMotion.new()
	m.position = b
	_sel._unhandled_input(m)
	_button(b, MOUSE_BUTTON_LEFT, false)


func _key(code: Key, ctrl := false) -> void:
	var e := InputEventKey.new()
	e.physical_keycode = code
	e.keycode = code
	e.pressed = true
	e.ctrl_pressed = ctrl
	_sel._unhandled_input(e)
