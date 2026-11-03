extends Control
## Top bar (HUD v2): one slim translucent strip. Left: faction, manpower
## and income, population. Centre: own tickets and bar, the A–E capture
## chips, enemy bar and tickets. Right: match clock and the menu button.
##
## Chips: owner colour fill; capture progress drawn inside the chip in the
## capturing side's colour; a hatched rest and an accent frame while
## contested. Click a chip to look at the point. A blinking triangle next
## to a side's tickets marks a drain.

var view: MatchView
var _points: Array = []
var _blink := 0.0
var _hover := -2  # chip index, -1 menu, -2 nothing


## S5a: the top-bar flag while a refill waits for manpower.
const NO_MP := "No manpower for ammo"
## S5a: right end of the MP / income / POP block (its tooltip).
var _mp_right := 0.0


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_STOP
	tooltip_text = " "  # enables _get_tooltip


func update_state(s: Dictionary) -> void:
	_points = s.get("points", [])
	_blink += get_process_delta_time()
	queue_redraw()


func _m() -> Dictionary:
	return view.hud.m


func chip_rect(i: int) -> Rect2:
	var m := _m()
	var cw: float = m.chip_w
	var n := _points.size()
	var chips_w := n * cw + maxf(n - 1, 0) * 4.0
	var x0 := size.x * 0.5 - chips_w * 0.5
	return Rect2(x0 + i * (cw + 4.0), 3.0, cw, size.y - 6.0)


func menu_rect() -> Rect2:
	return Rect2(size.x - 100.0, 3.0, 88.0, size.y - 6.0)


## Free x span (start, end) right of the enemy tickets (and their drain
## mark) and left of the clock with room for "PAUSED": where the net
## readout goes (S2). Uses the widest ticket count and clock text the
## rules allow, so the span does not move as numbers change.
func free_span() -> Vector2:
	var m := _m()
	var bold := HudStyle.bold()
	var n := maxi(_points.size(), 5)
	var chips_w := n * float(m.chip_w) + (n - 1) * 4.0
	var tmax := "%d" % int(view.rules().get("tickets_max", 500)) if view != null else "500"
	var x0 := size.x * 0.5 + chips_w * 0.5 + 10.0 + float(m.ticket_w) + 8.0 + HudStyle.text_w(bold, tmax, HudStyle.FONT) + 18.0
	var clock_w := HudStyle.text_w(bold, "88:88", HudStyle.FONT_SMALL)
	var x1 := size.x - 120.0 - clock_w - HudStyle.text_w(bold, "PAUSED", HudStyle.FONT) - 14.0
	return Vector2(x0, maxf(x1, x0))


func _draw() -> void:
	if view == null:
		return
	var m := _m()
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var fs := HudStyle.FONT
	var fs_small := HudStyle.FONT_SMALL
	draw_rect(Rect2(Vector2.ZERO, size), HudStyle.BG_TOP)
	var me := view.player(view.local_player)
	var them := view.player(1 - view.local_player)
	if me.is_empty() or them.is_empty():
		return
	var by := size.y * 0.5 + 5.0
	var rules := view.rules()
	# Left: faction, MP, income, POP.
	var x := 12.0
	var fac: String = (UiPalette.FACTION_SHORT if m.compact else UiPalette.FACTION_NAMES).get(me.faction, me.faction)
	fac = fac.to_upper()
	HudStyle.text(self, bold, Vector2(x, by), fac, fs, HudStyle.TEXT)
	x += HudStyle.text_w(bold, fac, fs) + 16.0
	x = _pair(font, bold, x, by, "MP", "%d" % int(me.manpower))
	HudStyle.text(self, font, Vector2(x, by), "+%.1f/s" % float(me.income), fs, HudStyle.DIM)
	x += HudStyle.text_w(font, "+%.1f/s" % float(me.income), fs) + 16.0
	x = _pair(font, bold, x, by, "POP", "%d/%d" % [int(me.pop), int(rules.get("pop_cap", 120))])
	# S5a: a refill stood paused for lack of manpower.
	_mp_right = x
	if bool(me.get("refill_paused", false)):
		HudStyle.tag(self, bold, x + HudStyle.text_w(bold, NO_MP, HudStyle.FONT_SMALL) + 8.0, by, NO_MP, HudStyle.DANGER)

	# Centre: tickets and chips.
	var tmax := maxf(float(rules.get("tickets_max", 500)), 1.0)
	var bw: float = m.ticket_w
	var first := chip_rect(0) if not _points.is_empty() else Rect2(size.x * 0.5, 3, 0, 0)
	var last := chip_rect(_points.size() - 1) if not _points.is_empty() else first
	var on := fmod(_blink, 1.0) < 0.6
	var own_bar := Rect2(first.position.x - 10.0 - bw, by - 9.0, bw, 8.0)
	HudStyle.box(self, own_bar, Color(0, 0, 0, 0.35), 2)
	HudStyle.box(self, Rect2(own_bar.position, Vector2(bw * clampf(float(me.tickets) / tmax, 0.0, 1.0), 8.0)), HudStyle.OWN, 2)
	var own_t := "%d" % int(me.tickets)
	HudStyle.text(self, bold, Vector2(own_bar.position.x - 8.0, by), own_t, fs, HudStyle.TEXT, 1)
	if float(me.get("ticket_drain", 0.0)) > 0.0 and on:
		_drain_mark(Vector2(own_bar.position.x - 14.0 - HudStyle.text_w(bold, own_t, fs), by - 5.0))
	var foe_bar := Rect2(last.end.x + 10.0, by - 9.0, bw, 8.0)
	HudStyle.box(self, foe_bar, Color(0, 0, 0, 0.35), 2)
	HudStyle.box(self, Rect2(foe_bar.position, Vector2(bw * clampf(float(them.tickets) / tmax, 0.0, 1.0), 8.0)), HudStyle.ENEMY, 2)
	var foe_t := "%d" % int(them.tickets)
	HudStyle.text(self, bold, Vector2(foe_bar.end.x + 8.0, by), foe_t, fs, HudStyle.TEXT)
	if float(them.get("ticket_drain", 0.0)) > 0.0 and on:
		_drain_mark(Vector2(foe_bar.end.x + 14.0 + HudStyle.text_w(bold, foe_t, fs), by - 5.0))
	for i in _points.size():
		_draw_chip(i, bold, by)

	# Right: clock and menu.
	var clock := UiPalette.mmss(view.state.get("time_s", 0.0))
	HudStyle.text(self, bold, Vector2(size.x - 112.0, by), clock, fs_small, HudStyle.TEXT, 1)
	if view.state.get("paused", false):
		HudStyle.text(self, bold, Vector2(size.x - 120.0 - HudStyle.text_w(bold, clock, fs_small), by), "PAUSED", fs, HudStyle.ACCENT, 1)
	var mr := menu_rect()
	HudStyle.box(self, mr, Color(1, 1, 1, 0.18 if _hover == -1 else 0.10), 3)
	HudStyle.text(self, font, Vector2(mr.get_center().x, by), "Menu  Esc", fs_small, HudStyle.TEXT, 0)


## Caption (dim, 13 px badge) and value (bold); returns the next x.
func _pair(font: Font, bold: Font, x: float, by: float, caption: String, value: String) -> float:
	HudStyle.text(self, font, Vector2(x, by), caption, HudStyle.FONT_SMALL, HudStyle.DIM)
	x += HudStyle.text_w(font, caption, HudStyle.FONT_SMALL) + 6.0
	HudStyle.text(self, bold, Vector2(x, by), value, HudStyle.FONT, HudStyle.TEXT)
	return x + HudStyle.text_w(bold, value, HudStyle.FONT) + 10.0


func _drain_mark(c: Vector2) -> void:
	draw_colored_polygon(PackedVector2Array([c + Vector2(-4, -3), c + Vector2(4, -3), c + Vector2(0, 3)]), HudStyle.DANGER)


func _draw_chip(i: int, bold: Font, by: float) -> void:
	var p: Dictionary = _points[i]
	var r := chip_rect(i)
	var local := view.local_player
	var owner_i := int(p.get("owner", -1))
	HudStyle.box(self, r, HudStyle.BG_CHIP if _hover != i else Color(1, 1, 1, 0.18), 3)
	if owner_i >= 0:
		HudStyle.box(self, r, Color(HudStyle.side_color(owner_i, local), 0.75), 3)
	var cap := int(p.get("capturing", -1))
	var prog := clampf(float(p.get("progress", 0.0)), 0.0, 1.0)
	if cap >= 0 and prog > 0.0:
		var fill := Rect2(r.position, Vector2(r.size.x * prog, r.size.y))
		draw_rect(fill, Color(HudStyle.side_color(cap, local), 0.9))
		draw_line(Vector2(fill.end.x, r.position.y), Vector2(fill.end.x, r.end.y), HudStyle.TEXT, 1.0)
	if p.get("contested", false):
		var rest := Rect2(r.position.x + r.size.x * prog, r.position.y, r.size.x * (1.0 - prog), r.size.y)
		HudStyle.hatch(self, rest, Color(HudStyle.TEXT, 0.5), 4.0, 1.4)
		HudStyle.box(self, r.grow(-0.5), Color(0, 0, 0, 0), 3, HudStyle.ACCENT, 2)
	HudStyle.text(self, bold, Vector2(r.get_center().x, by), str(p.id), HudStyle.FONT, Color.WHITE, 0)


func _chip_at(pos: Vector2) -> int:
	for i in _points.size():
		if chip_rect(i).has_point(pos):
			return i
	if menu_rect().has_point(pos):
		return -1
	return -2


func _gui_input(event: InputEvent) -> void:
	if view == null:
		return
	if event is InputEventMouseMotion:
		var h := _chip_at(event.position)
		if h != _hover:
			_hover = h
			queue_redraw()
	elif event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		var i := _chip_at(event.position)
		if i >= 0:
			view.jump_camera(_points[i].pos)
		elif i == -1:
			view.pause_menu.open()
		accept_event()


func _notification(what: int) -> void:
	if what == NOTIFICATION_MOUSE_EXIT and _hover != -2:
		_hover = -2
		queue_redraw()


func _get_tooltip(at: Vector2) -> String:
	# S5a: the left block (MP, income, POP): manpower spent on ammunition.
	if at.x < _mp_right:
		var me := view.player(view.local_player)
		var am: Dictionary = me.get("ammo_mp", {})
		var line := "Manpower %d (+%.1f/s)" % [int(me.get("manpower", 0)), float(me.get("income", 0.0))]
		if not am.is_empty():
			line += "\nSpent on ammunition this match: small arms %d, explosives %d, shells %d MP" % [roundi(float(am.get("small_arms", 0.0))), roundi(float(am.get("explosives", 0.0))), roundi(float(am.get("shells", 0.0)))]
		if bool(me.get("refill_paused", false)):
			line += "\n" + NO_MP + ": the base refill waits for manpower."
		return line
	var i := _chip_at(at)
	if i == -1:
		return "Match menu  [Esc]"
	if i < 0:
		return ""
	var p: Dictionary = _points[i]
	var local := view.local_player
	var owner_i := int(p.get("owner", -1))
	var who := "neutral" if owner_i < 0 else ("held by you" if owner_i == local else "held by the enemy")
	var line := "Point %s, %s" % [p.id, who]
	if p.get("contested", false):
		line += ", contested"
	elif int(p.get("capturing", -1)) >= 0:
		line += ", %s %d %%" % ["you capture" if int(p.capturing) == local else "enemy captures", roundi(float(p.progress) * 100.0)]
	return line + "\nClick to look."
