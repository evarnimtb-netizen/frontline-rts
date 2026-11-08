extends Control
## Call-in column (HUD v2), bottom right above the orders, callin_w wide:
## one strip with a tile per squad type from state.callin.catalog.
##
## S3b (owner, 2026-11-01): no queue and no delivery time. A call-in buys
## at once and the squad spawns in the base; then that type waits out its
## re-call cooldown (catalog `cooldown_s` left of `cooldown_total_s`). Every
## tile always shows its name and cost, and one state:
## * ready: affordable and off cooldown, drawn lit (accent fill and rim) so
##   the player notices;
## * cooldown: a fill from the bottom, a clock and the seconds left (the
##   cost greys when it is also unaffordable);
## * locked (vehicles before the unlock): a lock and the countdown;
## * limit (vehicles at the limit): the red "n/4" badge and "limit";
## * otherwise dimmed with the reason: "need 25" when short of manpower
##   (steady while the shortfall ticks down: see reason_text), "pop cap".
##
## Click a tile to call it in. Tab focuses the column: 1–9 buy that tile at
## once, Up/Down pick a tile, Enter or Space calls it in, Tab or Esc
## leaves (the camera's keys pause meanwhile). A tile that cannot be
## bought says why in the feed ("Rifle squad not called in: ready again in
## 12 s."); the sim's own refusals arrive the same way (bridge events).
##
## S3: vehicle tiles show cost and pop and an `n/4` badge for the vehicle
## limit. The call-in intent carries the catalog `id`.
##
## S4: one tile per card (catalog `id`; two cards may share a `type`). A
## soldier tile shows its men ("10") in a small disc on the title line;
## its re-call cooldown runs as a ring around the disc (instead of the
## vehicle tiles' fill). An understrength card (`understrength`) gets a
## half-filled chevron before the disc. The tooltip leads with the card's
## `gloss` (period name and English gloss: "Grenadiergruppe — rifle squad,
## 10 men: ..."). When one column of
## tiles would rise above CLEAR_TOP of the HUD height (the S1b 1280 bar: no
## higher than y460 of 800), the tiles go two per row in a column two
## tiles wide.

const TILE_NAMES := {"rifle": "Rifle squad", "mg": "MG team", "engineer": "Engineers", "mortar": "Mortar", "at": "AT team"}
## The column's top stays at or below this fraction of the HUD height.
const CLEAR_TOP := 0.575
const COL_GAP := 4.0
## Least gap between a tile's cost and its reason: 3 px lets "need 222"
## (56 px, tabular) sit next to a 3-digit cost in a 120 px tile.
const REASON_GAP := 3.0
## A ready tile: accent wash and rim.
const READY_FILL := Color(0.878, 0.678, 0.290, 0.16)
const READY_RIM := Color(0.878, 0.678, 0.290, 0.75)

var view: MatchView
var focused := false
var cursor := 0
## Tile columns: 1, or 2 when one column would be too tall.
var columns := 1
var _catalog: Array = []
var _hover := -1
var _vehicles := 0
var _vehicle_limit := 4


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_STOP
	focus_mode = Control.FOCUS_CLICK
	tooltip_text = " "
	focus_exited.connect(func() -> void: _set_focus(false))


## Column height for a metrics set in one column (S1b).
func column_height(m: Dictionary) -> float:
	var n := maxi(view.state.get("callin", {}).get("catalog", []).size(), 1)
	return _strip_h(m, n, 1)


## Column size for a metrics set when its bottom is at `bottom` on a HUD
## `hud_h` tall: one column of tiles, or two (S7: or three, with the
## second vehicle card) when fewer would rise above CLEAR_TOP × hud_h.
## Sets `columns`. The HUD layout calls this.
func column_size(m: Dictionary, bottom: float, hud_h: float) -> Vector2:
	var n := maxi(view.state.get("callin", {}).get("catalog", []).size(), 1)
	columns = 1
	var h := _strip_h(m, n, 1)
	while bottom - h < hud_h * CLEAR_TOP and columns < mini(n, 3):
		columns += 1
		h = _strip_h(m, n, columns)
	return Vector2(float(m.callin_w) * columns + COL_GAP * (columns - 1), h)


func _strip_h(m: Dictionary, n: int, cols := -1) -> float:
	var c := columns if cols < 0 else cols
	return float(m.ci_head) + ceili(n / float(c)) * float(m.tile_h)


func update_state(s: Dictionary) -> void:
	var ci: Dictionary = s.get("callin", {})
	var before := _catalog.size()
	_catalog = ci.get("catalog", [])
	_vehicles = int(ci.get("vehicles", 0))
	_vehicle_limit = int(ci.get("vehicle_limit", s.get("rules", {}).get("vehicles", {}).get("limit", 4)))
	if _catalog.size() != before:
		view.hud.relayout()
	cursor = clampi(cursor, 0, maxi(_catalog.size() - 1, 0))
	queue_redraw()


func tile_rect(i: int) -> Rect2:
	var m: Dictionary = view.hud.m
	var w := (size.x - COL_GAP * (columns - 1)) / columns
	var col := i % columns
	var row := int(i / float(columns))
	return Rect2(col * (w + COL_GAP), float(m.ci_head) + row * float(m.tile_h), w, float(m.tile_h))


## S8b-1: the tile's badge: this card's own alive/max for a card outside
## the vehicle limit, else vehicles/limit.
func _badge(c: Dictionary) -> String:
	if bool(c.get("own_cap", false)):
		return "%d/%d" % [int(c.get("alive", 0)), int(c.get("max_alive", 0))]
	return "%d/%d" % [_vehicles, _vehicle_limit]


func _full(c: Dictionary) -> bool:
	if bool(c.get("own_cap", false)):
		return int(c.get("max_alive", 0)) > 0 and int(c.get("alive", 0)) >= int(c.get("max_alive", 0))
	return _vehicles >= _vehicle_limit


static func is_vehicle(c: Dictionary) -> bool:
	return String(c.get("type", "")) == "vehicle" or c.has("vehicle")


## Why a catalog entry cannot be called in now ("" = it can), in the sim's
## order: "paused", vehicles "locked" before the unlock and "limit" at the
## vehicle limit, "cooldown", "need N" short of manpower, "pop cap".
func blocked_reason(c: Dictionary) -> String:
	if view.input_blocked():
		return "paused"
	if is_vehicle(c):
		if float(c.get("locked_s", 0.0)) > 0.0:
			return "locked"
		if bool(c.get("own_cap", false)):
			# S8b-1: recon cars, trucks and half-tracks count their own cap.
			if int(c.get("max_alive", 0)) > 0 and int(c.get("alive", 0)) >= int(c.get("max_alive", 0)):
				return "card"
		elif _vehicles >= _vehicle_limit:
			return "limit"
	if float(c.get("cooldown_s", 0.0)) > 0.0:
		return "cooldown"
	var short := _short_mp(c)
	if short > 0:
		return "need %d" % short
	var me := view.player(view.local_player)
	if int(me.get("pop", 0)) + int(c.get("pop", c.get("men", 0))) > int(view.rules().get("pop_cap", 120)):
		return "pop cap"
	return ""


## Manpower short of entry `c`'s cost (0 when affordable).
func _short_mp(c: Dictionary) -> int:
	var me := view.player(view.local_player)
	return maxi(int(c.cost) - int(me.get("manpower", 0.0)), 0)


## Whether tile `i` can be bought right now (drawn lit).
func is_ready(i: int) -> bool:
	return i >= 0 and i < _catalog.size() and blocked_reason(_catalog[i]) == ""


## The feed words for why entry `c` cannot be called in (the bridge's
## `refusal_words` for the same reasons).
func refusal_text(c: Dictionary) -> String:
	var why := blocked_reason(c)
	match why:
		"":
			return ""
		"paused":
			return "the match is paused"
		"locked":
			return "vehicles unlock in %s" % UiPalette.mmss(ceilf(float(c.get("locked_s", 0.0))))
		"limit":
			return "vehicle limit reached (%d)" % _vehicle_limit
		"card":
			return "this card's limit reached (%d)" % int(c.get("max_alive", 0))
		"cooldown":
			return "ready again in %d s" % ceili(float(c.get("cooldown_s", 0.0)))
		"pop cap":
			return "pop cap reached (%d)" % int(view.rules().get("pop_cap", 120))
	return "need %d more manpower" % _short_mp(c)


## Calls in catalog entry `i` (tile click, Enter, 1–9); false when
## blocked, and the feed says why.
func call_in_index(i: int) -> bool:
	if i < 0 or i >= _catalog.size():
		return false
	var c: Dictionary = _catalog[i]
	var why := refusal_text(c)
	if why != "":
		# The feed line plays the error sound.
		if view.event_feed != null:
			view.event_feed.push({"kind": "call_in_refused", "text": "%s not called in: %s." % [_short(c.type, c.get("short", "")), why]})
		else:
			AudioManager.play_ui(&"error")
		return false
	var data := {"type": c.type}
	if c.has("id"):
		data.id = c.id
	view.emit_intent(&"call_in", data)
	AudioManager.play_ui(&"confirm")
	return true


## Index of the first catalog entry of a squad type, or -1 (legacy: S4
## cards are keyed by `id`, see index_of_id).
func index_of(type: String) -> int:
	for i in _catalog.size():
		if _catalog[i].type == type:
			return i
	return -1


## S4: index of the catalog entry with card id `id`, or -1.
func index_of_id(id: String) -> int:
	for i in _catalog.size():
		if String(_catalog[i].get("id", "")) == id:
			return i
	return -1


## S4: the men disc of a soldier tile (zero rect for vehicle tiles): on
## the title line's right end.
func men_disc_rect(i: int) -> Rect2:
	if i < 0 or i >= _catalog.size() or is_vehicle(_catalog[i]):
		return Rect2()
	var r := tile_rect(i)
	var d := 18.0
	return Rect2(r.end.x - 8.0 - d, r.position.y + 3.0, d, d)


## S4: the share of tile `i`'s ring that is lit (its cooldown left of its
## full length; 0 when ready).
func ring_frac(i: int) -> float:
	if i < 0 or i >= _catalog.size():
		return 0.0
	var c: Dictionary = _catalog[i]
	var cd := float(c.get("cooldown_s", 0.0))
	if cd <= 0.0:
		return 0.0
	return clampf(cd / maxf(float(c.get("cooldown_total_s", cd)), 0.01), 0.0, 1.0)


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
	var hint := "1–%d" % mini(_catalog.size(), 9) if focused else "Tab"
	HudStyle.text(self, font, Vector2(w - 8, float(m.ci_head) - 4.0), hint, HudStyle.FONT_SMALL, HudStyle.ACCENT if focused else HudStyle.DIM, 1)
	for i in _catalog.size():
		_draw_tile(i, font, bold)


func _draw_tile(i: int, font: Font, bold: Font) -> void:
	var c: Dictionary = _catalog[i]
	var r := tile_rect(i)
	var why := blocked_reason(c)
	var off := why != ""
	var ready := not off
	if ready:
		draw_rect(r.grow_individual(-1, -1, -1, -1), READY_FILL)
		draw_rect(r.grow_individual(-1.5, -1.5, -1.5, -1.5), READY_RIM, false, 1.0)
	else:
		draw_rect(Rect2(r.position.x + 8, r.position.y, r.size.x - 16, 1), Color(HudStyle.TEXT, 0.14))
	var cd := float(c.get("cooldown_s", 0.0))
	var vehicle := is_vehicle(c)
	if cd > 0.0 and vehicle:
		var frac := clampf(cd / maxf(float(c.get("cooldown_total_s", cd)), 0.01), 0.0, 1.0)
		draw_rect(Rect2(r.position.x + 1, r.end.y - r.size.y * frac, r.size.x - 2, r.size.y * frac - 1), Color(HudStyle.TEXT, 0.12))
	if (_hover == i and not off) or (focused and cursor == i):
		draw_rect(r.grow_individual(-1, -1, -1, -1), HudStyle.HOVER)
	if focused and cursor == i:
		draw_rect(Rect2(r.position + Vector2(0, 2), Vector2(3, r.size.y - 4)), HudStyle.ACCENT)
	var x0 := r.position.x + 8.0
	var x1 := r.end.x - 8.0
	# Focused: the tile's buy key (1–9) before its name.
	if focused and i < 9:
		var key := "%d" % (i + 1)
		HudStyle.text(self, bold, Vector2(x0, r.position.y + 16.0), key, HudStyle.FONT_SMALL, HudStyle.ACCENT)
		x0 += HudStyle.text_w(bold, key, HudStyle.FONT_SMALL) + 5.0
	# Vehicle limit badge ("2/4") on the title line.
	var badge_w := 0.0
	if vehicle:
		var badge := _badge(c)
		badge_w = HudStyle.text_w(bold, badge, HudStyle.FONT_SMALL) + 8.0
		var br := Rect2(x1 - badge_w, r.position.y + 4.0, badge_w, 16.0)
		var full := _full(c)
		HudStyle.box(self, br, Color(HudStyle.DANGER, 0.35) if full else HudStyle.BG_CHIP, 3)
		HudStyle.text(self, bold, Vector2(br.get_center().x, br.position.y + 12.5), badge, HudStyle.FONT_SMALL, HudStyle.TEXT, 0)
		badge_w += 4.0
	else:
		badge_w = _draw_men_disc(i, c, bold, off)
	var title := HudStyle.fit(bold, _short(c.type, c.get("short", "")), x1 - x0 - badge_w, HudStyle.FONT)
	HudStyle.text(self, bold, Vector2(x0, r.position.y + 16.0), title, HudStyle.FONT, HudStyle.OFF_TEXT if off else HudStyle.TEXT)
	x0 = r.position.x + 8.0
	var cost := "%d MP" % int(c.cost)
	var cost_w := HudStyle.text_w(bold, cost, HudStyle.FONT)
	var afford := _short_mp(c) == 0
	HudStyle.text(self, bold, Vector2(x0, r.end.y - 5.0), cost, HudStyle.FONT, HudStyle.ACCENT if afford and why in ["", "cooldown"] else HudStyle.OFF_ACCENT)
	var locked := float(c.get("locked_s", 0.0))
	if vehicle and locked > 0.0:
		# Lock and the countdown to the unlock.
		var t := UiPalette.mmss(ceilf(locked))
		HudStyle.text(self, bold, Vector2(x1, r.end.y - 5.0), t, HudStyle.FONT_SMALL, HudStyle.TEXT, 1)
		HudStyle.lock(self, Vector2(x1 - HudStyle.text_w(bold, t, HudStyle.FONT_SMALL) - 9.0, r.end.y - 11.0), HudStyle.TEXT, true)
	elif cd > 0.0 and why == "cooldown":
		var secs := "%ds" % ceili(cd)
		HudStyle.text(self, bold, Vector2(x1, r.end.y - 5.0), secs, HudStyle.FONT_SMALL, HudStyle.TEXT, 1)
		HudStyle.clock(self, Vector2(x1 - 6.0 - HudStyle.text_w(bold, secs, HudStyle.FONT_SMALL) - 6.0, r.end.y - 10.0), 5.5, HudStyle.TEXT)
	elif off:
		var shown := reason_text(i)
		if shown != "":
			HudStyle.text(self, HudStyle.tabular(font), Vector2(x1, r.end.y - 5.0), shown, HudStyle.FONT, Color(HudStyle.TEXT, 0.75), 1)
	elif vehicle:
		var pop := "%d pop" % int(c.get("pop", 0))
		if HudStyle.text_w(font, pop, HudStyle.FONT_SMALL) <= x1 - x0 - 6.0 - cost_w:
			HudStyle.text(self, font, Vector2(x1, r.end.y - 5.0), pop, HudStyle.FONT_SMALL, HudStyle.DIM, 1)


## S4: the men disc (count, cooldown ring) and the understrength chevron
## of soldier tile `i`; returns the width they take on the title line.
func _draw_men_disc(i: int, c: Dictionary, bold: Font, off: bool) -> float:
	var dr := men_disc_rect(i)
	if dr.size == Vector2.ZERO:
		return 0.0
	var ctr := dr.get_center()
	var rad := dr.size.x * 0.5
	draw_circle(ctr, rad, HudStyle.BG_CHIP)
	draw_arc(ctr, rad - 1.0, 0.0, TAU, 24, Color(HudStyle.TEXT, 0.18), 1.5, true)
	var frac := ring_frac(i)
	if frac > 0.0:
		# The cooldown left, clockwise from the top.
		draw_arc(ctr, rad - 1.0, -PI * 0.5, -PI * 0.5 + TAU * frac, 24, HudStyle.ACCENT, 2.5, true)
	var men := "%d" % int(c.get("men", 0))
	HudStyle.text(self, bold, Vector2(ctr.x, ctr.y + 4.5), men, HudStyle.FONT_SMALL, HudStyle.OFF_TEXT if off else HudStyle.TEXT, 0)
	var w := dr.size.x + 4.0
	if bool(c.get("understrength", false)):
		# Half-filled chevron (a squad below establishment).
		var cx := dr.position.x - 9.0
		var cy := ctr.y
		var pts := PackedVector2Array([Vector2(cx - 5, cy - 4), Vector2(cx, cy + 3), Vector2(cx + 5, cy - 4)])
		draw_polyline(pts, HudStyle.ACCENT, 1.5, true)
		draw_colored_polygon(PackedVector2Array([Vector2(cx - 5, cy - 4), Vector2(cx, cy + 3), Vector2(cx, cy - 1)]), HudStyle.ACCENT)
		w += 14.0
	return w


## The reason tile `i` writes right of its cost ("need 192"), or "" when it
## writes none: it can be called in, it shows a lock or cooldown instead,
## or the reason does not fit. Numbers use tabular figures and the fit is
## judged with every digit as "0", so whether "need N" shows depends only
## on how many digits N has, never on which: a shortfall that ticks down
## with income does not blink the label (proportional "need 222" is 56 px,
## "need 221" 53 px, against 55 px of room next to "250 MP").
func reason_text(i: int) -> String:
	if i < 0 or i >= _catalog.size():
		return ""
	var c: Dictionary = _catalog[i]
	var why := blocked_reason(c)
	if why == "" or why == "cooldown" or why == "locked":
		return ""
	var room := tile_rect(i).size.x - 16.0 - REASON_GAP - HudStyle.text_w(HudStyle.bold(), "%d MP" % int(c.cost), HudStyle.FONT)
	var nums := HudStyle.tabular(get_theme_default_font())
	return why if HudStyle.text_w(nums, _zero_digits(why), HudStyle.FONT) <= room else ""


## `s` with every digit replaced by "0" (a width template for numbers).
static func _zero_digits(s: String) -> String:
	var out := ""
	for ch in s:
		out += "0" if ch >= "0" and ch <= "9" else ch
	return out


## Tile name (catalog `short`, else TILE_NAMES).
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
		var k: int = event.physical_keycode
		if k >= KEY_1 and k <= KEY_9:
			# The tile's number buys it at once.
			var i := k - KEY_1
			if i < _catalog.size():
				cursor = i
				call_in_index(i)
			queue_redraw()
			accept_event()
			return
		match k:
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
		return "Call-in  [Tab]: 1–9 buy a tile, Up/Down pick, Enter calls in."
	var c: Dictionary = _catalog[i]
	var what := "%d men" % int(c.get("men", 0))
	if is_vehicle(c):
		what = "%d pop, %s %s" % [int(c.get("pop", 0)), "own limit" if bool(c.get("own_cap", false)) else "vehicle", _badge(c)]
	var recall := float(c.get("recall_s", c.get("cooldown_total_s", 0.0)))
	var after := "" if recall <= 0.0 else " Then %d s before the next." % int(recall)
	var why := refusal_text(c)
	# S4: the card's gloss ("Grenadiergruppe — rifle squad, 10 men: ...").
	var head := String(c.get("gloss", "%s: %s" % [c.name, what]))
	# S4 review: the chevron's meaning, in words.
	if bool(c.get("understrength", false)):
		head += "\nUnderstrength (chevron): fewer men than the full card, cheaper."
	return "%s\n%d manpower. Deploys at the base at once.%s%s" % [head, int(c.cost), after, "" if why == "" else "\nNot now: " + why]
