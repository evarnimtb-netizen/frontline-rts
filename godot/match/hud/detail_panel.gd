extends Control
## Detail panel (HUD v2 console, middle). One selected soldier: his
## readout (health, stance and override, state, weapon, order target,
## items). Otherwise the focused squad: its men as clickable rows in pip
## order (row-first, two columns), the selected ones highlighted. Click a
## row to select that man (Shift toggles), double-click to look at him.
## Nothing selected: a short how-to.
##
## S3: a focused own vehicle (the only selection, or the vehicle whose card
## was clicked) shows its card's data at full size: name and state badge,
## the crew seats with their letters, the six components as labelled bars
## (the one being repaired outlined), the ammo with the loaded round, the
## ammo choice (N cycles it) and the reload. No penetration hints about
## enemies (show_pen_hint off, §5.12.4).
##
## S3c: a crew squad whose tank stands abandoned is headed "Crew — Panzer
## IV" with a "To tank" button that jumps the camera to the vehicle.
##
## S4: the squad card's rows name each man by his member slot (the
## state's soldier `name`, the squad's `slot_names` for the fallen) and
## his weapon; the man carrying a crew weapon (MG, mortar, launcher) has an
## accent diamond, and a man with a Panzerfaust a "Pzf" mark.
##
## S5b: an own mortar on a fire mission gets a mission strip under its men
## (the free fourth row): the mode tag (DIR / OBS / MAP), the ranging line
## (rounds seen land, aim error now, flight time of the last round) and a
## ranging bar (how far the error has come down from the first round's);
## the header's status gives the phase and the rounds left.

const PAD := 10.0
const HEAD := 27.0
const ROSTER := preload("res://match/hud/roster.gd")

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


## What the panel shows: "vehicle", "soldier", "squad" or "none".
func mode() -> String:
	var sel: Node = view.selection
	if sel.focused_vehicle() != -1:
		return "vehicle"
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
		"vehicle":
			_draw_vehicle(view.selection.focused_vehicle())
		"soldier":
			_draw_soldier(view.soldiers_by_id[view.selection.selected[0]])
		"squad":
			_draw_squad(view.selection.focused_squad())
		_:
			_draw_help()


func _header(left: String, right: String, right_col := HudStyle.DIM, reserve := 0.0) -> void:
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var rw := HudStyle.text_w(font, right, HudStyle.FONT) + reserve
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
	# S5a: the weapon in hand and the rounds he holds for it.
	var wv := view.soldier_weapon(so)
	if int(so.get("rounds", -1)) >= 0:
		wv = "%s · %d rds" % [String(so.get("active_weapon", wv)), int(so.rounds)]
	_pair(font, bold, x0, y + base, "Weapon", wv)
	if so.has("target"):
		var t: Vector3 = so.target
		var d := Vector2(t.x, t.z).distance_to(Vector2(so.pos.x, so.pos.z))
		var order: String = MatchView.ORDER_TEXT.get(so.get("order", "move"), "Order")
		HudStyle.text(self, font, Vector2(size.x - PAD, y + base), "%s %d m" % [order, roundi(d)], HudStyle.FONT, HudStyle.DIM, 1)
	y += rh
	# Items.
	var x := x0
	var items: Array = MatchView.ITEMS.duplicate()
	if view.item_count(so, "panzerfaust") > 0:
		items.append("panzerfaust")
	for item in items:
		var n := view.item_count(so, item)
		var label: String = item.capitalize()
		HudStyle.text(self, font, Vector2(x, y + base), label, HudStyle.FONT, HudStyle.DIM if n > 0 else HudStyle.OFF_TEXT)
		x += HudStyle.text_w(font, label, HudStyle.FONT) + 5.0
		HudStyle.text(self, bold, Vector2(x, y + base), str(n), HudStyle.FONT, HudStyle.ACCENT if n > 0 else HudStyle.OFF_ACCENT)
		x += HudStyle.text_w(bold, str(n), HudStyle.FONT) + 16.0


func _draw_vehicle(vid: int) -> void:
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var v: Dictionary = view.vehicles_by_id[vid]
	var b: Array = Vehicles.badge(v)
	var right: String = b[0]
	var fol := int(v.get("followers", 0))
	if fol > 0:
		right += " · escort %d" % fol
	_header(view.vehicle_label(vid), right, HudStyle.ACCENT if b[1] else HudStyle.DIM)
	var rh := (size.y - HEAD - 6.0) / 4.0
	var y := HEAD
	var base := rh * 0.5 + 5.0
	var ps: float = view.hud.m.pip
	# Crew: pip and seat letter per seat, D G C L R.
	HudStyle.text(self, font, Vector2(PAD, y + base), "Crew", HudStyle.FONT, HudStyle.DIM)
	var crew: PackedStringArray = v.get("crew", PackedStringArray())
	var x := PAD + 50.0
	var step := minf((size.x - x - PAD) / 5.0, 36.0)
	for i in Vehicles.SEATS.size():
		var st := crew[i] if i < crew.size() else "ok"
		if st != "none":
			ROSTER.crew_pip(self, Rect2(x, y + (rh - ps) * 0.5, ps, ps), st, true)
			HudStyle.text(self, bold, Vector2(x + ps + 3.0, y + base), Vehicles.SEAT_LETTERS[i], HudStyle.FONT_SMALL, HudStyle.TEXT if st in ["ok", "wounded"] else HudStyle.OFF_TEXT)
		x += step
	y += rh
	# Components: four columns, two rows (S3b: seven parts with the
	# transmission); label and a bar.
	var cols := 4
	var colw := (size.x - 2.0 * PAD) / float(cols)
	var rep_c := String(v.get("repairing", ""))
	for i in Vehicles.COMPONENTS.size():
		var c: String = Vehicles.COMPONENTS[i]
		var cx := PAD + (i % cols) * colw
		var cy := y + int(i / float(cols)) * rh
		var label: String = Vehicles.COMPONENT_SHORT[c]
		var lw := HudStyle.text_w(font, label, HudStyle.FONT)
		HudStyle.text(self, font, Vector2(cx, cy + base), label, HudStyle.FONT, HudStyle.DIM)
		var f := Vehicles.component(v, c)
		var bar := Rect2(cx + lw + 5.0, cy + rh * 0.5 - 3.0, colw - lw - 13.0, 7.0)
		HudStyle.box(self, bar, Color(0, 0, 0, 0.35), 2)
		var col := HudStyle.PIP_BRIGHT if f >= Vehicles.DAMAGED else (HudStyle.ACCENT if f > 0.0 else HudStyle.DANGER)
		if f <= 0.0:
			HudStyle.box(self, bar, Color(HudStyle.DANGER, 0.35), 2)
		else:
			HudStyle.box(self, Rect2(bar.position, Vector2(bar.size.x * clampf(f, 0.0, 1.0), bar.size.y)), col, 2)
		if rep_c == c:
			HudStyle.box(self, bar.grow(2.0), Color(0, 0, 0, 0), 3, HudStyle.ACCENT, 1)
	y += 2.0 * rh
	# Ammo: counts with the loaded round, the choice (N), the reload.
	var w := ROSTER.draw_ammo(self, font, bold, Vector2(PAD - 2.0, y + base), v, -1)
	# S3c (B9): MG stowage left (amber under a quarter, red when dry).
	var mg: Array = ROSTER.mg_label(v.get("ammo", {}))
	if mg[0] != "":
		HudStyle.text(self, bold, Vector2(PAD + w + 4.0, y + base), mg[0], HudStyle.FONT, mg[1])
		w += HudStyle.text_w(bold, mg[0], HudStyle.FONT) + 8.0
	var choice: String = view.selection.ammo_choice() if view.selection.is_selected(vid) else String(v.get("ammo_choice", "auto"))
	var chip := "%s N" % Vehicles.AMMO_TEXT.get(choice, "Auto")
	var cw := HudStyle.text_w(bold, chip, HudStyle.FONT_SMALL) + 10.0
	var chip_r := Rect2(PAD + w + 8.0, y + rh * 0.5 - 9.0, cw, 18.0)
	HudStyle.box(self, chip_r, HudStyle.BG_TAB_ON, 3, Color(HudStyle.ACCENT, 0.7), 1)
	HudStyle.text(self, bold, Vector2(chip_r.position.x + 5.0, y + base - 1.0), chip, HudStyle.FONT_SMALL, HudStyle.TEXT)
	var reload := clampf(float(v.get("reload", 1.0)), 0.0, 1.0)
	var rx := chip_r.end.x + 8.0
	var rbar := Rect2(rx, y + rh * 0.5 - 3.0, size.x - PAD - rx, 7.0)
	if rbar.size.x >= 24.0:
		HudStyle.box(self, rbar, Color(0, 0, 0, 0.35), 2)
		HudStyle.box(self, Rect2(rbar.position, Vector2(rbar.size.x * reload, rbar.size.y)), HudStyle.TEXT if reload >= 1.0 else HudStyle.ACCENT, 2)


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
	# S5b: a mortar on a mission shows its phase and rounds left instead.
	if sq.has("mission") and st[0] == "Fire mission":
		right = mission_status(sq.mission)
	var title := view.squad_label(sid, true)
	if sq.has("crew_of"):
		title = "Crew — %s" % String(sq.get("crew_of_name", "tank"))
	# S5a: tags left of the status (LOW / DRY, HOLD, Refilling) and a thin
	# ammunition bar under the header; the title fits what is left.
	var x0r := size.x - PAD - HudStyle.text_w(font, right, HudStyle.FONT) - 8.0
	var tx := x0r
	var at: Array = HudStyle.ammo_tag(String(sq.get("ammo_state", "")))
	if at[0] != "":
		tx -= HudStyle.tag(self, bold, tx, 19.0, at[0], at[1]) + 4.0
	if String(sq.get("fire_mode", "")) == "hold":
		tx -= HudStyle.tag(self, bold, tx, 19.0, "HOLD", HudStyle.OWN) + 4.0
	if bool(sq.get("refilling", false)):
		tx -= HudStyle.tag(self, bold, tx, 19.0, "Refilling", HudStyle.DIM) + 4.0
	_header(title, right, HudStyle.ACCENT if st[1] else HudStyle.DIM, x0r - tx)
	if sq.has("ammo_pct"):
		var ap := clampf(float(sq.ammo_pct), 0.0, 1.0)
		var bar := Rect2(PAD, HEAD - 4.0, size.x - 2.0 * PAD, 2.0)
		draw_rect(bar, Color(0, 0, 0, 0.35))
		var bc: Color = at[1] if at[0] != "" else HudStyle.PIP_MID
		draw_rect(Rect2(bar.position, Vector2(bar.size.x * ap, bar.size.y)), bc)
	var jr := crew_jump_rect()
	if jr.size != Vector2.ZERO:
		HudStyle.box(self, jr, HudStyle.BG_BUTTON, 3)
		HudStyle.text(self, font, Vector2(jr.get_center().x, jr.end.y - 5.0), "To tank", HudStyle.FONT_SMALL, HudStyle.TEXT, 0)
	if view.proxy_soldiers:
		var line := "Men %d/%d · health %d %%" % [int(sq.get("men", 0)), int(sq.get("men_max", 0)), roundi(float(sq.get("health", 1.0)) * 100.0)]
		HudStyle.text(self, font, Vector2(PAD, HEAD + 18.0), line, HudStyle.FONT, HudStyle.TEXT)
		return
	_draw_mission(sq)
	var sel: Node = view.selection
	var ps: float = view.hud.m.pip
	var names: Array = MatchView.MAN_NAMES.get(sq.type, [])
	if sq.has("slot_names"):
		names = Array(sq.slot_names)
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
		var f := bold if picked else font
		# S4 marks: the crew weapon's carrier, a Panzerfaust.
		var mark_w := 0.0
		# Owner fix after S5a (S5a-46): health is a thin bar along the
		# row's foot, so it is always shown and costs the name no width;
		# the exact number is in the tooltip.
		_hp_bar(Rect2(x + 24.0, r.end.y - 4.0, colw - 28.0, 2.0), float(so.get("hp", 100.0)))
		var has_pzf: bool = view.item_count(so, "panzerfaust") > 0
		var has_rounds := int(so.get("rounds", -1)) >= 0
		var rw := HudStyle.text_w(bold, "8888 rd", HudStyle.FONT_SMALL) if has_rounds else 0.0
		var hp_col := 4.0
		var mx := x + colw - hp_col
		if has_pzf:
			var pz := "Pzf"
			var pw := HudStyle.text_w(bold, pz, HudStyle.FONT_SMALL)
			HudStyle.text(self, bold, Vector2(mx - pw, by), pz, HudStyle.FONT_SMALL, HudStyle.ACCENT)
			mark_w += pw + 4.0
			mx -= pw + 4.0
		if bool(so.get("crew_weapon", false)):
			var c := Vector2(mx - 5.0, by - 4.5)
			draw_colored_polygon(PackedVector2Array([c + Vector2(0, -4.5), c + Vector2(4.5, 0), c + Vector2(0, 4.5), c + Vector2(-4.5, 0)]), HudStyle.ACCENT)
			mark_w += 13.0
			mx -= 13.0
		# His role word, and his weapon when the row has room for both.
		# S5a: his rounds in their own column left of his health (the
		# crew weapon's carrier: the team's rounds for it), and the weapon
		# in hand (a sidearm while dry: "P 38").
		var room := colw - 24.0 - hp_col - mark_w
		var label := view.soldier_name(so)
		var size := HudStyle.FONT
		var wtxt := String(so.get("weapon", ""))
		var rounds := int(so.get("rounds", -1))
		if rounds >= 0:
			wtxt = String(so.get("active_weapon", wtxt))
			if bool(so.get("crew_weapon", false)) and sq.has("crew_rounds") and not bool(so.get("sidearm", false)):
				wtxt = String(sq.get("crew_weapon_name", wtxt))
				rounds = int(sq.crew_rounds)
			var rtxt := "%d rd" % rounds
			var rcol := HudStyle.DANGER if rounds == 0 else (HudStyle.ACCENT if bool(so.get("sidearm", false)) else HudStyle.DIM)
			HudStyle.text(self, bold, Vector2(mx - 4.0, by), rtxt, HudStyle.FONT_SMALL, rcol, 1)
			room -= rw + 6.0
		if so.has("weapon") and so.has("name"):
			var both := "%s · %s" % [so.name, wtxt]
			if HudStyle.text_w(f, both, HudStyle.FONT) <= room:
				label = both
			elif HudStyle.text_w(f, both, HudStyle.FONT_SMALL) <= room:
				# S4 review: role and weapon in the small size before the
				# weapon is dropped (narrow HUDs).
				label = both
				size = HudStyle.FONT_SMALL
			elif String(so.name).begins_with("Rifleman"):
				# S4 review: a plain rifleman's role says nothing his weapon
				# does not ("Kar98k", "M1 Garand"); leaders and crew roles
				# keep the role word (both are in the tooltip).
				label = wtxt
		elif rounds >= 0:
			label = "%s · %s" % [label, wtxt]
		var nm := HudStyle.fit(f, label, room, size)
		HudStyle.text(self, f, Vector2(x + 24.0, by), nm, size, HudStyle.TEXT)


## A man's health as a thin bar, coloured as on the one-man card (dim
## while unhurt, so a whole squad reads quietly).
## S5b: the mission tag word and colour for a mode.
static func mission_tag(mode: String) -> Array:
	match mode:
		"direct":
			return ["DIR", HudStyle.OWN]
		"observed":
			return ["OBS", HudStyle.ACCENT]
	return ["MAP", HudStyle.DANGER]


## S5b: the mission strip's line, e.g. "Ranging 2 · ±6 m · 14.6 s" (rounds
## seen land, the aim error now, flight time of the last round).
static func mission_line(m: Dictionary) -> String:
	var parts: Array[String] = []
	if String(m.get("mode", "map")) == "map":
		parts.append("No ranging")
	else:
		parts.append("Ranging %d" % int(m.get("seen", 0)))
	parts.append("±%d m" % roundi(float(m.get("error_m", 0.0))))
	if int(m.get("fired", 0)) > 0:
		parts.append("%.1f s" % float(m.get("flight_s", 0.0)))
	return " · ".join(parts)


## S5b: the card header's status for a mortar on a mission: the phase and
## the rounds left, e.g. "For effect · 6 rds".
static func mission_status(m: Dictionary) -> String:
	var phase: String = {"laying": "Laying", "adjusting": "Adjusting", "effect": "For effect"}.get(String(m.get("phase", "")), "Firing")
	return "%s · %d rds" % [phase, int(m.get("rounds_left", 0))]


## S5b: share of the first round's aim error already ranged out (0..1).
static func ranging_progress(m: Dictionary) -> float:
	var e0 := float(m.get("first_error_m", 0.0))
	if e0 <= 0.0:
		return 0.0
	return clampf(1.0 - float(m.get("error_m", e0)) / e0, 0.0, 1.0)


## S5b: the mission strip (module docs).
func _draw_mission(sq: Dictionary) -> void:
	var m: Dictionary = sq.get("mission", {})
	if m.is_empty() or rows.is_empty():
		return
	var last: Rect2 = rows[rows.size() - 1].rect
	var y := last.end.y + 2.0
	var h := last.size.y - 2.0
	if y + h > size.y - 2.0:
		return
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var r := Rect2(PAD - 4.0, y, size.x - 2.0 * PAD + 8.0, h)
	HudStyle.box(self, r, HudStyle.BG_BUTTON, 3)
	var tag: Array = mission_tag(String(m.get("mode", "map")))
	var by := y + h * 0.5 + 4.0
	var tw := HudStyle.text_w(bold, tag[0], HudStyle.FONT_SMALL) + 8.0
	HudStyle.tag(self, bold, r.position.x + 4.0 + tw, by, tag[0], tag[1])
	var tx := r.position.x + tw + 10.0
	HudStyle.text(self, font, Vector2(tx, by), HudStyle.fit(font, mission_line(m), r.end.x - tx - 6.0, HudStyle.FONT_SMALL), HudStyle.FONT_SMALL, HudStyle.TEXT)
	# Ranging bar along the strip's foot.
	var bar := Rect2(tx, r.end.y - 4.0, r.end.x - tx - 6.0, 2.0)
	draw_rect(bar, Color(0, 0, 0, 0.35))
	draw_rect(Rect2(bar.position, Vector2(bar.size.x * ranging_progress(m), bar.size.y)), tag[1])


func _hp_bar(bar: Rect2, hp: float) -> void:
	hp = clampf(hp, 0.0, 100.0)
	draw_rect(bar, Color(0, 0, 0, 0.35))
	var col := HudStyle.PIP_MID if hp >= 99.5 else (HudStyle.TEXT if hp >= MatchView.WOUNDED_HP else (HudStyle.ACCENT if hp >= 30.0 else HudStyle.DANGER))
	draw_rect(Rect2(bar.position, Vector2(bar.size.x * hp / 100.0, bar.size.y)), col)


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


## S3c: the "To tank" button of a crew squad card (zero rect: none).
func crew_jump_rect() -> Rect2:
	if mode() != "squad":
		return Rect2()
	var sq: Dictionary = view.squads_by_id.get(view.selection.focused_squad(), {})
	if not sq.has("crew_of"):
		return Rect2()
	return Rect2(size.x * 0.5 - 8.0, 4.0, 64.0, 18.0)


## S3c: jumps the camera to the focused crew squad's abandoned tank.
func jump_to_crew_tank() -> bool:
	var sq: Dictionary = view.squads_by_id.get(view.selection.focused_squad(), {})
	var v: Dictionary = view.vehicles_by_id.get(int(sq.get("crew_of", -1)), {})
	if v.is_empty():
		return false
	view.jump_camera(v.pos)
	return true


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
	if crew_jump_rect().has_point(event.position):
		jump_to_crew_tank()
		return
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
	if mode() == "vehicle":
		return ROSTER.vehicle_tooltip(view, view.selection.focused_vehicle())
	var id := row_at(at)
	if id == -1:
		return ""
	var so: Dictionary = view.soldiers_by_id[id]
	return "%s · %s · %d HP\nClick: select him · Shift: add/remove · Double-click: look" % [view.soldier_name(so), view.soldier_weapon(so), roundi(float(so.get("hp", 100.0)))]
