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
## S3 vehicles get a card of their own in squad-id order (a vehicle is a
## squad of one), in the same two rows and within the same maximum width:
## row one a side-view silhouette drawn from the type's box sizes, the
## name and a state badge (Moving, Bogged, Immobilised, Burning,
## Repairing); row two the crew pips (D G C L R: fill ok, half wounded,
## X dead, dashed swapping), the component strip (engine, left and right
## track, gun, ring: dim ok, accent damaged, red destroyed, outlined while
## being repaired) and `AP 24 · HE 18` with the loaded round in the accent
## colour and a forced ammo choice boxed (Auto: no box).
##
## Click a card to select the whole squad (Shift adds / removes it), click
## a pip to select that man (Shift toggles), double-click a card to look at
## the squad. Click a group tab to recall the group. When the squads do not
## fit, squads with selected men keep their cards, then vehicles (few and
## costly, so they stay on the roster with nothing selected), and a "+N"
## tile counts the rest.

const MORE_W := 44.0
const PAD := 10.0
## Vehicle card pieces (px).
const SIL_W := 30.0
const SEG_W := 7.0
const SEG_GAP := 2.0
const PART_GAP := 10.0

var view: MatchView
## Laid-out cards: {squad, rect, pips: [[Rect2, soldier id or -1]],
## vehicle: vehicle id or -1}.
var cards: Array = []
## Laid-out group tabs: {group, rect, active}.
var tabs: Array = []
var hidden_count := 0
var _more_rect := Rect2()
var _hover_squad := -1
## Own vehicles by their squad id (rebuilt each layout).
var _vehicle_of := {}


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


## Own squads and own vehicles' squad ids, ascending (roster order).
func _own_squads() -> PackedInt32Array:
	var out := PackedInt32Array()
	_vehicle_of.clear()
	for sq in view.state.get("squads", []):
		if int(sq.owner) == view.local_player:
			out.append(int(sq.id))
	for v in view.state.get("vehicles", []):
		if int(v.owner) == view.local_player and not Vehicles.is_wreck(v):
			var sid := int(v.get("squad", -1))
			if sid >= 0:
				_vehicle_of[sid] = int(v.id)
				out.append(sid)
	out.sort()
	return out


## Vehicle id of a card's squad id, or -1 for a soldier squad.
func vehicle_of(sid: int) -> int:
	return int(_vehicle_of.get(sid, -1))


## Natural card width: header text or pip row, whichever is wider.
func card_width(sid: int, font: Font, bold: Font) -> float:
	var vid := vehicle_of(sid)
	if vid != -1:
		return vehicle_card_width(vid, font, bold)
	var m := _m()
	var sq: Dictionary = view.squads_by_id[sid]
	var st: Array = view.squad_status(sq)
	var head := HudStyle.text_w(bold, view.squad_label(sid), HudStyle.FONT) + 14.0 + HudStyle.text_w(font, st[0], HudStyle.FONT)
	var men_max := maxi(int(sq.get("men_max", 1)), 1)
	var pips := men_max * (float(m.pip) + float(m.pip_gap)) - float(m.pip_gap) + 12.0 + HudStyle.text_w(font, "%d/%d" % [men_max, men_max], HudStyle.FONT)
	# S5a: room for a LOW / DRY tag (always, so the card does not twitch).
	if sq.has("ammo_state"):
		pips += HudStyle.text_w(bold, "LOW", HudStyle.FONT_SMALL) + 13.0
	return maxf(head, pips) + PAD + 8.0


## Natural vehicle card width: silhouette, name and badge, or crew pips,
## component strip and ammo text, whichever row is wider. The ammo text is
## measured with two-digit counts so the card does not twitch.
func vehicle_card_width(vid: int, font: Font, bold: Font) -> float:
	var m := _m()
	var v: Dictionary = view.vehicles_by_id[vid]
	var head := SIL_W + 6.0 + HudStyle.text_w(bold, view.vehicle_label(vid), HudStyle.FONT) + 14.0 + HudStyle.text_w(font, Vehicles.badge(v)[0], HudStyle.FONT)
	var pips := Vehicles.SEATS.size() * (float(m.pip) + float(m.pip_gap)) - float(m.pip_gap)
	var strip := Vehicles.STRIP.size() * (SEG_W + SEG_GAP) - SEG_GAP
	var row2 := pips + PART_GAP + strip + PART_GAP + ammo_width(font, bold, 88, 88, Vehicles.has_apcr(v))
	return maxf(head, row2) + PAD + 8.0


## S3b: with `apcr`, the special round's count too ("AP 88 · APCR 8 · HE 88").
static func ammo_width(font: Font, bold: Font, ap: int, he: int, apcr := false) -> float:
	var w := HudStyle.text_w(bold, "AP %d" % ap, HudStyle.FONT) + HudStyle.text_w(font, " · ", HudStyle.FONT) + HudStyle.text_w(bold, "HE %d" % he, HudStyle.FONT) + 4.0
	if apcr:
		w += HudStyle.text_w(bold, "APCR 8/8", HudStyle.FONT) + HudStyle.text_w(font, " · ", HudStyle.FONT)
	return w


func _layout() -> void:
	cards.clear()
	tabs.clear()
	var m := _m()
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var sel: Node = view.selection
	# Group tabs: stored groups with a living man or vehicle.
	var x := 0.0
	for g in range(1, 10):
		var alive := PackedInt32Array()
		for id in sel.groups[g]:
			if view.own_unit(id):
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
		# Keep squads with selected men (and selected vehicles) first, then
		# the other vehicles, then the rest by id.
		var marked := _marked()
		var order := PackedInt32Array()
		for sid in squads:
			if marked.has(sid):
				order.append(sid)
		for sid in squads:
			if not marked.has(sid) and vehicle_of(sid) != -1:
				order.append(sid)
		for sid in squads:
			if not marked.has(sid) and vehicle_of(sid) == -1:
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
		var vid := vehicle_of(sid)
		cards.append({"squad": sid, "rect": r, "pips": _pips(sid, r) if vid == -1 else [], "vehicle": vid})
		cx += w + gap
	_more_rect = Rect2()
	if hidden_count > 0:
		if cx + MORE_W > size.x + 0.5:
			cx = 0.0
			row += 1
		_more_rect = Rect2(cx, top + row * (ch + gap), MORE_W, ch)


## Squad ids of cards with a selected man or a selected vehicle.
func _marked() -> PackedInt32Array:
	var sel: Node = view.selection
	var out: PackedInt32Array = sel.selected_squads()
	for sid in _vehicle_of:
		if sel.is_selected(_vehicle_of[sid]):
			out.append(sid)
	return out


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
	var focus_v: int = sel.focused_vehicle()
	var marked := _marked()
	for c in cards:
		var sid: int = c.squad
		var r: Rect2 = c.rect
		HudStyle.box(self, r, HudStyle.BG_PANEL, 5)
		if sid == _hover_squad:
			HudStyle.box(self, r, HudStyle.HOVER, 5)
		if marked.has(sid):
			HudStyle.box(self, r.grow(-0.5), Color(0, 0, 0, 0), 5, Color(HudStyle.ACCENT, 0.65), 1)
		var vid: int = c.vehicle
		if (vid == -1 and sid == focus) or (vid != -1 and vid == focus_v):
			draw_rect(Rect2(r.position, Vector2(3, r.size.y)), HudStyle.ACCENT)
		if vid != -1:
			_draw_vehicle_card(r, vid, font, bold)
			continue
		var sq: Dictionary = view.squads_by_id[sid]
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
		# S5a: LOW / DRY left of the men count.
		var at: Array = HudStyle.ammo_tag(String(sq.get("ammo_state", "")))
		var tx := r.end.x - 8.0 - HudStyle.text_w(font, count, HudStyle.FONT) - 5.0
		if at[0] != "":
			tx -= HudStyle.tag(self, bold, tx, pr.end.y - 1.0, at[0], at[1]) + 4.0
		# S6: a crate tag ("CRATE" or "CRATE×n") while its men carry any.
		var n := crates_carried(sid)
		if n > 0:
			HudStyle.tag(self, bold, tx, pr.end.y - 1.0, "CRATE" if n == 1 else "CRATE×%d" % n, HudStyle.ACCENT)
	if hidden_count > 0:
		HudStyle.box(self, _more_rect, HudStyle.BG_PANEL, 5)
		HudStyle.text(self, bold, _more_rect.get_center() + Vector2(0, 5), "+%d" % hidden_count, HudStyle.FONT, HudStyle.TEXT, 0)


## S6: crates the men of squad `sid` carry.
func crates_carried(sid: int) -> int:
	var n := 0
	for id in view.squad_units.get(sid, PackedInt32Array()):
		if view.soldiers_by_id.get(id, {}).has("crate"):
			n += 1
	return n


func _draw_vehicle_card(r: Rect2, vid: int, font: Font, bold: Font) -> void:
	var m := _m()
	var v: Dictionary = view.vehicles_by_id[vid]
	var vt := view.vehicle_type(vid)
	var picked: bool = view.selection.is_selected(vid)
	# Row one: silhouette, name, badge.
	var b: Array = Vehicles.badge(v)
	var badge_w := HudStyle.text_w(font, b[0], HudStyle.FONT)
	Vehicles.silhouette(self, Rect2(r.position + Vector2(PAD, 6.0), Vector2(SIL_W, 14.0)), vt, HudStyle.TEXT if picked else HudStyle.DIM)
	var x0 := r.position.x + PAD + SIL_W + 6.0
	# S5a: LOW / DRY from the main-gun rounds, left of the badge.
	var at: Array = HudStyle.ammo_tag(String(v.get("ammo_state", "")))
	var tag_w := 0.0
	if at[0] != "":
		tag_w = HudStyle.tag(self, bold, r.end.x - 8.0 - badge_w - 6.0, r.position.y + 18.0, at[0], at[1]) + 6.0
	var title := HudStyle.fit(bold, view.vehicle_label(vid), r.end.x - 8.0 - badge_w - 14.0 - tag_w - x0, HudStyle.FONT)
	HudStyle.text(self, bold, Vector2(x0, r.position.y + 18.0), title, HudStyle.FONT, HudStyle.TEXT)
	HudStyle.text(self, font, Vector2(r.end.x - 8.0, r.position.y + 18.0), b[0], HudStyle.FONT, HudStyle.ACCENT if b[1] else HudStyle.DIM, 1)
	# Row two: crew pips, component strip, ammo.
	var ps: float = m.pip
	var y := r.end.y - ps - 7.0
	var x := r.position.x + PAD
	var crew: PackedStringArray = v.get("crew", PackedStringArray())
	for i in Vehicles.SEATS.size():
		var st := crew[i] if i < crew.size() else "ok"
		if st != "none":
			crew_pip(self, Rect2(x, y, ps, ps), st, picked)
		x += ps + float(m.pip_gap)
	x += PART_GAP - float(m.pip_gap)
	for c in Vehicles.STRIP:
		component_seg(self, Rect2(x, y, SEG_W, ps), Vehicles.component(v, c), String(v.get("repairing", "")) == c)
		x += SEG_W + SEG_GAP
	draw_ammo(self, font, bold, Vector2(r.end.x - 8.0, y + ps - 1.0), v, 1)


## A crew pip: fill ok, lower half wounded, X dead, dashed swapping.
static func crew_pip(ci: CanvasItem, r: Rect2, st: String, bright: bool) -> void:
	match st:
		"dead":
			HudStyle.pip(ci, r, false, false, false, false, true)
		"wounded":
			HudStyle.pip(ci, r, bright, true, false, false, false)
		"swapping":
			HudStyle.pip(ci, r, bright, false, false, true, false)
		_:
			HudStyle.pip(ci, r, bright, false, false, false, false)


## A component segment: dim ok, accent damaged, red destroyed (with a
## slash); outlined in the accent while the crew repairs it.
static func component_seg(ci: CanvasItem, r: Rect2, f: float, repairing: bool) -> void:
	if f <= 0.0:
		ci.draw_rect(r, Color(HudStyle.DANGER, 0.85))
		ci.draw_line(Vector2(r.position.x, r.end.y), Vector2(r.end.x, r.position.y), Color(0.08, 0.1, 0.12), 1.4)
	elif f < Vehicles.DAMAGED:
		ci.draw_rect(r, HudStyle.ACCENT)
	else:
		ci.draw_rect(r, HudStyle.PIP_MID)
	if repairing:
		ci.draw_rect(r.grow(1.5), HudStyle.ACCENT, false, 1.5)


## `AP 24 · HE 18` ending (align 1) or starting (-1) at `at` (baseline):
## the loaded round in the accent colour, a forced choice boxed. Returns
## the width drawn.
## S3c (B9): the vehicle card's MG stowage text and colour: "MG 2,950",
## amber under a quarter of the full stowage, red "MG dry" at 0; "" when the
## mounts are unlimited (no "mg" key).
static func mg_label(ammo: Dictionary) -> Array:
	if not ammo.has("mg"):
		return ["", HudStyle.TEXT]
	var mg := int(ammo.get("mg", 0))
	if mg <= 0:
		return ["MG dry", HudStyle.DANGER]
	var full := int(ammo.get("mg_full", 0))
	var digits := str(mg)
	var txt := ""
	for i in digits.length():
		if i > 0 and (digits.length() - i) % 3 == 0:
			txt += ","
		txt += digits[i]
	return ["MG " + txt, HudStyle.ACCENT if full > 0 and mg * 4 < full else HudStyle.TEXT]


static func draw_ammo(ci: CanvasItem, font: Font, bold: Font, at: Vector2, v: Dictionary, align: int) -> float:
	var ammo: Dictionary = v.get("ammo", {})
	var parts := ["AP %d" % int(ammo.get("ap", 0)), " · ", "HE %d" % int(ammo.get("he", 0))]
	var keys := ["ap", "", "he"]
	var fonts := [bold, font, bold]
	if ammo.has("apcr"):
		# S3b: the special round between AP and HE; S5a: out of the
		# vehicle's rare_cap ("APCR 2/3").
		var rare := "APCR %d" % int(ammo.get("apcr", 0))
		if ammo.has("rare_cap"):
			rare += "/%d" % int(ammo.rare_cap)
		parts = ["AP %d" % int(ammo.get("ap", 0)), " · ", rare, " · ", "HE %d" % int(ammo.get("he", 0))]
		keys = ["ap", "", "apcr", "", "he"]
		fonts = [bold, font, bold, font, bold]
	var w := 0.0
	for i in parts.size():
		w += HudStyle.text_w(fonts[i], parts[i], HudStyle.FONT)
	var x := at.x - w - 2.0 if align == 1 else at.x + 2.0
	var loaded := String(v.get("loaded", ""))
	var choice := String(v.get("ammo_choice", "auto"))
	for i in parts.size():
		var pw := HudStyle.text_w(fonts[i], parts[i], HudStyle.FONT)
		var col := HudStyle.DIM
		if keys[i] != "":
			col = HudStyle.ACCENT if keys[i] == loaded else HudStyle.TEXT
			if keys[i] == choice:
				HudStyle.box(ci, Rect2(x - 2.0, at.y - 13.0, pw + 4.0, 17.0), Color(0, 0, 0, 0), 3, Color(HudStyle.ACCENT, 0.9), 1)
		HudStyle.text(ci, fonts[i], Vector2(x, at.y), parts[i], HudStyle.FONT, col)
		x += pw
	return w + 4.0


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


## A vehicle's card rect (empty when it has no card shown).
func vehicle_card_rect(vid: int) -> Rect2:
	for c in cards:
		if c.vehicle == vid:
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
	var vid := vehicle_of(sid)
	if vid != -1:
		if event.double_click:
			view.jump_camera(view.vehicles_by_id[vid].pos)
		elif event.shift_pressed:
			sel.toggle(vid)
		else:
			sel.select_only(PackedInt32Array([vid]))
		return
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
	var vid := vehicle_of(h[0])
	if vid != -1:
		return vehicle_tooltip(view, vid) + "\nClick: select · Shift: add/remove · Double-click: look"
	if h[1] != -1 and not view.proxy_soldiers:
		var so: Dictionary = view.soldiers_by_id[h[1]]
		return "%s · %d HP\nClick: select him · Shift: add/remove" % [view.soldier_name(so), roundi(float(so.get("hp", 100.0)))]
	return "%s\nClick: select the squad · Shift: add/remove · Double-click: look" % view.squad_label(h[0], true)


## Text summary of an own vehicle (card and marker tooltips).
static func vehicle_tooltip(mv: MatchView, vid: int) -> String:
	var v: Dictionary = mv.vehicles_by_id[vid]
	var crew: PackedStringArray = v.get("crew", PackedStringArray())
	var seats: Array = []
	for i in mini(crew.size(), Vehicles.SEATS.size()):
		if crew[i] != "none":
			seats.append("%s %s" % [Vehicles.SEAT_LETTERS[i], crew[i]])
	var parts: Array = []
	for c in Vehicles.COMPONENTS:
		parts.append("%s %d%%" % [Vehicles.COMPONENT_SHORT[c], roundi(Vehicles.component(v, c) * 100.0)])
	var ammo: Dictionary = v.get("ammo", {})
	var rounds := "AP %d · HE %d" % [int(ammo.get("ap", 0)), int(ammo.get("he", 0))]
	if ammo.has("apcr"):
		rounds = "AP %d · APCR %d · HE %d" % [int(ammo.get("ap", 0)), int(ammo.get("apcr", 0)), int(ammo.get("he", 0))]
	if ammo.has("mg"):
		# S3b Fix: MG belts left in stowage.
		rounds += " · MG %d" % int(ammo.get("mg", 0))
	return "%s · %s\nCrew: %s\n%s\n%s · loaded %s · ammo %s [N]" % [
		mv.vehicle_label(vid, true), Vehicles.badge(v)[0], ", ".join(seats), ", ".join(parts),
		rounds, String(v.get("loaded", "")).to_upper(),
		Vehicles.AMMO_TEXT.get(String(v.get("ammo_choice", "auto")), "Auto")]
