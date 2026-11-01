class_name NetReadout
extends Control
## Net readout in the top bar (S2 Presenter): a status dot, then ping, the
## input delay D in ticks and ms, restamps and stalls. hud_layout places it
## in the free span between the enemy tickets and the clock; it picks the
## widest of three wordings that fits there (1600: words, 1280: letters).
## Hidden outside net mode. The tooltip spells everything out.
##
## Dot: green ok, amber unstable / waiting / paused (blinks while
## unstable), red desync (blinks).

## Wordings, widest first: [ping caption, delay caption, restamp caption,
## stall caption, delay value format ("%d" ticks, "%d" ms)].
const LEVELS := [
	["PING", "DELAY", "RESTAMPS", "STALLS", "%d t · %d ms"],
	["PING", "D", "RST", "STL", "%d · %d ms"],
	["", "D", "R", "S", "%d·%dms"],
]
const DOT_R := 4.5
const GAP := 10.0

var view: MatchView
## Wording in use (index into LEVELS) after the last draw; -1 hidden.
var level := -1
var _blink := 0.0


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_PASS
	tooltip_text = " "  # enables _get_tooltip
	visible = false


func update_state(_s: Dictionary) -> void:
	visible = view.net_mode
	_blink += get_process_delta_time()
	if visible:
		queue_redraw()


## [caption, value] pairs for a wording.
func parts(lv: int) -> Array:
	var n := view.net
	var w: Array = LEVELS[lv]
	var d := int(n.get("input_delay", 0))
	var dms := int(n.get("delay_ms", NetUi.delay_ms(d)))
	return [
		[w[0], NetUi.ping_text(float(n.get("ping_ms", -1.0)))],
		[w[1], w[4] % [d, dms]],
		[w[2], "%d" % int(n.get("restamps", 0))],
		[w[3], "%d" % int(n.get("stalls", 0))],
	]


## Width a wording needs in HUD pixels.
func width_of(lv: int) -> float:
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var w := DOT_R * 2.0 + 8.0
	var ps := parts(lv)
	for i in ps.size():
		var cap: String = ps[i][0]
		if cap != "":
			w += HudStyle.text_w(font, cap, HudStyle.FONT_SMALL) + 5.0
		w += HudStyle.text_w(bold, ps[i][1], HudStyle.FONT)
		if i < ps.size() - 1:
			w += GAP
	return w


## The widest wording that fits the current size (the last one if none).
func pick_level() -> int:
	for lv in LEVELS.size():
		if width_of(lv) <= size.x:
			return lv
	return LEVELS.size() - 1


func _draw() -> void:
	if view == null or not view.net_mode:
		level = -1
		return
	level = pick_level()
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var by := size.y * 0.5 + 5.0
	# Right-aligned in its span, so it sits next to the clock.
	var x := size.x - width_of(level)
	var st := String(view.net.get("state", "ok"))
	var col := NetUi.state_color(st)
	var blink := st in ["unstable", "desync"] and fmod(_blink, 1.0) > 0.6
	var c := Vector2(x + DOT_R, size.y * 0.5)
	draw_circle(c, DOT_R + 1.5, Color(0, 0, 0, 0.45))
	draw_circle(c, DOT_R, Color(col, 0.35) if blink else col)
	x += DOT_R * 2.0 + 8.0
	var ps := parts(level)
	for i in ps.size():
		var cap: String = ps[i][0]
		if cap != "":
			HudStyle.text(self, font, Vector2(x, by), cap, HudStyle.FONT_SMALL, HudStyle.DIM)
			x += HudStyle.text_w(font, cap, HudStyle.FONT_SMALL) + 5.0
		var val: String = ps[i][1]
		var vcol := HudStyle.TEXT
		# Ping turns amber over 150 ms; restamps and stalls once non-zero
		# stay plain (they count up slowly on any real link).
		if i == 0 and float(view.net.get("ping_ms", 0.0)) > 150.0:
			vcol = UiPalette.WARN
		HudStyle.text(self, bold, Vector2(x, by), val, HudStyle.FONT, vcol)
		x += HudStyle.text_w(bold, val, HudStyle.FONT) + GAP


func _get_tooltip(_at: Vector2) -> String:
	if view == null or not view.net_mode:
		return ""
	var n := view.net
	var d := int(n.get("input_delay", 0))
	var lines := PackedStringArray([
		NetUi.STATE_TEXT.get(String(n.get("state", "ok")), String(n.get("state", ""))),
		"Ping %s: round trip to the relay." % NetUi.ping_text(float(n.get("ping_ms", -1.0))),
		"Input delay D = %d ticks = %d ms: your orders take effect this long after you give them (fixed for the match)." % [d, int(n.get("delay_ms", NetUi.delay_ms(d)))],
		"Restamps %d: orders that reached the relay too late for their turn and went into the next one." % int(n.get("restamps", 0)),
		"Stalls %d: ticks this game waited for a late turn from the relay." % int(n.get("stalls", 0)),
	])
	return "\n".join(lines)
