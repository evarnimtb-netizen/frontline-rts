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
##
## Vehicles (S3): own vehicle ids sit in the same selection as soldiers.
## A vehicle is picked inside its projected footprint or within 14 px of
## its centre (soldiers under the cursor win). Move, attack-move, stop and
## retreat send vehicle ids in the *_units intent next to the soldiers'
## orders; vehicle-only orders touch only vehicles. Right-click on a seen
## enemy vehicle: own vehicles and AT gunners get attack_unit, other men
## attack-move to it (S4: men of squads with Panzerfausts get attack_unit
## too and attack-move to it; the Panzerfaust tool does the same for them
## alone). Right-click on an own vehicle with infantry selected:
## the soldiers escort it (follow_units). Alt+right-click: vehicles reverse
## (reverse_units), soldiers move. O face tool, N next ammo, J repair.
##
## S3b: P arms the suppress tool (soldiers and vehicles): a left click fires
## at a point, a left drag draws a line (capped at rules.suppress_line_max_m,
## 40 m); the selection (mortar men are dropped by the sim) gets
## suppress_units. Shift keeps the tool armed.
##
## S3c: K arms the Crew tool (infantry selected): a left click on an
## abandoned vehicle sends `recrew` for the selected squads (the sim
## refuses AT teams and, per the rules, non-crews; the feed says why).
## Right-click on an abandoned vehicle with a tank crew squad selected
## does the same. Bail out (vehicles, no key) asks for a second press
## within BAIL_CONFIRM_MS, then sends `abandon` for the selected vehicles.
##
## S5b: B arms the fire-mission tool (the old Barrage tool id `barrage`
## stays internal). While it is armed the controller asks the source for
## the preview of the first selected mortar squad at the cursor
## (MatchView.request_fire_preview; the answer is state.fire_preview) and
## fire_tool_label() says what the cursor label shows: the mode and the
## expected miss, or in red the refusal. Shift+wheel sets the rounds
## (1–50, 0: the rules' player_rounds). A click sends `fire_mission`
## {squads, target, rounds} unless the preview at that point is refused
## (the reason stays on the label); right-click on the ground with mortars
## selected sends it too for those in range.

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
## Orders that carry vehicle ids too (in the *_units intent).
const VEHICLE_ORDERS := [&"move", &"attack_move", &"stop", &"retreat"]
## Orders for vehicles only.
const VEHICLE_ONLY := [&"reverse", &"face", &"repair", &"ammo", &"ammo_auto", &"ammo_ap", &"ammo_he", &"ammo_apcr", &"bail"]
## S3c: Bail out needs a second press within this long.
const BAIL_CONFIRM_MS := 2000

var view: MatchView
## Selected own soldier ids, ascending.
var selected := PackedInt32Array()
var groups: Array[PackedInt32Array] = []
## &"", &"move", &"attack_move", &"barrage", &"dig", &"throw", (S3)
## &"face", &"reverse" or (S3b) &"suppress".
var tool: StringName = &""
## Suppress tool (S3b): where the left button went down on the ground
## (the line's first point) while it is held; null otherwise.
var suppress_from: Variant = null
## Armed item while tool == &"throw": "frag", "smoke" or "satchel".
var throw_item := ""
var dig_points := PackedVector3Array()
## Box drag in screen space while dragging.
var dragging := false
var drag_from := Vector2.ZERO
var drag_to := Vector2.ZERO
## Soldier under the cursor (either side), or -1.
var hover_id := -1
## Squad the detail panel lists (last card clicked; a vehicle card sets its
## vehicle's squad id); -1 = first selected.
var focus_squad := -1
## CPU time of the last vehicle pick in microseconds (tests).
var pick_vehicle_us := 0
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
## S5a: fire modes just ordered, by man: [mode, msec].
var _pending_fire := {}
## vehicle id -> [ammo choice, msec sent] (N twice before the state shows it)
var _pending_ammo := {}
## S3c: when Bail out was pressed once (msec), 0 = not armed.
var bail_armed_ms := 0
## S5b: rounds the fire-mission tool orders (0: the rules' player_rounds).
var fm_rounds := 0
## S5b: the last preview asked for ([squad, ground point]), to ask again
## only when the cursor moved.
var _fp_asked: Array = []


func bind(v: MatchView) -> void:
	view = v
	groups.resize(10)
	for i in 10:
		groups[i] = PackedInt32Array()


## Drops ids that are gone or no longer ours (called by MatchView each frame).
func prune() -> void:
	var kept := PackedInt32Array()
	for id in selected:
		if view.own_unit(id):
			kept.append(id)
	if kept.size() != selected.size():
		_set_selection(kept)
	if hover_id != -1 and not view.soldiers_by_id.has(hover_id) and not view.vehicles_by_id.has(hover_id):
		hover_id = -1
	if tool in [&"face", &"reverse"] and selected_vehicles().is_empty():
		cancel_tool()
	elif tool == &"crew" and not can(&"crew"):
		cancel_tool()
	elif tool == &"dig" and not can(&"dig"):
		cancel_tool()
	elif tool == &"barrage" and not can(&"barrage"):
		cancel_tool()
	elif tool == &"panzerfaust" and not can(&"panzerfaust"):
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


## Selected own vehicles, ascending.
func selected_vehicles() -> PackedInt32Array:
	var out := PackedInt32Array()
	for id in selected:
		if view.is_vehicle(id):
			out.append(id)
	return out


## Selected soldiers (no vehicles), ascending.
func selected_soldiers() -> PackedInt32Array:
	var out := PackedInt32Array()
	for id in selected:
		if not view.is_vehicle(id):
			out.append(id)
	return out


## The vehicle the detail panel shows: the one whose card was clicked last
## while it is selected, else the first selected vehicle when no soldier is
## selected, else -1.
func focused_vehicle() -> int:
	var vs := selected_vehicles()
	if vs.is_empty():
		return -1
	for id in vs:
		if int(view.vehicles_by_id[id].get("squad", -2)) == focus_squad:
			return id
	return vs[0] if selected_soldiers().is_empty() else -1


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


## S4: whether a selected squad has any of `caps` (its state
## `capabilities`; a state without them, the mock's, falls back to the
## legacy `types`).
func _has_cap(caps: Array, types: Array) -> bool:
	for sid in selected_squads():
		if squad_has_cap(sid, caps, types):
			return true
	return false


## S4: whether squad `sid` has any of `caps` (or, without capabilities in
## the state, is one of `types`).
func squad_has_cap(sid: int, caps: Array, types: Array) -> bool:
	var sq: Dictionary = view.squads_by_id.get(sid, {})
	if sq.has("capabilities"):
		for c in sq.capabilities:
			if String(c) in caps:
				return true
		return false
	return view.squad_type(sid) in types


func _squads_of_type(types: Array) -> PackedInt32Array:
	var out := PackedInt32Array()
	for sid in selected_squads():
		if view.squad_type(sid) in types:
			out.append(sid)
	return out


## Splits the selection for a squad-or-units order: [squads, units].
## `types` keeps only squads of those types (empty = all); `exclude` drops
## these squads; `vehicles` adds the selected vehicles to `units`.
func split(types: Array = [], exclude := PackedInt32Array(), vehicles := false) -> Array:
	var squads := PackedInt32Array()
	var units := PackedInt32Array()
	for sid in selected_squads():
		if exclude.has(sid) or (not types.is_empty() and not view.squad_type(sid) in types):
			continue
		if squad_fully_selected(sid):
			squads.append(sid)
		else:
			units.append_array(selected_in(sid))
	if vehicles:
		units.append_array(selected_vehicles())
	units.sort()
	return [squads, units]


## Emits `kind` for whole squads and `kind`_units for the rest of the
## selection (see split); move, attack-move, stop and retreat also carry
## the selected vehicles in `units` unless `types` filters squad types.
## Returns false when nobody got the order.
func send_order(kind: StringName, data := {}, types: Array = [], exclude := PackedInt32Array(), vehicles := true) -> bool:
	var parts := split(types, exclude, vehicles and types.is_empty() and kind in VEHICLE_ORDERS)
	if not parts[0].is_empty():
		var d := data.duplicate()
		d.squads = parts[0]
		view.emit_intent(kind, d)
	if not parts[1].is_empty():
		var d := data.duplicate()
		d.units = parts[1]
		view.emit_intent(StringName(String(kind) + "_units"), d)
	return not parts[0].is_empty() or not parts[1].is_empty()


## S4: whether a man of squad `sid` carries `item` (state items).
func _carries(sid: int, item: String) -> bool:
	for id in selected_in(sid):
		if view.item_count(view.soldiers_by_id.get(id, {}), item) > 0:
			return true
	return false


## S4 Panzerfaust tool: the selected men of squads carrying one get
## attack_unit on enemy vehicle `target` and attack-move to it. False
## when none carries one.
func panzerfaust_at(target: int) -> bool:
	var v: Dictionary = view.vehicles_by_id.get(target, {})
	if v.is_empty():
		return false
	var men := PackedInt32Array()
	var squads := PackedInt32Array()
	for sid in selected_squads():
		if _carries(sid, "panzerfaust"):
			squads.append(sid)
			men.append_array(selected_in(sid))
	if men.is_empty():
		return false
	men.sort()
	view.emit_intent(&"attack_unit", {"units": men, "target": target})
	view.emit_intent(&"attack_move", {"squads": squads, "target": v.pos})
	if view.overlay != null:
		view.overlay.marker(v.pos, &"attack_move")
	AudioManager.play_ui(&"confirm")
	return true


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


## S5a: the fire mode every selected man shares ("discipline",
## "at_will", "hold"), or "" when mixed or empty. A mode just ordered
## shows at once (pending, as stances).
func active_fire() -> String:
	var out := ""
	var now := Time.get_ticks_msec()
	for id in selected:
		var so: Dictionary = view.soldiers_by_id.get(id, {})
		var f: String = so.get("fire", "discipline")
		var p: Array = _pending_fire.get(id, [])
		if not p.is_empty() and p[0] != f and now - int(p[1]) <= STANCE_PENDING_MS:
			f = p[0]
		if out == "":
			out = f
		elif out != f:
			return ""
	return out


## S5a: sends `set_fire` for the selected men.
func set_fire(mode: String) -> void:
	if selected.is_empty() or view.proxy_soldiers or view.input_blocked():
		return
	var now := Time.get_ticks_msec()
	for id in selected:
		_pending_fire[id] = [mode, now]
	view.emit_intent(&"set_fire", {"units": selected.duplicate(), "mode": mode})


## S6: whether a selected squad has card tag `tag` (state `tags`).
func has_tag(tag: String) -> bool:
	for sid in selected_squads():
		var tags: PackedStringArray = view.squads_by_id.get(sid, {}).get("tags", PackedStringArray())
		if tags.has(tag):
			return true
	return false


## S6: whether every selected squad is an HQ squad or bearers (the supply
## grid; no vehicles selected).
func supply_only() -> bool:
	var sq := selected_squads()
	if sq.is_empty() or not selected_vehicles().is_empty():
		return false
	for sid in sq:
		var tags: PackedStringArray = view.squads_by_id.get(sid, {}).get("tags", PackedStringArray())
		if not (tags.has("hq") or tags.has("bearers")):
			return false
	return true


## S6: crates the selected men carry.
func carried_crates() -> Array:
	var out: Array = []
	for id in selected:
		var so: Dictionary = view.soldiers_by_id.get(id, {})
		if so.has("crate"):
			out.append(so.crate)
	return out


## S6: the own set-down crate nearest `g` within 4 m (-1: none).
func crate_near(g: Vector3) -> int:
	var best := -1
	var bd := 16.0
	for c in view.state.get("crates", []):
		if int(c.owner) != view.local_player or int(c.get("carrier", -1)) != -1:
			continue
		var d: float = Vector2(c.pos.x - g.x, c.pos.z - g.z).length_squared()
		if d <= bd:
			bd = d
			best = int(c.id)
	return best


## Whether an order is available for the current selection (orders grid).
func can(order: StringName) -> bool:
	if selected.is_empty():
		return false
	match order:
		# S6.
		&"build_hq":
			return has_tag("hq")
		&"build_dump":
			return has_tag("hq") or has_tag("engineer")
		&"crate_shells", &"crate_small_arms", &"crate_explosives", &"pick_up":
			return not selected_squads().is_empty()
		&"place_crate":
			return not carried_crates().is_empty()
		&"dig":
			# S4: by capability (`dig_basic`).
			return _has_cap(["dig_basic"], MatchView.DIGGERS)
		&"barrage":
			return _has_cap(["barrage"], MatchView.MORTARS)
		&"frag", &"smoke", &"satchel", &"panzerfaust":
			return item_count(String(order)) > 0
		&"regroup":
			return not selected_soldiers().is_empty()
		&"crew":
			# S3c: infantry (not AT teams) can man an abandoned vehicle;
			# S4: squads with `recrew` or `vehicle_crew`.
			for sid in selected_squads():
				var sq: Dictionary = view.squads_by_id.get(sid, {})
				if sq.has("capabilities"):
					if squad_has_cap(sid, ["recrew", "vehicle_crew"], []):
						return true
				elif not view.squad_type(sid) in ["at", "vehicle"]:
					return true
			return false
		&"ammo_apcr":
			return selection_has_apcr()
		&"suppress":
			# Anyone but mortar crews (they keep Barrage).
			if not selected_vehicles().is_empty():
				return true
			for sid in selected_squads():
				if not view.squads_by_id.get(sid, {}).get("type", "") in MatchView.MORTARS:
					return true
			return false
	if order in VEHICLE_ONLY:
		return not selected_vehicles().is_empty()
	return true


## Whether the first selected vehicle's gun carries the special round
## (S3b APCR).
func selection_has_apcr() -> bool:
	var vs := selected_vehicles()
	return not vs.is_empty() and Vehicles.has_apcr(view.vehicles_by_id.get(vs[0], {}))


## Sets the ammo choice of the selected vehicles ("auto", "ap", "he"; S3b
## "apcr": a gun without it fires its standard round).
func set_ammo(choice: String) -> void:
	var vs := selected_vehicles()
	if vs.is_empty() or not choice in Vehicles.AMMO_CHOICES:
		return
	var now := Time.get_ticks_msec()
	for id in vs:
		_pending_ammo[id] = [choice, now]
	view.emit_intent(&"set_ammo", {"units": vs, "ammo": choice})


## The ammo choice shown for the selection: the first selected vehicle's
## (as the state reports it, or as just sent), "" without vehicles.
func ammo_choice() -> String:
	var vs := selected_vehicles()
	if vs.is_empty():
		return ""
	var p: Array = _pending_ammo.get(vs[0], [])
	var have := String(view.vehicles_by_id[vs[0]].get("ammo_choice", "auto"))
	if not p.is_empty() and p[0] != have and Time.get_ticks_msec() - int(p[1]) < STANCE_PENDING_MS:
		return p[0]
	return have


## N: the next ammo choice after the first selected vehicle's.
func next_ammo() -> void:
	var cur := ammo_choice()
	if cur != "":
		set_ammo(Vehicles.next_ammo(cur, selection_has_apcr()))


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
		&"repair":
			view.emit_intent(&"repair_units", {"units": selected_vehicles()})
			cancel_tool()
		&"bail":
			# S3c: a second press within BAIL_CONFIRM_MS bails out.
			var now := Time.get_ticks_msec()
			if bail_armed_ms > 0 and now - bail_armed_ms <= BAIL_CONFIRM_MS:
				bail_armed_ms = 0
				view.emit_intent(&"abandon", {"units": selected_vehicles()})
			else:
				bail_armed_ms = maxi(now, 1)
			cancel_tool()
		&"ammo":
			next_ammo()
		&"ammo_auto", &"ammo_ap", &"ammo_he", &"ammo_apcr":
			set_ammo(String(order).trim_prefix("ammo_"))
		&"regroup":
			view.emit_intent(&"regroup", {"squads": selected_squads()})
			cancel_tool()
		# S6: a crate of that kind for each selected squad (in the base).
		&"crate_shells", &"crate_small_arms", &"crate_explosives":
			view.emit_intent(&"issue_crate", {"squads": selected_squads(), "kind": String(order).trim_prefix("crate_")})
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


## S5b: rounds the fire-mission tool orders now.
func fire_rounds() -> int:
	if fm_rounds > 0:
		return fm_rounds
	return int(view.rules().get("fire_mission_rounds", 6))


## S5b: the mortar squad the fire-mission preview is for (the focused one
## if it is a mortar, else the first selected mortar squad; -1: none).
func preview_squad() -> int:
	var mortars := _squads_of_type(MatchView.MORTARS)
	if mortars.is_empty():
		return -1
	if focus_squad in mortars:
		return focus_squad
	return mortars[0]


## S5b: the fire-mission tool's cursor label: {text, ok}.
func fire_tool_label() -> Dictionary:
	var fp: Dictionary = view.state.get("fire_preview", {})
	var n := fire_rounds()
	if fp.is_empty():
		return {"text": "FIRE MISSION  ·  %d rounds  ·  click a target" % n, "ok": true}
	if not fp.get("ok", false):
		return {"text": "FIRE MISSION  ·  %s" % String(fp.get("text", "Refused")), "ok": false}
	var mode := String(fp.get("mode", "map"))
	var word: String = {"direct": "Direct", "observed": "Observed", "map": "Map fire"}.get(mode, mode)
	var text := "FIRE MISSION  ·  %s  ·  ±%d m  ·  %d rounds" % [word, roundi(float(fp.get("ring_m", 0.0))), n]
	if fp.get("ranged", false):
		text += "  ·  ranged in"
	if fp.get("walk", false):
		text += "  ·  out of range: the team will move"
	return {"text": text + "  ·  Shift+wheel rounds", "ok": true}


## S5b: asks for the preview at the cursor while the tool is armed.
func _update_fire_preview() -> void:
	if tool != &"barrage":
		if not _fp_asked.is_empty():
			_fp_asked = []
			view.clear_fire_preview()
		return
	var sid := preview_squad()
	var g: Variant = view.ground_pick(cursor())
	if sid == -1 or g == null:
		return
	if not _fp_asked.is_empty() and _fp_asked[0] == sid and _fp_asked[2] == fire_rounds() \
			and Vector2(g.x, g.z).distance_to(Vector2(_fp_asked[1].x, _fp_asked[1].z)) < 1.0:
		return
	_fp_asked = [sid, g, fire_rounds()]
	view.request_fire_preview(sid, g, fire_rounds())


## Whether the latest preview refuses a mission at `g`.
func _preview_refuses(g: Vector3) -> bool:
	var fp: Dictionary = view.state.get("fire_preview", {})
	if fp.is_empty() or fp.get("ok", true):
		return false
	var p: Vector3 = fp.point
	return Vector2(p.x, p.z).distance_to(Vector2(g.x, g.z)) < 2.0


## S3c: whether Bail out is waiting for its second press.
func bail_armed() -> bool:
	return bail_armed_ms > 0 and Time.get_ticks_msec() - bail_armed_ms <= BAIL_CONFIRM_MS


## S3c: the selected infantry squads (not AT teams) re-crew abandoned
## vehicle `vehicle`; only tank crews with `crews_only`.
func recrew(vehicle: int, crews_only := false) -> bool:
	var v: Dictionary = view.vehicles_by_id.get(vehicle, {})
	if not Vehicles.is_abandoned(v):
		return false
	var squads := PackedInt32Array()
	for sid in selected_squads():
		var t := view.squad_type(sid)
		if t == "crew" or (not crews_only and not t in ["at", "vehicle"]):
			squads.append(sid)
	if squads.is_empty():
		return false
	view.emit_intent(&"recrew", {"squads": squads, "vehicle": vehicle})
	if view.overlay != null:
		view.overlay.marker(v.pos, &"follow")
	AudioManager.play_ui(&"confirm")
	return true


func cancel_tool() -> void:
	dig_points.clear()
	throw_item = ""
	suppress_from = null
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

## Unit under a screen point: a soldier (see pick_soldier), else a live
## vehicle (see pick_vehicle). `own` = 1 own only, 0 enemies only, -1 any.
func pick(screen: Vector2, own := 1) -> int:
	var id := pick_soldier(screen, own)
	return id if id != -1 else pick_vehicle(screen, own)


## Vehicle under a screen point: inside its projected footprint or within
## PICK_RADIUS_PX of its centre; the nearest centre wins, then the one
## closer to the camera. `own` as in pick().
func pick_vehicle(screen: Vector2, own := 1) -> int:
	var t0 := Time.get_ticks_usec()
	var best := -1
	var best_d := INF
	var best_z := INF
	for i in view.veh_pick_ids.size():
		if own != -1 and view.veh_pick_own[i] != own:
			continue
		var id := view.veh_pick_ids[i]
		var d := screen.distance_to(view.veh_pick_screen[i])
		if d > PICK_RADIUS_PX:
			# Outside the vehicle's screen reach no footprint can hold it.
			if d > view.veh_pick_reach[i]:
				continue
			var fp: PackedVector2Array = view.vehicle_screen_footprint(id)
			if fp.size() < 3 or not Geometry2D.is_point_in_polygon(screen, fp):
				continue
		var z := view.veh_pick_depth[i]
		if best == -1 or d < best_d - PICK_TIE_PX or (absf(d - best_d) <= PICK_TIE_PX and z < best_z):
			best = id
			best_d = d
			best_z = z
	pick_vehicle_us = Time.get_ticks_usec() - t0
	return best


## Soldier under a screen point: nearest within PICK_RADIUS_PX, or within
## his body's half length on screen when that is more (close up); near-ties
## go to the one closer to the camera, then the lower id.
## `own` = 1 own only, 0 enemies only, -1 any.
func pick_soldier(screen: Vector2, own := 1) -> int:
	var best := -1
	var best_d := INF
	var best_z := INF
	for i in view.pick_ids.size():
		if own != -1 and view.pick_own[i] != own:
			continue
		var d := screen.distance_to(view.pick_screen[i])
		var reach := view.pick_reach[i] if i < view.pick_reach.size() else 0.0
		if d > maxf(PICK_RADIUS_PX, reach):
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


## Box-select own men. With Shift the box adds its men to the selection,
## or, when every man in it is already selected, takes them out.
func _box_select(a: Vector2, b: Vector2, add: bool) -> void:
	var r := Rect2(a, Vector2.ZERO).expand(b)
	var boxed := PackedInt32Array()
	for i in view.pick_ids.size():
		if view.pick_own[i] == 1 and r.has_point(view.pick_screen[i]):
			boxed.append(view.pick_ids[i])
	for i in view.veh_pick_ids.size():
		if view.veh_pick_own[i] == 1 and r.has_point(view.veh_pick_screen[i]):
			boxed.append(view.veh_pick_ids[i])
	if not add:
		_set_selection(boxed)
		return
	var ids := selected.duplicate()
	var remove := not boxed.is_empty() and Array(boxed).all(func(id: int) -> bool: return ids.has(id))
	for id in boxed:
		if remove:
			ids.remove_at(ids.find(id))
		else:
			ids.append(id)
	_set_selection(ids)


## Ctrl+double-click on a vehicle: every own vehicle of that type on
## screen.
func _select_same_vehicles(id: int, add: bool) -> void:
	var t := String(view.vehicles_by_id[id].type)
	var ids := selected.duplicate() if add else PackedInt32Array()
	var vp := view.get_viewport().get_visible_rect()
	for i in view.veh_pick_ids.size():
		var vid := view.veh_pick_ids[i]
		if view.veh_pick_own[i] == 1 and vp.has_point(view.veh_pick_screen[i]) and String(view.vehicles_by_id[vid].type) == t:
			ids.append(vid)
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

func _order_at(screen: Vector2, attack: bool, reverse := false) -> void:
	if selected.is_empty():
		return
	# S3c: right-click on an abandoned vehicle with a tank crew selected
	# re-crews it; anything else treats it as the ground there.
	var any_v := pick_vehicle(screen, -1) if pick_soldier(screen, -1) == -1 else -1
	if any_v != -1 and Vehicles.is_abandoned(view.vehicles_by_id.get(any_v, {})):
		if recrew(any_v, true):
			return
		var gv: Variant = view.ground_pick(screen)
		if gv != null:
			order_to_ground(gv, attack, reverse)
		return
	var enemy_v := pick_vehicle(screen, 0) if pick_soldier(screen, 0) == -1 else -1
	if enemy_v != -1:
		attack_vehicle(enemy_v)
		return
	var own_v := pick_vehicle(screen, 1) if pick_soldier(screen, 1) == -1 else -1
	if own_v != -1 and not selected_soldiers().is_empty() and not selected.has(own_v):
		escort(own_v)
		return
	var enemy := pick_soldier(screen, 0)
	if enemy != -1:
		var p: Vector3 = view.soldiers_by_id[enemy].pos
		send_order(&"attack_move", {"target": p})
		if view.overlay != null:
			view.overlay.marker(p, &"attack_move")
		return
	var g: Variant = view.ground_pick(screen)
	if g == null:
		return
	order_to_ground(g, attack, reverse)


## Right-click on a seen enemy vehicle: own vehicles and the men of AT
## teams focus it (attack_unit); the other men attack-move to it.
func attack_vehicle(target: int) -> void:
	var v: Dictionary = view.vehicles_by_id.get(target, {})
	if v.is_empty():
		return
	var focus := selected_vehicles()
	var at_squads := PackedInt32Array()
	for sid in selected_squads():
		if view.squad_type(sid) == "at":
			at_squads.append(sid)
			focus.append_array(selected_in(sid))
		elif _carries(sid, "panzerfaust"):
			# S4: Panzerfaust carriers aim at it first and close in.
			focus.append_array(selected_in(sid))
	focus.sort()
	if not focus.is_empty():
		view.emit_intent(&"attack_unit", {"units": focus, "target": target})
	send_order(&"attack_move", {"target": v.pos}, [], at_squads, false)
	if view.overlay != null:
		view.overlay.marker(v.pos, &"attack_move")


## Right-click on an own vehicle with infantry selected: the selected men
## escort it (follow_units; vehicle ids are not sent).
func escort(vehicle: int) -> void:
	var men := selected_soldiers()
	if men.is_empty():
		return
	view.emit_intent(&"follow_units", {"units": men, "vehicle": vehicle})
	if view.overlay != null:
		view.overlay.marker(view.vehicles_by_id[vehicle].pos, &"follow")
	AudioManager.play_ui(&"confirm")


## Right-click semantics on a ground point (also used by the minimap).
## Plain: mortar squads with the point in range barrage it, the rest move.
## attack: everyone attack-moves. reverse (Alt): vehicles back up to the
## point (reverse_units), soldiers move.
func order_to_ground(g: Vector3, attack := false, reverse := false) -> void:
	if selected.is_empty():
		return
	if reverse and not selected_vehicles().is_empty():
		view.emit_intent(&"reverse_units", {"units": selected_vehicles(), "target": g})
		send_order(&"move", {"target": g}, [], PackedInt32Array(), false)
		if view.overlay != null:
			view.overlay.marker(g, &"reverse")
		return
	if attack:
		send_order(&"attack_move", {"target": g})
		if view.overlay != null:
			view.overlay.marker(g, &"attack_move")
		return
	var mortars := _mortars_in_range(g)
	if not mortars.is_empty():
		view.emit_intent(&"fire_mission", {"squads": mortars, "target": g, "rounds": fire_rounds()})
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


## S5b: mortar squads (with a selected man) not inside their minimum range
## of `g`.
func _mortars_beyond_min(g: Vector3) -> PackedInt32Array:
	var lo := float(view.rules().get("mortar_min_m", 60.0))
	var out := PackedInt32Array()
	for sid in _squads_of_type(MatchView.MORTARS):
		var p: Vector3 = view.squads_by_id[sid].pos
		if Vector2(p.x, p.z).distance_to(Vector2(g.x, g.z)) >= lo:
			out.append(sid)
	return out


func _use_tool_at(screen: Vector2, shift: bool) -> void:
	var g: Variant = view.ground_pick(screen)
	if g == null:
		AudioManager.play_ui(&"error")
		return
	match tool:
		&"panzerfaust":
			# S4: the seen enemy vehicle under the click; the selected
			# squads carrying Panzerfausts aim at it and close in.
			var pv := pick_vehicle(screen, 0)
			if pv == -1 or not panzerfaust_at(pv):
				AudioManager.play_ui(&"error")
				return
			if not shift:
				_set_tool(&"")
			return
		&"crew":
			# S3c: the abandoned vehicle under the click.
			var v := pick_vehicle(screen, -1)
			if v == -1 or not recrew(v):
				AudioManager.play_ui(&"error")
				return
			if not shift:
				_set_tool(&"")
			return
		# S6: build an HQ or dump here (long axis across the line to the
		# camera's forward), set crates down, pick one up.
		&"build_hq", &"build_dump":
			var fwd: Vector3 = -view.camera.global_transform.basis.z if view.camera != null else Vector3(0, 0, -1)
			var across := Vector3(-fwd.z, 0, fwd.x)
			view.emit_intent(&"build", {"squads": selected_squads(), "target": g, "what": "hq" if tool == &"build_hq" else "dump", "facing": across})
		&"place_crate":
			view.emit_intent(&"place_crate", {"squads": selected_squads(), "target": g})
		&"pick_up":
			var c := crate_near(g)
			if c == -1:
				AudioManager.play_ui(&"error")
				return
			view.emit_intent(&"pick_up_crate", {"squads": selected_squads(), "crate": c})
		&"move", &"attack_move":
			send_order(tool, {"target": g})
		&"face":
			view.emit_intent(&"face_units", {"units": selected_vehicles(), "target": g})
		&"reverse":
			order_to_ground(g, false, true)
			if not shift:
				_set_tool(&"")
			return
		&"barrage":
			# S5b: every selected mortar outside its minimum range (beyond
			# its range it walks up first); not at a point the preview
			# refuses (the label says why).
			var ids := _mortars_beyond_min(g)
			if ids.is_empty() or _preview_refuses(g):
				AudioManager.play_ui(&"error")
				return
			view.emit_intent(&"fire_mission", {"squads": ids, "target": g, "rounds": fire_rounds()})
		&"throw":
			throw_at(g, shift)
			return
	if view.overlay != null:
		view.overlay.marker(g, tool)
	if not shift:
		_set_tool(&"")


## Longest suppress line (m; rules.suppress_line_max_m).
func suppress_max() -> float:
	return float(view.rules().get("suppress_line_max_m", 40.0))


## The suppress line from `a` toward `b` on the ground, capped at
## suppress_max(): [a] for a click (under 2 m), else [a, b'].
func suppress_line(a: Vector3, b: Variant) -> PackedVector3Array:
	if b == null or Vector2(a.x, a.z).distance_to(Vector2(b.x, b.z)) < 2.0:
		return PackedVector3Array([a])
	var d := Vector3(b.x - a.x, 0.0, b.z - a.z)
	var n := minf(d.length(), suppress_max())
	var e := a + d.normalized() * n
	e.y = view.ground_y(e.x, e.z)
	return PackedVector3Array([a, e])


## Sends suppress_units for the selection at `line` (one or two points).
func suppress_at(line: PackedVector3Array) -> void:
	if line.is_empty():
		return
	var data := {"units": selected}
	if line.size() == 1:
		data["target"] = line[0]
	else:
		data["polyline"] = line
	view.emit_intent(&"suppress_units", data)
	if view.overlay != null:
		view.overlay.marker(line[0], &"suppress")


# --- input -----------------------------------------------------------------

func _process(_delta: float) -> void:
	if view != null:
		_update_fire_preview()
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
	if event.is_action_pressed(&"debug_reload_data") and OS.is_debug_build() and not view.net_mode:
		view.emit_intent(&"reload_data", {})
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
	# S5b: Shift+wheel sets the fire-mission rounds while the tool is armed.
	if tool == &"barrage" and e.shift_pressed and e.pressed \
			and e.button_index in [MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN]:
		var step := 1 if e.button_index == MOUSE_BUTTON_WHEEL_UP else -1
		fm_rounds = clampi(fire_rounds() + step, 1, 50)
		get_viewport().set_input_as_handled()
		return
	if e.button_index == MOUSE_BUTTON_LEFT:
		if e.pressed:
			if tool == &"dig":
				var p: Variant = dig_candidate(e.position)
				if p != null and (dig_points.is_empty() or dig_length() < dig_max() - 0.5):
					dig_points.append(p)
				get_viewport().set_input_as_handled()
				return
			if tool == &"suppress":
				suppress_from = view.ground_pick(e.position)
				if suppress_from == null:
					AudioManager.play_ui(&"error")
				get_viewport().set_input_as_handled()
				return
			if tool != &"":
				_use_tool_at(e.position, e.shift_pressed)
				get_viewport().set_input_as_handled()
				return
			if e.double_click:
				var id := pick(e.position, 1)
				if id != -1 and view.is_vehicle(id):
					# A vehicle is a squad of one: double-click keeps it,
					# Ctrl+double-click takes its type on screen.
					if ctrl:
						_select_same_vehicles(id, e.shift_pressed)
					focus_squad = int(view.vehicles_by_id[id].get("squad", -1))
					_skip_release = true
					return
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
			if tool == &"suppress" and suppress_from != null:
				var line := suppress_line(suppress_from, view.ground_pick(e.position))
				suppress_from = null
				suppress_at(line)
				if not e.shift_pressed:
					_set_tool(&"")
				get_viewport().set_input_as_handled()
				return
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
			if id != -1 and view.is_vehicle(id):
				focus_squad = int(view.vehicles_by_id[id].get("squad", -1))
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
			_order_at(e.position, ctrl, e.alt_pressed)
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
	for order in [&"move", &"stop", &"retreat", &"dig", &"barrage", &"regroup", &"face", &"repair", &"suppress", &"crew"]:
		if e.is_action_pressed(StringName("order_" + order)):
			activate(order)
			get_viewport().set_input_as_handled()
			return
	if e.is_action_pressed(&"ammo_next"):
		activate(&"ammo")
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
		if view.own_unit(id):
			ids.append(id)
	groups[g] = ids
	if ids.is_empty():
		return
	var now := Time.get_ticks_msec()
	if g == _last_recall_group and now - _last_recall_ms < RECALL_DOUBLE_MS:
		var c := Vector3.ZERO
		for id in ids:
			c += view.unit_pos(id)
		view.jump_camera(c / ids.size())
	_last_recall_group = g
	_last_recall_ms = now
	_set_selection(ids)
