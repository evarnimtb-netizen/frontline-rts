extends Control
## Roster (HUD v2 console, left): control-group tabs, then one card per own
## squad in two rows. A card shows the squad's name and state, one pip per
## man in slot order and the men count; it grows to fit its text (never
## below an equal share of the row, cards_per_row per row), so names are
## not cut off. Cards with a selected man are framed; the focused squad
## (detail panel) has an accent bar.
##
## Pips are shape-coded (HudStyle.pip): fill = selected, dashed = detached,
## hatch = pinned, half fill = wounded, X = dead.
##
## Click a card to select the whole squad (Shift adds / removes it), click
## a pip to select that man (Shift toggles), double-click a card to look at
## the squad. Click a group tab to recall the group. When the squads do not
## fit, squads with selected men keep their cards and a "+N" tile counts
## the rest.

const MORE_W := 44.0
const PAD := 10.0

var view: MatchView
## Laid-out cards: {squad, rect, pips: [[Rect2, soldier id or -1]]}.
var cards: Array = []
## Laid-out group tabs: {group, rect, active}.
var tabs: Array = []
var hidden_count := 0
var _more_rect := Rect2()
var _hover_squad := -1


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_STOP
	tooltip_text = " "


## Only tabs, cards and the "+N" tile take the mouse; the gaps between
## them belong to the world.
func _has_point(p: Vector2) -> bool:
	if _more_rect.has_point(p):
		return true
	for t in tabs:
		if t.rect.has_point(p):
			return true
	for c in cards:
		if c.rect.has_point(p):
			return true
	return false


func update_state(_s: Dictionary) -> void:
	_layout()
	queue_redraw()


func _m() -> Dictionary:
	return view.hud.m


func _own_squads() -> PackedInt32Array:
	var out := PackedInt32Array()
	for sq in view.state.get("squads", []):
		if int(sq.owner) == view.local_player:
			out.append(int(sq.id))
	out.sort()
	return out


## Natural card width: header text or pip row, whichever is wider.
func card_width(sid: int, font: Font, bold: Font) -> float:
	var m := _m()
	var sq: Dictionary = view.squads_by_id[sid]
	var st: Array = view.squad_status(sq)
	var head := HudStyle.text_w(bold, view.squad_label(sid), HudStyle.FONT) + 14.0 + HudStyle.text_w(font, st[0], HudStyle.FONT)
	var men_max := maxi(int(sq.get("men_max", 1)), 1)
	var pips := men_max * (float(m.pip) + float(m.pip_gap)) - float(m.pip_gap) + 12.0 + HudStyle.text_w(font, "%d/%d" % [men_max, men_max], HudStyle.FONT)
	return maxf(head, pips) + PAD + 8.0


func _layout() -> void:
	cards.clear()
	tabs.clear()
	var m := _m()
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var sel: Node = view.selection
	# Group tabs: stored groups with a living man.
	var x := 0.0
	for g in range(1, 10):
		var alive := PackedInt32Array()
		for id in sel.groups[g]:
			if view.own_soldier(id):
				alive.append(id)
		if alive.is_empty():
			continue
		alive.sort()
		tabs.append({"group": g, "rect": Rect2(x, 0, 28, float(m.tab_h)), "active": alive == sel.selected})
		x += 32.0
	# Cards.
	var gap: float = m.card_gap
	var cols: int = m.cards_per_row
	var min_w := (size.x - (cols - 1) * gap) / cols
	var top := float(m.tab_h) + 6.0
	var ch := (size.y - top - gap) * 0.5
	var squads := _own_squads()
	var widths := {}
	for sid in squads:
		widths[sid] = clampf(card_width(sid, font, bold), min_w, size.x)
	var shown := squads
	hidden_count = 0
	if _rows_needed(squads, widths, 0.0) > 2:
		# Keep squads with selected men first, then the rest by id.
		var marked: PackedInt32Array = sel.selected_squads()
		var order := PackedInt32Array()
		for sid in squads:
			if marked.has(sid):
				order.append(sid)
		for sid in squads:
			if not marked.has(sid):
				order.append(sid)
		shown = PackedInt32Array()
		for sid in order:
			var trial := shown.duplicate()
			trial.append(sid)
			trial.sort()
			if _rows_needed(trial, widths, MORE_W + gap) <= 2:
				shown = trial
		hidden_count = squads.size() - shown.size()
	var cx := 0.0
	var row := 0
	for sid in shown:
		var w: float = widths[sid]
		if cx > 0.0 and cx + w > size.x + 0.5:
			cx = 0.0
			row += 1
		var r := Rect2(cx, top + row * (ch + gap), w, ch)
		cards.append({"squad": sid, "rect": r, "pips": _pips(sid, r)})
		cx += w + gap
	_more_rect = Rect2()
	if hidden_count > 0:
		if cx + MORE_W > size.x + 0.5:
			cx = 0.0
			row += 1
		_more_rect = Rect2(cx, top + row * (ch + gap), MORE_W, ch)


func _rows_needed(ids: PackedInt32Array, widths: Dictionary, tail: float) -> int:
	var gap: float = _m().card_gap
	var rows := 1
	var cx := 0.0
	for sid in ids:
		var w: float = widths[sid]
		if cx > 0.0 and cx + w > size.x + 0.5:
			rows += 1
			cx = 0.0
		cx += w + gap
	if tail > 0.0 and cx + tail - gap > size.x + 0.5:
		rows += 1
	return rows


## Pip rects in slot order with the soldier in that slot (-1 = dead).
func _pips(sid: int, r: Rect2) -> Array:
	var m := _m()
	var sq: Dictionary = view.squads_by_id[sid]
	var men_max := maxi(int(sq.get("men_max", 1)), 1)
	var ps: float = m.pip
	var y := r.end.y - ps - 7.0
	var by_slot := {}
	var units: PackedInt32Array = view.squad_units.get(sid, PackedInt32Array())
	if view.proxy_soldiers:
		for i in int(sq.get("men", 0)):
			by_slot[i] = units[0] if not units.is_empty() else -1
	else:
		var next_free := 0
		for id in units:
			var slot := int(view.soldiers_by_id[id].get("slot", -1))
			if slot < 0 or by_slot.has(slot):
				while by_slot.has(next_free):
					next_free += 1
				slot = next_free
			by_slot[slot] = id
	var out: Array = []
	for i in men_max:
		var pr := Rect2(r.position.x + PAD + i * (ps + float(m.pip_gap)), y, ps, ps)
		out.append([pr, int(by_slot.get(i, -1))])
	return out


func _draw() -> void:
	if view == null:
		return
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var sel: Node = view.selection
	for t in tabs:
		var r: Rect2 = t.rect
		HudStyle.box(self, r, HudStyle.BG_TAB_ON if t.active else HudStyle.BG_PANEL, 4)
		if t.active:
			draw_rect(Rect2(r.position.x, r.end.y - 2.0, r.size.x, 2.0), HudStyle.ACCENT)
		HudStyle.text(self, bold, Vector2(r.get_center().x, r.position.y + 16.0), str(t.group), HudStyle.FONT, HudStyle.TEXT, 0)
	var focus: int = sel.focused_squad()
	var marked: PackedInt32Array = sel.selected_squads()
	for c in cards:
		var sid: int = c.squad
		var r: Rect2 = c.rect
		var sq: Dictionary = view.squads_by_id[sid]
		HudStyle.box(self, r, HudStyle.BG_PANEL, 5)
		if sid == _hover_squad:
			HudStyle.box(self, r, HudStyle.HOVER, 5)
		if marked.has(sid):
			HudStyle.box(self, r.grow(-0.5), Color(0, 0, 0, 0), 5, Color(HudStyle.ACCENT, 0.65), 1)
		if sid == focus:
			draw_rect(Rect2(r.position, Vector2(3, r.size.y)), HudStyle.ACCENT)
		var st: Array = view.squad_status(sq)
		var status_w := HudStyle.text_w(font, st[0], HudStyle.FONT)
		var title := HudStyle.fit(bold, view.squad_label(sid), r.size.x - PAD - 8.0 - status_w - 14.0, HudStyle.FONT)
		HudStyle.text(self, bold, r.position + Vector2(PAD, 18.0), title, HudStyle.FONT, HudStyle.TEXT)
		HudStyle.text(self, font, Vector2(r.end.x - 8.0, r.position.y + 18.0), st[0], HudStyle.FONT, HudStyle.ACCENT if st[1] else HudStyle.DIM, 1)
		for p in c.pips:
			_draw_pip(p[0], int(p[1]))
		var men := int(sq.get("men", 0))
		var count := "%d/%d" % [men, int(sq.get("men_max", men))]
		var pr: Rect2 = c.pips[0][0] if not c.pips.is_empty() else r
		HudStyle.text(self, font, Vector2(r.end.x - 8.0, pr.end.y - 1.0), count, HudStyle.FONT, HudStyle.TEXT, 1)
	if hidden_count > 0:
		HudStyle.box(self, _more_rect, HudStyle.BG_PANEL, 5)
		HudStyle.text(self, bold, _more_rect.get_center() + Vector2(0, 5), "+%d" % hidden_count, HudStyle.FONT, HudStyle.TEXT, 0)


func _draw_pip(r: Rect2, id: int) -> void:
	if id == -1:
		HudStyle.pip(self, r, false, false, false, false, true)
		return
	var sel: Node = view.selection
	if view.proxy_soldiers:
		HudStyle.pip(self, r, sel.is_selected(id), false, false, false, false)
		return
	var so: Dictionary = view.soldiers_by_id[id]
	HudStyle.pip(self, r, sel.is_selected(id), view.soldier_wounded(so), view.soldier_pinned(so), bool(so.get("detached", false)), false)


## [squad id, soldier id or -1] under a point; squad -1 = no card.
func hit(p: Vector2) -> Array:
	for c in cards:
		if not c.rect.has_point(p):
			continue
		for pp in c.pips:
			if pp[0].grow(1.5).has_point(p):
				return [c.squad, pp[1]]
		return [c.squad, -1]
	return [-1, -1]


func card_rect(squad_id: int) -> Rect2:
	for c in cards:
		if c.squad == squad_id:
			return c.rect
	return Rect2()


func _gui_input(event: InputEvent) -> void:
	if view == null or view.input_blocked():
		return
	if event is InputEventMouseMotion:
		var h: int = hit(event.position)[0]
		if h != _hover_squad:
			_hover_squad = h
			queue_redraw()
		return
	if not (event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT):
		return
	var sel: Node = view.selection
	for t in tabs:
		if t.rect.has_point(event.position):
			sel.recall(t.group)
			accept_event()
			return
	var h := hit(event.position)
	var sid: int = h[0]
	if sid == -1:
		return
	accept_event()
	sel.focus_squad = sid
	if event.double_click:
		view.jump_camera(view.squads_by_id[sid].pos)
		return
	var man: int = h[1]
	if man != -1 and not view.proxy_soldiers:
		if event.shift_pressed:
			sel.toggle(man)
		else:
			sel.select_only(PackedInt32Array([man]))
	elif event.shift_pressed:
		sel.toggle_squad(sid)
	else:
		sel.select_squads(PackedInt32Array([sid]))


func _notification(what: int) -> void:
	if what == NOTIFICATION_MOUSE_EXIT and _hover_squad != -1:
		_hover_squad = -1
		queue_redraw()


func _get_tooltip(at: Vector2) -> String:
	if _more_rect.has_point(at):
		return "%d more squads (select them in the world to bring their cards up)." % hidden_count
	var h := hit(at)
	if h[0] == -1:
		for t in tabs:
			if t.rect.has_point(at):
				return "Group %d  [%d; Ctrl+%d stores]" % [t.group, t.group, t.group]
		return ""
	if h[1] != -1 and not view.proxy_soldiers:
		var so: Dictionary = view.soldiers_by_id[h[1]]
		return "%s · %d HP\nClick: select him · Shift: add/remove" % [view.soldier_name(so), roundi(float(so.get("hp", 100.0)))]
	return "%s\nClick: select the squad · Shift: add/remove · Double-click: look" % view.squad_label(h[0], true)
