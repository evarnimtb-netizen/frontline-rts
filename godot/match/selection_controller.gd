extends Node
## Selection and order input for the match (S1). Works on the screen
## positions MatchView keeps for each squad (state "screen", or projected
## from "pos"), so it needs no physics and no Node per unit.
##
## Mouse: left click selects (Shift toggles), drag box-selects (Shift adds),
## double-click selects every own squad of that type on screen. Right-click
## orders: move; on a spotted enemy, attack-move to it; with mortars
## selected and the target inside their range, the mortars barrage it and
## the rest move.
## Keys (MatchInput): M move, A attack-move, B barrage (armed tools: left
## click places, right-click or Esc cancels), S stop, R retreat, D dig tool
## (left click adds points along the ground, Backspace removes the last,
## right-click confirms, total length capped at rules.dig_max_m),
## Ctrl+1–9 store a group, 1–9 recall it (twice quickly: centre on it),
## Space jumps to the latest event, Esc opens the match menu.

signal tool_changed(tool: StringName)

const CLICK_RADIUS_PX := 20.0
const DRAG_THRESHOLD_PX := 6.0
const RECALL_DOUBLE_MS := 350

var view: MatchView
## Selected own squad ids, ascending.
var selected := PackedInt32Array()
var groups: Array[PackedInt32Array] = []
## &"", &"move", &"attack_move", &"barrage" or &"dig".
var tool: StringName = &""
var dig_points := PackedVector3Array()
## Box drag in screen space while dragging.
var dragging := false
var drag_from := Vector2.ZERO
var drag_to := Vector2.ZERO
var hover_id := -1
## Screen point used instead of the mouse by tool previews (screenshots,
## autoplay); null = the real mouse.
var cursor_override: Variant = null

var _press_pos := Vector2.ZERO
var _pressing := false
var _skip_release := false
var _last_recall_group := 0
var _last_recall_ms := 0


func bind(v: MatchView) -> void:
	view = v
	groups.resize(10)
	for i in 10:
		groups[i] = PackedInt32Array()


## Drops ids that are gone or no longer ours (called by MatchView each frame).
func prune() -> void:
	var kept := PackedInt32Array()
	for id in selected:
		if view.own_squad(id):
			kept.append(id)
	if kept.size() != selected.size():
		_set_selection(kept)
	if tool == &"dig" and not _has_type(MatchView.DIGGERS):
		cancel_tool()
	elif tool == &"barrage" and not _has_type(MatchView.MORTARS):
		cancel_tool()


func cursor() -> Vector2:
	if cursor_override != null:
		return cursor_override
	return view.get_viewport().get_mouse_position()


func is_selected(id: int) -> bool:
	return selected.has(id)


func select_only(ids: PackedInt32Array) -> void:
	_set_selection(ids)


func deselect(id: int) -> void:
	var ids := selected.duplicate()
	var i := ids.find(id)
	if i >= 0:
		ids.remove_at(i)
		_set_selection(ids)


func _set_selection(ids: PackedInt32Array) -> void:
	var s := ids.duplicate()
	s.sort()
	# Dedupe (sorted).
	var out := PackedInt32Array()
	for id in s:
		if out.is_empty() or out[out.size() - 1] != id:
			out.append(id)
	if out == selected:
		return
	selected = out
	view.emit_intent(&"select", {"squads": selected})


func _has_type(types: Array) -> bool:
	for id in selected:
		var sq: Dictionary = view.squads_by_id.get(id, {})
		if not sq.is_empty() and sq.type in types:
			return true
	return false


func _ids_of_type(types: Array, want: bool) -> PackedInt32Array:
	var out := PackedInt32Array()
	for id in selected:
		var sq: Dictionary = view.squads_by_id.get(id, {})
		if not sq.is_empty() and (sq.type in types) == want:
			out.append(id)
	return out


## Whether an order is available for the current selection (CommandPanel).
func can(order: StringName) -> bool:
	if selected.is_empty():
		return false
	match order:
		&"dig":
			return _has_type(MatchView.DIGGERS)
		&"barrage":
			return _has_type(MatchView.MORTARS)
	return true


# --- tools -----------------------------------------------------------------

## Starts an order tool or issues an instant order (stop, retreat).
func activate(order: StringName) -> void:
	if view.input_blocked() or not can(order):
		if not selected.is_empty():
			AudioManager.play_ui(&"error")
		return
	match order:
		&"stop":
			view.emit_intent(&"stop", {"squads": selected})
			cancel_tool()
		&"retreat":
			view.emit_intent(&"retreat", {"squads": selected})
			cancel_tool()
		&"dig":
			_set_tool(&"dig")
			dig_points.clear()
		_:
			_set_tool(order)


func cancel_tool() -> void:
	dig_points.clear()
	_set_tool(&"")


func _set_tool(t: StringName) -> void:
	if t == tool:
		return
	tool = t
	tool_changed.emit(t)


func dig_length() -> float:
	var n := 0.0
	for i in range(1, dig_points.size()):
		n += dig_points[i - 1].distance_to(dig_points[i])
	return n


func dig_max() -> float:
	return float(view.rules().get("dig_max_m", 60.0))


## Next dig point for a cursor position, shortened so the polyline stays
## within the length cap. Null when the cursor is off the ground.
func dig_candidate(screen: Vector2) -> Variant:
	var g: Variant = view.ground_pick(screen)
	if g == null or dig_points.is_empty():
		return g
	var last := dig_points[dig_points.size() - 1]
	var left := dig_max() - dig_length()
	var p: Vector3 = g
	if last.distance_to(p) > left:
		p = last + (p - last).normalized() * maxf(left, 0.0)
		p.y = view.ground_y(p.x, p.z)
	return p


func _confirm_dig() -> void:
	if dig_points.size() < 2 or dig_length() < 2.0:
		AudioManager.play_ui(&"error")
		return
	var ids := _ids_of_type(MatchView.DIGGERS, true)
	view.emit_intent(&"dig", {"squads": ids, "polyline": dig_points.duplicate()})
	if view.overlay != null:
		view.overlay.flash_polyline(dig_points)
	AudioManager.play_ui(&"confirm")
	cancel_tool()


# --- picking ---------------------------------------------------------------

## Squad under a screen point: nearest anchor within the click radius.
## `own` = 1 own only, 0 enemies only, -1 any.
func pick(screen: Vector2, own := 1) -> int:
	var best := -1
	var best_d := CLICK_RADIUS_PX
	for id in view.squad_screen:
		if own != -1 and view.own_squad(id) != (own == 1):
			continue
		var d: float = screen.distance_to(view.squad_screen[id])
		if d < best_d or (d == best_d and id < best):
			best_d = d
			best = id
	return best


func _box_select(a: Vector2, b: Vector2, add: bool) -> void:
	var r := Rect2(a, Vector2.ZERO).expand(b)
	var ids := selected.duplicate() if add else PackedInt32Array()
	for id in view.squad_screen:
		if view.own_squad(id) and r.has_point(view.squad_screen[id]):
			ids.append(id)
	_set_selection(ids)


func _select_same_type(id: int) -> void:
	var t: String = view.squads_by_id[id].type
	var ids := PackedInt32Array()
	var vp := view.get_viewport().get_visible_rect()
	for other in view.squad_screen:
		if view.own_squad(other) and view.squads_by_id[other].type == t and vp.has_point(view.squad_screen[other]):
			ids.append(other)
	_set_selection(ids)


# --- orders ----------------------------------------------------------------

func _order_at(screen: Vector2) -> void:
	if selected.is_empty():
		return
	var enemy := pick(screen, 0)
	if enemy != -1:
		view.emit_intent(&"attack_move", {"squads": selected, "target": view.squads_by_id[enemy].pos})
		if view.overlay != null:
			view.overlay.marker(view.squads_by_id[enemy].pos, &"attack_move")
		return
	var g: Variant = view.ground_pick(screen)
	if g == null:
		return
	order_to_ground(g)


## Right-click semantics on a ground point (also used by the minimap).
func order_to_ground(g: Vector3) -> void:
	if selected.is_empty():
		return
	var mortars := _mortars_in_range(g)
	var movers := PackedInt32Array()
	for id in selected:
		if not mortars.has(id):
			movers.append(id)
	if not mortars.is_empty():
		view.emit_intent(&"barrage", {"squads": mortars, "target": g})
	if not movers.is_empty():
		view.emit_intent(&"move", {"squads": movers, "target": g})
	if view.overlay != null:
		view.overlay.marker(g, &"barrage" if movers.is_empty() else &"move")


func _mortars_in_range(g: Vector3) -> PackedInt32Array:
	var lo := float(view.rules().get("mortar_min_m", 60.0))
	var hi := float(view.rules().get("mortar_max_m", 600.0))
	var out := PackedInt32Array()
	for id in _ids_of_type(MatchView.MORTARS, true):
		var p: Vector3 = view.squads_by_id[id].pos
		var d := Vector2(p.x, p.z).distance_to(Vector2(g.x, g.z))
		if d >= lo and d <= hi:
			out.append(id)
	return out


func _use_tool_at(screen: Vector2, keep: bool) -> void:
	var g: Variant = view.ground_pick(screen)
	if g == null:
		AudioManager.play_ui(&"error")
		return
	match tool:
		&"move":
			view.emit_intent(&"move", {"squads": selected, "target": g})
		&"attack_move":
			view.emit_intent(&"attack_move", {"squads": selected, "target": g})
		&"barrage":
			var ids := _mortars_in_range(g)
			if ids.is_empty():
				AudioManager.play_ui(&"error")
				return
			view.emit_intent(&"barrage", {"squads": ids, "target": g})
	if view.overlay != null:
		view.overlay.marker(g, tool)
	if not keep:
		_set_tool(&"")


# --- input -----------------------------------------------------------------

func _process(_delta: float) -> void:
	# A release over a HUD panel never reaches _unhandled_input: finish the
	# box here once the button is up.
	if _pressing and not Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT):
		_pressing = false
		_skip_release = false
		if dragging:
			_box_select(drag_from, drag_to, Input.is_key_pressed(KEY_SHIFT))
		dragging = false


func _unhandled_input(event: InputEvent) -> void:
	if view == null:
		return
	if event.is_action_pressed(&"match_menu") or event.is_action_pressed(&"match_menu_alt"):
		if view.ended:
			return
		if tool != &"" and not view.pause_menu.visible and event.is_action_pressed(&"match_menu"):
			cancel_tool()
		else:
			view.pause_menu.toggle()
		get_viewport().set_input_as_handled()
		return
	if view.input_blocked():
		return
	if event is InputEventMouseButton:
		_mouse_button(event)
	elif event is InputEventMouseMotion:
		hover_id = pick(event.position, -1)
		if _pressing and tool == &"":
			drag_to = event.position
			dragging = dragging or _press_pos.distance_to(drag_to) > DRAG_THRESHOLD_PX
	elif event is InputEventKey and event.pressed and not event.echo:
		_key(event)


func _mouse_button(e: InputEventMouseButton) -> void:
	if e.button_index == MOUSE_BUTTON_LEFT:
		if e.pressed:
			if tool == &"dig":
				var p: Variant = dig_candidate(e.position)
				if p != null and (dig_points.is_empty() or dig_length() < dig_max() - 0.5):
					dig_points.append(p)
				get_viewport().set_input_as_handled()
				return
			if tool != &"":
				_use_tool_at(e.position, e.shift_pressed)
				get_viewport().set_input_as_handled()
				return
			if e.double_click:
				var id := pick(e.position, 1)
				if id != -1:
					_select_same_type(id)
					_skip_release = true
					return
			_pressing = true
			_press_pos = e.position
			drag_from = e.position
			drag_to = e.position
			dragging = false
		else:
			if not _pressing:
				_skip_release = false
				return
			_pressing = false
			if _skip_release:
				_skip_release = false
				dragging = false
				return
			if dragging:
				_box_select(drag_from, drag_to, e.shift_pressed)
				dragging = false
				return
			var id := pick(e.position, 1)
			if id == -1:
				if not e.shift_pressed:
					_set_selection(PackedInt32Array())
			elif e.shift_pressed:
				if selected.has(id):
					deselect(id)
				else:
					var ids := selected.duplicate()
					ids.append(id)
					_set_selection(ids)
			else:
				_set_selection(PackedInt32Array([id]))
	elif e.button_index == MOUSE_BUTTON_RIGHT and e.pressed:
		if tool == &"dig":
			_confirm_dig()
		elif tool != &"":
			cancel_tool()
		else:
			_order_at(e.position)
		get_viewport().set_input_as_handled()


func _key(e: InputEventKey) -> void:
	var g := MatchInput.group_of(e)
	if g > 0:
		if e.ctrl_pressed or e.meta_pressed:
			groups[g] = selected.duplicate()
			AudioManager.play_ui(&"confirm")
		else:
			_recall(g)
		get_viewport().set_input_as_handled()
		return
	for order in [&"move", &"attack_move", &"stop", &"retreat", &"dig", &"barrage"]:
		if e.is_action_pressed(StringName("order_" + order)):
			activate(order)
			get_viewport().set_input_as_handled()
			return
	if e.is_action_pressed(&"dig_undo_point") and tool == &"dig" and not dig_points.is_empty():
		dig_points.remove_at(dig_points.size() - 1)
		get_viewport().set_input_as_handled()
	elif e.is_action_pressed(&"jump_to_event"):
		var pos: Variant = view.event_feed.last_position()
		if pos != null:
			view.jump_camera(pos)
		get_viewport().set_input_as_handled()


func _recall(g: int) -> void:
	var ids := PackedInt32Array()
	for id in groups[g]:
		if view.own_squad(id):
			ids.append(id)
	if ids.is_empty():
		return
	var now := Time.get_ticks_msec()
	if g == _last_recall_group and now - _last_recall_ms < RECALL_DOUBLE_MS:
		var c := Vector3.ZERO
		for id in ids:
			c += view.squads_by_id[id].pos
		view.jump_camera(c / ids.size())
	_last_recall_group = g
	_last_recall_ms = now
	_set_selection(ids)
