extends Node
## Selection and order input for the match (S1b: soldier level,
## docs/slices/S1b-soldier-control.md). The selection is a sorted set of
## own soldier ids. Picking works in screen space on the positions MatchView
## projects each frame (pick_ids / pick_screen), so it needs no physics and
## no Node per soldier.
##
## Mouse: click selects one soldier (nearest within PICK_RADIUS_PX; ties go
## to the one closer to the camera), double-click selects his squad,
## Ctrl+double-click every own squad of that type on screen, drag
## box-selects soldiers; Shift adds or toggles (click, box, double-click).
## Right-click orders a move; Ctrl+right-click an attack-move; on a spotted
## enemy, an attack-move to him; mortar squads with a selected man barrage
## a point in their range and the rest move.
##
## Order mapping: a squad whose every living man is selected gets the
## squad-level intent (move, stop, ...); the selected men of partly
## selected squads go into one *_units intent per order. Stance and throws
## always go per soldier; barrage and regroup always per squad.
##
## Keys (MatchInput): H stop, R retreat, L regroup (instant); Y dig tool
## (left click adds points along the ground, Backspace removes the last,
## right-click confirms, length capped at rules.dig_max_m); B barrage tool;
## M move tool; G/T/F throw tool (frag, smoke, satchel: left click throws
## with the nearest eligible man, Shift+click with every eligible man);
## X/C/Z stand/crouch/prone (the active stance again returns to Auto);
## Ctrl+1–9 store a group, 1–9 recall it (twice quickly: centre on it);
## Tab focuses the call-in column; Space jumps to the latest event; Esc
## cancels the tool, else opens the match menu.

signal tool_changed(tool: StringName)

const PICK_RADIUS_PX := 14.0
## Candidates this close in screen distance count as a tie (camera
## distance decides).
const PICK_TIE_PX := 1.0
const DRAG_THRESHOLD_PX := 6.0
const RECALL_DOUBLE_MS := 350
## A stance the player just sent counts as the soldier's override for this
## long, or until the state shows it (the sim applies commands a few ticks
## later).
const STANCE_PENDING_MS := 1500
const STANCE_KEYS := {&"stance_stand": "stand", &"stance_crouch": "crouch", &"stance_prone": "prone"}
const ABILITY_KEYS := {&"ability_frag": "frag", &"ability_smoke": "smoke", &"ability_satchel": "satchel"}

var view: MatchView
## Selected own soldier ids, ascending.
var selected := PackedInt32Array()
var groups: Array[PackedInt32Array] = []
## &"", &"move", &"attack_move", &"barrage", &"dig" or &"throw".
var tool: StringName = &""
## Armed item while tool == &"throw": "frag", "smoke" or "satchel".
var throw_item := ""
var dig_points := PackedVector3Array()
## Box drag in screen space while dragging.
var dragging := false
var drag_from := Vector2.ZERO
var drag_to := Vector2.ZERO
## Soldier under the cursor (either side), or -1.
var hover_id := -1
## Squad the detail panel lists (last card clicked); -1 = first selected.
var focus_squad := -1
## Screen point used instead of the mouse by tool previews (screenshots,
## autoplay); null = the real mouse.
var cursor_override: Variant = null

var _press_pos := Vector2.ZERO
var _pressing := false
var _skip_release := false
var _last_recall_group := 0
var _last_recall_ms := 0
## soldier id -> [stance, msec sent]
var _pending_stance := {}


func bind(v: MatchView) -> void:
	view = v
	groups.resize(10)
	for i in 10:
		groups[i] = PackedInt32Array()


## Drops ids that are gone or no longer ours (called by MatchView each frame).
func prune() -> void:
	var kept := PackedInt32Array()
	for id in selected:
		if view.own_soldier(id):
			kept.append(id)
	if kept.size() != selected.size():
		_set_selection(kept)
	if hover_id != -1 and not view.soldiers_by_id.has(hover_id):
		hover_id = -1
	if tool == &"dig" and not _has_type(MatchView.DIGGERS):
		cancel_tool()
	elif tool == &"barrage" and not _has_type(MatchView.MORTARS):
		cancel_tool()
	elif tool == &"throw" and item_count(throw_item) == 0:
		cancel_tool()
	elif tool != &"" and selected.is_empty():
		cancel_tool()


func cursor() -> Vector2:
	if cursor_override != null:
		return cursor_override
	return view.get_viewport().get_mouse_position()


func is_selected(id: int) -> bool:
	return selected.has(id)


## Selects exactly these soldiers.
func select_only(ids: PackedInt32Array) -> void:
	_set_selection(ids)


## Selects every living man of these squads (add: keep the rest).
func select_squads(squad_ids: PackedInt32Array, add := false) -> void:
	var ids := selected.duplicate() if add else PackedInt32Array()
	for sid in squad_ids:
		if view.own_squad(sid):
			ids.append_array(view.squad_units.get(sid, PackedInt32Array()))
	_set_selection(ids)


## Adds a soldier, or removes him when already selected (Shift-click).
func toggle(id: int) -> void:
	var ids := selected.duplicate()
	var i := ids.find(id)
	if i >= 0:
		ids.remove_at(i)
	else:
		ids.append(id)
	_set_selection(ids)


func deselect(id: int) -> void:
	var ids := selected.duplicate()
	var i := ids.find(id)
	if i >= 0:
		ids.remove_at(i)
		_set_selection(ids)


## Adds a whole squad, or drops all its men when some are selected.
func toggle_squad(squad_id: int) -> void:
	if selected_in(squad_id).is_empty():
		select_squads(PackedInt32Array([squad_id]), true)
		return
	var ids := PackedInt32Array()
	for id in selected:
		if view.soldier_squad(id) != squad_id:
			ids.append(id)
	_set_selection(ids)


func _set_selection(ids: PackedInt32Array) -> void:
	var s := ids.duplicate()
	s.sort()
	var out := PackedInt32Array()
	for id in s:
		if out.is_empty() or out[out.size() - 1] != id:
			out.append(id)
	if out == selected:
		return
	selected = out
	view.emit_intent(&"select", {"squads": selected_squads(), "units": selected})


## Squads with at least one selected man, ascending.
func selected_squads() -> PackedInt32Array:
	var out := PackedInt32Array()
	for id in selected:
		var sid := view.soldier_squad(id)
		if sid >= 0 and not out.has(sid):
			out.append(sid)
	out.sort()
	return out


## Selected men of one squad, ascending.
func selected_in(squad_id: int) -> PackedInt32Array:
	var out := PackedInt32Array()
	for id in selected:
		if view.soldier_squad(id) == squad_id:
			out.append(id)
	return out


func squad_fully_selected(squad_id: int) -> bool:
	var all: PackedInt32Array = view.squad_units.get(squad_id, PackedInt32Array())
	return not all.is_empty() and selected_in(squad_id).size() == all.size()


## The squad the detail panel shows: the focused one while it has a
## selected man, else the first selected squad, else -1.
func focused_squad() -> int:
	var sq := selected_squads()
	if sq.has(focus_squad):
		return focus_squad
	return sq[0] if not sq.is_empty() else -1


func _has_type(types: Array) -> bool:
	for sid in selected_squads():
		if view.squad_type(sid) in types:
			return true
	return false


func _squads_of_type(types: Array) -> PackedInt32Array:
	var out := PackedInt32Array()
	for sid in selected_squads():
		if view.squad_type(sid) in types:
			out.append(sid)
	return out


## Splits the selection for a squad-or-units order: [squads, units].
## `types` keeps only squads of those types (empty = all); `exclude` drops
## these squads.
func split(types: Array = [], exclude := PackedInt32Array()) -> Array:
	var squads := PackedInt32Array()
	var units := PackedInt32Array()
	for sid in selected_squads():
		if exclude.has(sid) or (not types.is_empty() and not view.squad_type(sid) in types):
			continue
		if squad_fully_selected(sid):
			squads.append(sid)
		else:
			units.append_array(selected_in(sid))
	units.sort()
	return [squads, units]


## Emits `kind` for whole squads and `kind`_units for the rest of the
## selection (see split). Returns false when nobody got the order.
func send_order(kind: StringName, data := {}, types: Array = [], exclude := PackedInt32Array()) -> bool:
	var parts := split(types, exclude)
	if not parts[0].is_empty():
		var d := data.duplicate()
		d.squads = parts[0]
		view.emit_intent(kind, d)
	if not parts[1].is_empty():
		var d := data.duplicate()
		d.units = parts[1]
		view.emit_intent(StringName(String(kind) + "_units"), d)
	return not parts[0].is_empty() or not parts[1].is_empty()


## Sum of an item over the selected men (ability button counts).
func item_count(item: String) -> int:
	var n := 0
	for id in selected:
		n += view.item_count(view.soldiers_by_id.get(id, {}), item)
	return n


## Selected men who could throw `item` now: alive, not pinned, carrying it.
func throwers(item: String) -> PackedInt32Array:
	var out := PackedInt32Array()
	for id in selected:
		var so: Dictionary = view.soldiers_by_id.get(id, {})
		if view.item_count(so, item) > 0 and not view.soldier_pinned(so):
			out.append(id)
	return out


## Whether some eligible thrower has `g` within the item's range.
func throw_in_range(item: String, g: Vector3) -> bool:
	var r := view.throw_range(item)
	for id in throwers(item):
		var p: Vector3 = view.soldiers_by_id[id].pos
		var d := Vector2(p.x, p.z).distance_to(Vector2(g.x, g.z))
		if d >= r.x and d <= r.y:
			return true
	return false


## A soldier's stance override as the player last set it.
func stance_override(id: int) -> String:
	var so: Dictionary = view.soldiers_by_id.get(id, {})
	var have: String = so.get("stance_override", "auto")
	var p: Array = _pending_stance.get(id, [])
	if p.is_empty():
		return have
	if p[0] == have or Time.get_ticks_msec() - int(p[1]) > STANCE_PENDING_MS:
		_pending_stance.erase(id)
		return have
	return p[0]


## The override shared by every selected man, or "" when mixed or empty.
func active_stance() -> String:
	var out := ""
	for id in selected:
		var s := stance_override(id)
		if out == "":
			out = s
		elif out != s:
			return ""
	return out


## Stance key: sets `stance` on the selection; when every selected man
## already has it, returns them to Auto instead.
func press_stance(stance: String) -> void:
	if selected.is_empty() or view.input_blocked() or view.proxy_soldiers:
		return
	set_stance(&"auto" if active_stance() == stance else StringName(stance))


func set_stance(stance: StringName) -> void:
	# Stand-in soldiers (no `soldiers` in the state) carry squad ids: no
	# per-soldier commands for them.
	if selected.is_empty() or view.proxy_soldiers:
		return
	var now := Time.get_ticks_msec()
	for id in selected:
		_pending_stance[id] = [String(stance), now]
	view.emit_intent(&"set_stance", {"units": selected.duplicate(), "stance": String(stance)})


## Whether an order is available for the current selection (orders grid).
func can(order: StringName) -> bool:
	if selected.is_empty():
		return false
	match order:
		&"dig":
			return _has_type(MatchView.DIGGERS)
		&"barrage":
			return _has_type(MatchView.MORTARS)
		&"frag", &"smoke", &"satchel":
			return item_count(String(order)) > 0
		&"regroup":
			return true
	return true


# --- tools -----------------------------------------------------------------

## Starts an order tool or issues an instant order (stop, retreat, regroup).
## Abilities arm the throw tool: &"frag", &"smoke", &"satchel".
func activate(order: StringName) -> void:
	if view.input_blocked() or not can(order):
		if not selected.is_empty():
			AudioManager.play_ui(&"error")
		return
	match order:
		&"stop", &"retreat":
			send_order(order)
			cancel_tool()
		&"regroup":
			view.emit_intent(&"regroup", {"squads": selected_squads()})
			cancel_tool()
		&"dig":
			_set_tool(&"dig")
			dig_points.clear()
		&"frag", &"smoke", &"satchel":
			throw_item = String(order)
			tool = &""  # re-arm even when another item was armed
			_set_tool(&"throw")
		_:
			_set_tool(order)


func cancel_tool() -> void:
	dig_points.clear()
	throw_item = ""
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
	send_order(&"dig", {"polyline": dig_points.duplicate()}, MatchView.DIGGERS)
	if view.overlay != null:
		view.overlay.flash_polyline(dig_points)
	AudioManager.play_ui(&"confirm")
	cancel_tool()


## Throw tool click: the nearest eligible man throws (each = every eligible
## man). Out of range: error, the tool stays armed.
func throw_at(g: Vector3, each: bool) -> bool:
	if not throw_in_range(throw_item, g):
		AudioManager.play_ui(&"error")
		return false
	view.emit_intent(&"throw_each" if each else &"throw", {"units": selected.duplicate(), "item": throw_item, "target": g})
	if view.overlay != null:
		view.overlay.marker(g, &"throw")
	AudioManager.play_ui(&"confirm")
	cancel_tool()
	return true


# --- picking ---------------------------------------------------------------

## Soldier under a screen point: nearest within PICK_RADIUS_PX; near-ties
## go to the one closer to the camera, then the lower id.
## `own` = 1 own only, 0 enemies only, -1 any.
func pick(screen: Vector2, own := 1) -> int:
	var best := -1
	var best_d := INF
	var best_z := INF
	for i in view.pick_ids.size():
		if own != -1 and view.pick_own[i] != own:
			continue
		var d := screen.distance_to(view.pick_screen[i])
		if d > PICK_RADIUS_PX:
			continue
		var z := view.pick_depth[i]
		var id := view.pick_ids[i]
		var better := best == -1 or d < best_d - PICK_TIE_PX
		if not better and absf(d - best_d) <= PICK_TIE_PX:
			better = z < best_z or (z == best_z and id < best)
		if better:
			best = id
			best_d = d
			best_z = z
	return best


func _box_select(a: Vector2, b: Vector2, add: bool) -> void:
	var r := Rect2(a, Vector2.ZERO).expand(b)
	var ids := selected.duplicate() if add else PackedInt32Array()
	for i in view.pick_ids.size():
		if view.pick_own[i] == 1 and r.has_point(view.pick_screen[i]):
			ids.append(view.pick_ids[i])
	_set_selection(ids)


## Ctrl+double-click: every own squad of that type with a man on screen.
func _select_same_type(id: int, add: bool) -> void:
	var t := view.squad_type(view.soldier_squad(id))
	var squads := PackedInt32Array()
	var vp := view.get_viewport().get_visible_rect()
	for i in view.pick_ids.size():
		if view.pick_own[i] != 1 or not vp.has_point(view.pick_screen[i]):
			continue
		var sid := view.soldier_squad(view.pick_ids[i])
		if view.squad_type(sid) == t and not squads.has(sid):
			squads.append(sid)
	select_squads(squads, add)


# --- orders ----------------------------------------------------------------

func _order_at(screen: Vector2, attack: bool) -> void:
	if selected.is_empty():
		return
	var enemy := pick(screen, 0)
	if enemy != -1:
		var p: Vector3 = view.soldiers_by_id[enemy].pos
		send_order(&"attack_move", {"target": p})
		if view.overlay != null:
			view.overlay.marker(p, &"attack_move")
		return
	var g: Variant = view.ground_pick(screen)
	if g == null:
		return
	order_to_ground(g, attack)


## Right-click semantics on a ground point (also used by the minimap).
## Plain: mortar squads with the point in range barrage it, the rest move.
## attack: everyone attack-moves.
func order_to_ground(g: Vector3, attack := false) -> void:
	if selected.is_empty():
		return
	if attack:
		send_order(&"attack_move", {"target": g})
		if view.overlay != null:
			view.overlay.marker(g, &"attack_move")
		return
	var mortars := _mortars_in_range(g)
	if not mortars.is_empty():
		view.emit_intent(&"barrage", {"squads": mortars, "target": g})
	var moved := send_order(&"move", {"target": g}, [], mortars)
	if view.overlay != null:
		view.overlay.marker(g, &"move" if moved else &"barrage")


## Mortar squads (with a selected man) that can reach `g`.
func _mortars_in_range(g: Vector3) -> PackedInt32Array:
	var lo := float(view.rules().get("mortar_min_m", 60.0))
	var hi := float(view.rules().get("mortar_max_m", 600.0))
	var out := PackedInt32Array()
	for sid in _squads_of_type(MatchView.MORTARS):
		var p: Vector3 = view.squads_by_id[sid].pos
		var d := Vector2(p.x, p.z).distance_to(Vector2(g.x, g.z))
		if d >= lo and d <= hi:
			out.append(sid)
	return out


func _use_tool_at(screen: Vector2, shift: bool) -> void:
	var g: Variant = view.ground_pick(screen)
	if g == null:
		AudioManager.play_ui(&"error")
		return
	match tool:
		&"move", &"attack_move":
			send_order(tool, {"target": g})
		&"barrage":
			var ids := _mortars_in_range(g)
			if ids.is_empty():
				AudioManager.play_ui(&"error")
				return
			view.emit_intent(&"barrage", {"squads": ids, "target": g})
		&"throw":
			throw_at(g, shift)
			return
	if view.overlay != null:
		view.overlay.marker(g, tool)
	if not shift:
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
	var ctrl := e.ctrl_pressed or e.meta_pressed
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
					if ctrl:
						_select_same_type(id, e.shift_pressed)
					else:
						select_squads(PackedInt32Array([view.soldier_squad(id)]), e.shift_pressed)
					focus_squad = view.soldier_squad(id)
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
				toggle(id)
			else:
				_set_selection(PackedInt32Array([id]))
	elif e.button_index == MOUSE_BUTTON_RIGHT and e.pressed:
		if tool == &"dig":
			_confirm_dig()
		elif tool != &"":
			cancel_tool()
		else:
			_order_at(e.position, ctrl)
		get_viewport().set_input_as_handled()


func _key(e: InputEventKey) -> void:
	var g := MatchInput.group_of(e)
	if g > 0:
		if e.ctrl_pressed or e.meta_pressed:
			groups[g] = selected.duplicate()
			AudioManager.play_ui(&"confirm")
		else:
			recall(g)
		get_viewport().set_input_as_handled()
		return
	for action in STANCE_KEYS:
		if e.is_action_pressed(action):
			press_stance(STANCE_KEYS[action])
			get_viewport().set_input_as_handled()
			return
	for action in ABILITY_KEYS:
		if e.is_action_pressed(action):
			activate(StringName(ABILITY_KEYS[action]))
			get_viewport().set_input_as_handled()
			return
	for order in [&"move", &"stop", &"retreat", &"dig", &"barrage", &"regroup"]:
		if e.is_action_pressed(StringName("order_" + order)):
			activate(order)
			get_viewport().set_input_as_handled()
			return
	if e.is_action_pressed(&"dig_undo_point") and tool == &"dig" and not dig_points.is_empty():
		dig_points.remove_at(dig_points.size() - 1)
		get_viewport().set_input_as_handled()
	elif e.is_action_pressed(&"focus_call_in"):
		view.call_in.toggle_focus()
		get_viewport().set_input_as_handled()
	elif e.is_action_pressed(&"jump_to_event"):
		var pos: Variant = view.event_feed.last_position()
		if pos != null:
			view.jump_camera(pos)
		get_viewport().set_input_as_handled()


## Recalls control group `g` (its living own men); a second recall within
## RECALL_DOUBLE_MS centres the camera on them.
func recall(g: int) -> void:
	var ids := PackedInt32Array()
	for id in groups[g]:
		if view.own_soldier(id):
			ids.append(id)
	groups[g] = ids
	if ids.is_empty():
		return
	var now := Time.get_ticks_msec()
	if g == _last_recall_group and now - _last_recall_ms < RECALL_DOUBLE_MS:
		var c := Vector3.ZERO
		for id in ids:
			c += view.soldiers_by_id[id].pos
		view.jump_camera(c / ids.size())
	_last_recall_group = g
	_last_recall_ms = now
	_set_selection(ids)
