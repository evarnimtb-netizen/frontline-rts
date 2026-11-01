extends Control
## Detail panel (HUD v2 console, middle). One selected soldier: his
## readout (health, stance and override, state, weapon, order target,
## items). Otherwise the focused squad: its men as clickable rows in pip
## order (row-first, two columns), the selected ones highlighted. Click a
## row to select that man (Shift toggles), double-click to look at him.
## Nothing selected: a short how-to.

const PAD := 10.0
const HEAD := 27.0

var view: MatchView
## Laid-out man rows of the focused squad: {id (-1 dead), slot, rect}.
var rows: Array = []


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_STOP
	tooltip_text = " "


func update_state(_s: Dictionary) -> void:
	rows = _squad_rows(view.selection.focused_squad()) if mode() == "squad" and not view.proxy_soldiers else []
	queue_redraw()


## What the panel shows: "soldier", "squad" or "none".
func mode() -> String:
	var sel: Node = view.selection
	if sel.selected.size() == 1 and not view.proxy_soldiers:
		return "soldier"
	return "squad" if sel.focused_squad() != -1 else "none"


## Man rows of a squad in pip (slot) order, row-first in two columns.
func _squad_rows(sid: int) -> Array:
	var out: Array = []
	var sq: Dictionary = view.squads_by_id.get(sid, {})
	if sq.is_empty():
		return out
	var units: PackedInt32Array = view.squad_units.get(sid, PackedInt32Array())
	var men_max := maxi(int(sq.get("men_max", units.size())), units.size())
	var by_slot := {}
	for id in units:
		by_slot[int(view.soldiers_by_id[id].get("slot", by_slot.size()))] = id
	var n_rows := maxi(ceili(men_max / 2.0), 1)
	var rh := minf((size.y - HEAD - 6.0) / maxf(n_rows, 4.0), 26.0)
	var colw := (size.x - 2.0 * PAD - 8.0) / 2.0
	for i in men_max:
		var x := PAD + (i % 2) * (colw + 8.0)
		var y := HEAD + int(i / 2.0) * rh
		out.append({"id": int(by_slot.get(i, -1)), "slot": i, "rect": Rect2(x - 4.0, y, colw + 4.0, rh)})
	return out


func _draw() -> void:
	if view == null:
		return
	HudStyle.box(self, Rect2(Vector2.ZERO, size), HudStyle.BG_PANEL, 6)
	match mode():
		"soldier":
			_draw_soldier(view.soldiers_by_id[view.selection.selected[0]])
		"squad":
			_draw_squad(view.selection.focused_squad())
		_:
			_draw_help()


func _header(left: String, right: String, right_col := HudStyle.DIM) -> void:
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var rw := HudStyle.text_w(font, right, HudStyle.FONT)
	HudStyle.text(self, bold, Vector2(PAD, 19), HudStyle.fit(bold, left, size.x - 2.0 * PAD - rw - 12.0, HudStyle.FONT), HudStyle.FONT, HudStyle.TEXT)
	HudStyle.text(self, font, Vector2(size.x - PAD, 19), right, HudStyle.FONT, right_col, 1)


func _draw_help() -> void:
	var font := get_theme_default_font()
	_header("No selection", "")
	var lines := ["Click a soldier · double-click: his squad", "Ctrl+double-click: that type on screen", "Drag: box-select · Shift adds or toggles"]
	for i in lines.size():
		HudStyle.text(self, font, Vector2(PAD, HEAD + 18.0 + i * 24.0), HudStyle.fit(font, lines[i], size.x - 2.0 * PAD, HudStyle.FONT), HudStyle.FONT, HudStyle.DIM)


func _draw_soldier(so: Dictionary) -> void:
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	_header(view.soldier_name(so), view.squad_label(int(so.squad)))
	var rh := (size.y - HEAD - 6.0) / 4.0
	var x0 := PAD
	var y := HEAD
	var base := rh * 0.5 + 5.0
	# Health.
	var hp := clampf(float(so.get("hp", 100.0)), 0.0, 100.0)
	HudStyle.text(self, font, Vector2(x0, y + base), "Health", HudStyle.FONT, HudStyle.DIM)
	var bar := Rect2(x0 + 64.0, y + rh * 0.5 - 4.0, size.x - x0 - 64.0 - PAD - 40.0, 8.0)
	HudStyle.box(self, bar, Color(0, 0, 0, 0.35), 2)
	var col := HudStyle.TEXT if hp >= MatchView.WOUNDED_HP else (HudStyle.ACCENT if hp >= 30.0 else HudStyle.DANGER)
	HudStyle.box(self, Rect2(bar.position, Vector2(bar.size.x * hp / 100.0, bar.size.y)), col, 2)
	HudStyle.text(self, bold, Vector2(size.x - PAD, y + base), "%d" % roundi(hp), HudStyle.FONT, HudStyle.TEXT, 1)
	y += rh
	# Stance and state.
	var stance: String = so.get("stance", "stand")
	var ov: String = view.selection.stance_override(int(so.id))
	_pair(font, bold, x0, y + base, "Stance", "%s (%s)" % [stance.capitalize(), "Auto" if ov == "auto" else "set"])
	var pinned := view.soldier_pinned(so)
	var cond := "Pinned" if pinned else ("Detached" if so.get("detached", false) else "With squad")
	HudStyle.text(self, font, Vector2(size.x - PAD, y + base), cond, HudStyle.FONT, HudStyle.ACCENT if pinned else HudStyle.DIM, 1)
	y += rh
	# Weapon and order target.
	_pair(font, bold, x0, y + base, "Weapon", view.soldier_weapon(so))
	if so.has("target"):
		var t: Vector3 = so.target
		var d := Vector2(t.x, t.z).distance_to(Vector2(so.pos.x, so.pos.z))
		var order: String = MatchView.ORDER_TEXT.get(so.get("order", "move"), "Order")
		HudStyle.text(self, font, Vector2(size.x - PAD, y + base), "%s %d m" % [order, roundi(d)], HudStyle.FONT, HudStyle.DIM, 1)
	y += rh
	# Items.
	var x := x0
	for item in MatchView.ITEMS:
		var n := view.item_count(so, item)
		var label: String = item.capitalize()
		HudStyle.text(self, font, Vector2(x, y + base), label, HudStyle.FONT, HudStyle.DIM if n > 0 else HudStyle.OFF_TEXT)
		x += HudStyle.text_w(font, label, HudStyle.FONT) + 5.0
		HudStyle.text(self, bold, Vector2(x, y + base), str(n), HudStyle.FONT, HudStyle.ACCENT if n > 0 else HudStyle.OFF_ACCENT)
		x += HudStyle.text_w(bold, str(n), HudStyle.FONT) + 16.0


func _pair(font: Font, bold: Font, x: float, by: float, label: String, value: String) -> void:
	HudStyle.text(self, font, Vector2(x, by), label, HudStyle.FONT, HudStyle.DIM)
	HudStyle.text(self, bold, Vector2(x + 64.0, by), value, HudStyle.FONT, HudStyle.TEXT)


func _draw_squad(sid: int) -> void:
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var sq: Dictionary = view.squads_by_id.get(sid, {})
	if sq.is_empty():
		return
	var st: Array = view.squad_status(sq)
	var units: PackedInt32Array = view.squad_units.get(sid, PackedInt32Array())
	var right: String = st[0]
	var common := _common_stance(units)
	if common != "":
		right += " · " + common
	_header(view.squad_label(sid, true), right, HudStyle.ACCENT if st[1] else HudStyle.DIM)
	if view.proxy_soldiers:
		var line := "Men %d/%d · health %d %%" % [int(sq.get("men", 0)), int(sq.get("men_max", 0)), roundi(float(sq.get("health", 1.0)) * 100.0)]
		HudStyle.text(self, font, Vector2(PAD, HEAD + 18.0), line, HudStyle.FONT, HudStyle.TEXT)
		return
	var sel: Node = view.selection
	var ps: float = view.hud.m.pip
	var names: Array = MatchView.MAN_NAMES.get(sq.type, [])
	for row in rows:
		var r: Rect2 = row.rect
		var id: int = row.id
		var x := r.position.x + 4.0
		var rh := r.size.y
		var colw := r.size.x - 4.0
		var picked: bool = id != -1 and sel.is_selected(id)
		if picked:
			HudStyle.box(self, r, HudStyle.ROW_SELECTED, 3)
			draw_rect(Rect2(r.position, Vector2(3, rh)), HudStyle.ACCENT)
		var pr := Rect2(x + 4.0, r.position.y + (rh - ps) * 0.5, ps, ps)
		var by := r.position.y + rh * 0.5 + 5.0
		if id == -1:
			var slot: int = row.slot
			HudStyle.pip(self, pr, false, false, false, false, true)
			var dead_name: String = names[slot] if slot < names.size() else "Man %d" % (slot + 1)
			HudStyle.text(self, font, Vector2(x + 24.0, by), dead_name, HudStyle.FONT, HudStyle.OFF_TEXT)
			continue
		var so: Dictionary = view.soldiers_by_id[id]
		HudStyle.pip(self, pr, picked, view.soldier_wounded(so), view.soldier_pinned(so), bool(so.get("detached", false)), false)
		var hp := "%d" % roundi(float(so.get("hp", 100.0)))
		var f := bold if picked else font
		var nm := HudStyle.fit(f, view.soldier_name(so), colw - 32.0 - HudStyle.text_w(font, "100", HudStyle.FONT), HudStyle.FONT)
		HudStyle.text(self, f, Vector2(x + 24.0, by), nm, HudStyle.FONT, HudStyle.TEXT)
		HudStyle.text(self, font, Vector2(x + colw - 4.0, by), hp, HudStyle.FONT, HudStyle.DIM, 1)


## The stance override every living man of the list shares, as a word
## ("crouch"), or "" when they differ or all are on Auto.
func _common_stance(units: PackedInt32Array) -> String:
	var out := ""
	for id in units:
		var s: String = view.selection.stance_override(id)
		if out == "":
			out = s
		elif out != s:
			return ""
	return "" if out == "auto" else out


func row_at(p: Vector2) -> int:
	for r in rows:
		if r.rect.has_point(p):
			return int(r.id)
	return -1


func _gui_input(event: InputEvent) -> void:
	if view == null or view.input_blocked():
		return
	if not (event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT):
		return
	accept_event()
	var id := row_at(event.position)
	if id == -1:
		return
	var sel: Node = view.selection
	if event.double_click:
		view.jump_camera(view.soldiers_by_id[id].pos)
	elif event.shift_pressed:
		sel.toggle(id)
	else:
		sel.select_only(PackedInt32Array([id]))


func _get_tooltip(at: Vector2) -> String:
	var id := row_at(at)
	if id == -1:
		return ""
	var so: Dictionary = view.soldiers_by_id[id]
	return "%s · %s · %d HP\nClick: select him · Shift: add/remove · Double-click: look" % [view.soldier_name(so), view.soldier_weapon(so), roundi(float(so.get("hp", 100.0)))]
