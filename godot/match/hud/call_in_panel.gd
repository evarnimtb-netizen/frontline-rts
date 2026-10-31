extends Control
## Call-in column (HUD v2), bottom right above the orders, callin_w wide:
## one shared strip with a tile per squad type from state.callin.catalog,
## then one queue bar. Every tile always shows its name and cost; a tile
## the player cannot buy now is dimmed and says why ("need 25" when short
## of manpower). An optional re-call cooldown (catalog `cooldown_s` /
## `cooldown_total_s`) fills the tile from the bottom with a clock and the
## seconds left. The queue bar shows "Next: <unit> <s>", "n/5" and the
## head's progress.
##
## Click a tile to call it in. Tab focuses the column: Up/Down pick a tile,
## Enter or Space calls it in, Tab or Esc leaves (the camera's keys pause
## meanwhile).

const TILE_NAMES := {"rifle": "Rifle squad", "mg": "MG team", "engineer": "Engineers", "mortar": "Mortar"}

var view: MatchView
var focused := false
var cursor := 0
var _catalog: Array = []
var _queue: Array = []
var _queue_max := 5
var _hover := -1


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_STOP
	focus_mode = Control.FOCUS_CLICK
	tooltip_text = " "
	focus_exited.connect(func() -> void: _set_focus(false))


## Column height for a metrics set (the HUD layout calls this).
func column_height(m: Dictionary) -> float:
	var n := maxi(view.state.get("callin", {}).get("catalog", []).size(), 1)
	return _strip_h(m, n) + 4.0 + float(m.ci_q)


func _strip_h(m: Dictionary, n: int) -> float:
	return float(m.ci_head) + n * float(m.tile_h)


func update_state(s: Dictionary) -> void:
	var ci: Dictionary = s.get("callin", {})
	var before := _catalog.size()
	_catalog = ci.get("catalog", [])
	_queue = ci.get("queue", [])
	_queue_max = int(ci.get("queue_max", 5))
	if _catalog.size() != before:
		view.hud.relayout()
	cursor = clampi(cursor, 0, maxi(_catalog.size() - 1, 0))
	queue_redraw()


func tile_rect(i: int) -> Rect2:
	var m: Dictionary = view.hud.m
	return Rect2(0, float(m.ci_head) + i * float(m.tile_h), size.x, float(m.tile_h))


## Why a catalog entry cannot be called in now ("" = it can).
func blocked_reason(c: Dictionary) -> String:
	if view.input_blocked():
		return "Paused."
	var me := view.player(view.local_player)
	var short := int(c.cost) - int(me.get("manpower", 0.0))
	if short > 0:
		return "need %d" % short
	if _queue.size() >= _queue_max:
		return "queue full"
	var queued := 0
	for q in _queue:
		queued += int(q.get("men", 0))
	if int(me.get("pop", 0)) + queued + int(c.get("men", 0)) > int(view.rules().get("pop_cap", 120)):
		return "pop cap"
	if float(c.get("cooldown_s", 0.0)) > 0.0:
		return "cooldown"
	return ""


## Calls in catalog entry `i` (tile click, Enter); false when blocked.
func call_in_index(i: int) -> bool:
	if i < 0 or i >= _catalog.size():
		return false
	var c: Dictionary = _catalog[i]
	if blocked_reason(c) != "":
		AudioManager.play_ui(&"error")
		return false
	view.emit_intent(&"call_in", {"type": c.type})
	AudioManager.play_ui(&"confirm")
	return true


## Index of a squad type in the catalog, or -1.
func index_of(type: String) -> int:
	for i in _catalog.size():
		if _catalog[i].type == type:
			return i
	return -1


func toggle_focus() -> void:
	_set_focus(not focused)


func _set_focus(on: bool) -> void:
	if on == focused:
		return
	focused = on
	if on and is_inside_tree() and is_visible_in_tree():
		grab_focus()
	elif not on and has_focus():
		release_focus()
	if view != null and view.camera != null:
		view.camera.set("input_enabled", not on)
	queue_redraw()


func _draw() -> void:
	if view == null:
		return
	var m: Dictionary = view.hud.m
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var w := size.x
	var strip := Rect2(0, 0, w, _strip_h(m, _catalog.size()))
	HudStyle.box(self, strip, HudStyle.BG_CALLIN, 5)
	if focused:
		HudStyle.box(self, strip.grow(1.0), Color(0, 0, 0, 0), 5, HudStyle.ACCENT, 2)
	HudStyle.text(self, bold, Vector2(8, float(m.ci_head) - 4.0), "CALL-IN", HudStyle.FONT, HudStyle.DIM)
	HudStyle.text(self, font, Vector2(w - 8, float(m.ci_head) - 4.0), "Tab", HudStyle.FONT_SMALL, HudStyle.ACCENT if focused else HudStyle.DIM, 1)
	for i in _catalog.size():
		_draw_tile(i, font, bold)
	# Queue bar.
	var qy := strip.size.y + 4.0
	var qh: float = m.ci_q
	var qr := Rect2(0, qy, w, qh)
	HudStyle.box(self, qr, HudStyle.BG_CALLIN, 5)
	var count := "%d/%d" % [_queue.size(), _queue_max]
	HudStyle.text(self, bold, Vector2(w - 6, qy + 15.0), count, HudStyle.FONT_SMALL, HudStyle.TEXT, 1)
	var avail := w - 18.0 - HudStyle.text_w(bold, count, HudStyle.FONT_SMALL)
	if _queue.is_empty():
		HudStyle.text(self, font, Vector2(6, qy + 15.0), "Queue empty", HudStyle.FONT, HudStyle.OFF_TEXT)
		return
	var head: Dictionary = _queue[0]
	# "Next: Rifle" (14) then the time (13, a time). Drop "Next:" when the
	# column is too narrow for both.
	var unit: String = MatchView.SHORT_NAMES.get(head.type, head.type)
	var secs := "%ds" % ceili(float(head.get("remaining_s", 0.0)))
	var label := "Next: " + unit
	var secs_w := HudStyle.text_w(bold, secs, HudStyle.FONT_SMALL) + 4.0
	if HudStyle.text_w(font, label, HudStyle.FONT) + secs_w > avail:
		label = unit
	label = HudStyle.fit(font, label, avail - secs_w, HudStyle.FONT)
	HudStyle.text(self, font, Vector2(6, qy + 15.0), label, HudStyle.FONT, HudStyle.TEXT)
	HudStyle.text(self, bold, Vector2(10.0 + HudStyle.text_w(font, label, HudStyle.FONT), qy + 15.0), secs, HudStyle.FONT_SMALL, HudStyle.TEXT)
	var total := maxf(float(head.get("total_s", 1.0)), 0.01)
	var frac := clampf(1.0 - float(head.get("remaining_s", 0.0)) / total, 0.0, 1.0)
	var bar := Rect2(6, qy + qh - 6.0, w - 12.0, 3.0)
	draw_rect(bar, Color(1, 1, 1, 0.18))
	draw_rect(Rect2(bar.position, Vector2(bar.size.x * frac, bar.size.y)), HudStyle.ACCENT)


func _draw_tile(i: int, font: Font, bold: Font) -> void:
	var c: Dictionary = _catalog[i]
	var r := tile_rect(i)
	var why := blocked_reason(c)
	var off := why != ""
	draw_rect(Rect2(8, r.position.y, r.size.x - 16, 1), Color(HudStyle.TEXT, 0.14))
	var cd := float(c.get("cooldown_s", 0.0))
	if cd > 0.0:
		var frac := clampf(cd / maxf(float(c.get("cooldown_total_s", cd)), 0.01), 0.0, 1.0)
		draw_rect(Rect2(1, r.end.y - r.size.y * frac, r.size.x - 2, r.size.y * frac - 1), Color(HudStyle.TEXT, 0.12))
	if (_hover == i and not off) or (focused and cursor == i):
		draw_rect(r.grow_individual(-1, -1, -1, -1), HudStyle.HOVER)
	if focused and cursor == i:
		draw_rect(Rect2(r.position + Vector2(0, 2), Vector2(3, r.size.y - 4)), HudStyle.ACCENT)
	var title := HudStyle.fit(bold, _short(c.type, c.get("short", "")), r.size.x - 16.0, HudStyle.FONT)
	HudStyle.text(self, bold, Vector2(8, r.position.y + 16.0), title, HudStyle.FONT, HudStyle.OFF_TEXT if off else HudStyle.TEXT)
	var cost := "%d MP" % int(c.cost)
	HudStyle.text(self, bold, Vector2(8, r.end.y - 5.0), cost, HudStyle.FONT, HudStyle.OFF_ACCENT if off else HudStyle.ACCENT)
	if cd > 0.0:
		var secs := "%ds" % ceili(cd)
		HudStyle.text(self, bold, Vector2(r.size.x - 8.0, r.end.y - 5.0), secs, HudStyle.FONT_SMALL, HudStyle.TEXT, 1)
		HudStyle.clock(self, Vector2(r.size.x - 14.0 - HudStyle.text_w(bold, secs, HudStyle.FONT_SMALL) - 6.0, r.end.y - 10.0), 5.5, HudStyle.TEXT)
	elif off:
		# The reason, if it fits next to the cost (always at 120 px).
		var room := r.size.x - 20.0 - HudStyle.text_w(bold, cost, HudStyle.FONT)
		if HudStyle.text_w(font, why, HudStyle.FONT) <= room:
			HudStyle.text(self, font, Vector2(r.size.x - 8.0, r.end.y - 5.0), why, HudStyle.FONT, Color(HudStyle.TEXT, 0.75), 1)


## Tile name (catalog `short`, else TILE_NAMES); the queue bar uses the
## card names ("Next: Rifle 7s").
func _short(type: String, given := "") -> String:
	if given != "":
		return given
	return TILE_NAMES.get(type, MatchView.SHORT_NAMES.get(type, type))


func _tile_at(p: Vector2) -> int:
	for i in _catalog.size():
		if tile_rect(i).has_point(p):
			return i
	return -1


func _gui_input(event: InputEvent) -> void:
	if view == null:
		return
	if event is InputEventMouseMotion:
		var h := _tile_at(event.position)
		if h != _hover:
			_hover = h
			queue_redraw()
	elif event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		var i := _tile_at(event.position)
		if i >= 0:
			cursor = i
			call_in_index(i)
		accept_event()
	elif event is InputEventKey and event.pressed and focused:
		match event.physical_keycode:
			KEY_UP:
				cursor = posmod(cursor - 1, maxi(_catalog.size(), 1))
			KEY_DOWN:
				cursor = posmod(cursor + 1, maxi(_catalog.size(), 1))
			KEY_ENTER, KEY_KP_ENTER, KEY_SPACE:
				call_in_index(cursor)
			KEY_TAB, KEY_ESCAPE:
				_set_focus(false)
			_:
				return
		queue_redraw()
		accept_event()


func _notification(what: int) -> void:
	if what == NOTIFICATION_MOUSE_EXIT and _hover != -1:
		_hover = -1
		queue_redraw()


func _get_tooltip(at: Vector2) -> String:
	var i := _tile_at(at)
	if i < 0:
		return "Call-in  [Tab]: Up/Down pick, Enter calls in."
	var c: Dictionary = _catalog[i]
	var why := blocked_reason(c)
	return "%s: %d men, %d manpower, arrives at base after %d s.%s" % [c.name, int(c.men), int(c.cost), int(c.delay_s), "" if why == "" else "\nNot now: " + why]
