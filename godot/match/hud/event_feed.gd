extends Control
## Event feed (HUD v2), top right: at most MAX_LINES messages, newest on
## top, each fading out after LIFETIME s. Click-through (mouse_filter
## IGNORE): Space jumps to the latest message with a position.

const LIFETIME := 8.0
const FADE := 1.0
const MAX_LINES := 3
## Older lines are a little dimmer (newest first).
const LINE_ALPHA := [1.0, 0.9, 0.8]
## kind -> [stripe colour, ui sound or &""]
const KINDS := {
	"point_captured": [HudStyle.OWN, &"confirm"],
	"point_lost": [HudStyle.ENEMY, &"error"],
	"squad_lost": [HudStyle.DANGER, &""],
	"squad_arrived": [HudStyle.TEXT, &""],
	"under_attack": [HudStyle.ACCENT, &""],
	"tickets_low": [HudStyle.DANGER, &"error"],
	"info": [HudStyle.DIM, &""],
	# S3: main-gun and rocket matters only (the bridge derives these).
	"vehicle_arrived": [HudStyle.TEXT, &""],
	"vehicle_knocked_out": [HudStyle.DANGER, &""],
	"vehicle_damaged": [HudStyle.ACCENT, &""],
	"vehicle_bogged": [HudStyle.ACCENT, &""],
	# S3b: a call-in the sim or the panel refused, with the reason.
	"call_in_refused": [HudStyle.DANGER, &"error"],
}

var view: MatchView
## Newest first: {text, time, kind, born_ms}.
var lines: Array = []
var _last_pos: Variant = null


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_IGNORE


func push(e: Dictionary) -> void:
	var kind: String = e.get("kind", "info")
	var def: Array = KINDS.get(kind, KINDS.info)
	if e.has("pos"):
		_last_pos = e.pos
	lines.push_front({
		"text": String(e.get("text", kind)),
		"time": UiPalette.mmss(e.get("time_s", view.state.get("time_s", 0.0))),
		"kind": kind,
		# An enemy vehicle knocked out is good news: own colour stripe.
		"good": kind == "vehicle_knocked_out" and int(e.get("player", view.local_player)) != view.local_player,
		"born_ms": Time.get_ticks_msec(),
	})
	if lines.size() > MAX_LINES:
		lines.resize(MAX_LINES)
	if def[1] != &"":
		AudioManager.play_ui(def[1])
	queue_redraw()


func last_position() -> Variant:
	return _last_pos


## Opacity of a line by age and position.
func line_alpha(i: int) -> float:
	var age := (Time.get_ticks_msec() - int(lines[i].born_ms)) / 1000.0
	return clampf((LIFETIME + FADE - age) / FADE, 0.0, 1.0) * float(LINE_ALPHA[mini(i, LINE_ALPHA.size() - 1)])


func update_state(_s: Dictionary) -> void:
	var now := Time.get_ticks_msec()
	lines = lines.filter(func(l: Dictionary) -> bool: return (now - int(l.born_ms)) / 1000.0 < LIFETIME + FADE)
	queue_redraw()


func _draw() -> void:
	if view == null:
		return
	var font := get_theme_default_font()
	var lh: float = view.hud.m.feed_line
	for i in lines.size():
		var l: Dictionary = lines[i]
		var a := line_alpha(i)
		if a <= 0.0:
			continue
		var y := i * lh
		var avail := size.x - 30.0 - HudStyle.text_w(font, l.time, HudStyle.FONT_SMALL)
		var text := HudStyle.fit(font, l.text, avail, HudStyle.FONT)
		var tw := HudStyle.text_w(font, text, HudStyle.FONT)
		var tm := HudStyle.text_w(font, l.time, HudStyle.FONT_SMALL)
		var w := tw + tm + 30.0
		var r := Rect2(size.x - w, y, w, lh - 3.0)
		HudStyle.box(self, r, Color(HudStyle.BG_CALLIN, HudStyle.BG_CALLIN.a * a), 3)
		var stripe: Color = HudStyle.OWN if l.get("good", false) else KINDS.get(l.kind, KINDS.info)[0]
		draw_rect(Rect2(r.position + Vector2(0, 3), Vector2(2, r.size.y - 6)), Color(stripe, a))
		var by := y + 16.0
		HudStyle.text(self, font, Vector2(size.x - 10.0, by), text, HudStyle.FONT, Color(HudStyle.TEXT, a), 1)
		HudStyle.text(self, font, Vector2(size.x - 20.0 - tw, by), l.time, HudStyle.FONT_SMALL, Color(HudStyle.DIM, a), 1)
