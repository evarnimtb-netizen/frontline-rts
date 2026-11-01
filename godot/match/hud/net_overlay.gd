class_name NetOverlay
extends Control
## Net-mode panels over the battlefield (S2 Presenter), in HUD pixels
## (hud_layout gives it the whole HUD rect):
##
##   desync banner  red, under the top bar between the minimap and the
##                  feed: the tick, the dump folder (Copy path), Leave
##   waiting        "Waiting for <player> (Ns)" with a draining bar; for
##                  the local player's own drop it reads "Reconnecting"
##   paused         "Paused by <player>", with Resume (either player may)
##
## Only one shows at a time (desync over waiting over paused). The panels
## are drawn like the rest of HUD v2; the buttons are HudButtons placed
## on them. The root ignores the mouse; only the buttons take clicks.

const BANNER_W := 720.0
const BANNER_H := 66.0
const PANEL_W := 440.0
## Waiting timeout the bar drains from (S2: Surrender after 60 s).
const WAIT_TOTAL_S := 60.0

var view: MatchView
## What is showing: "", "desync", "waiting", "paused" (tests read it).
var mode := ""
var banner_rect := Rect2()
var panel_rect := Rect2()
var copy_button: Button
var leave_button: Button
var resume_button: Button
## Last text drawn per line (tests read it).
var lines: PackedStringArray = []


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	copy_button = _button("Copy path", _copy_path)
	leave_button = _button("Leave match", func() -> void: view.pause_menu.ask_leave())
	resume_button = _button("Resume", func() -> void: view.emit_intent(&"pause", {"paused": false}))


func _button(text: String, cb: Callable) -> Button:
	var b := Button.new()
	b.text = text
	b.theme_type_variation = &"HudButton"
	b.focus_mode = Control.FOCUS_NONE
	b.visible = false
	b.pressed.connect(cb)
	add_child(b)
	return b


func update_state(_s: Dictionary) -> void:
	var st := String(view.net.get("state", "ok")) if view.net_mode else ""
	mode = st if st in ["desync", "waiting", "paused"] else ""
	_layout()
	queue_redraw()


func _copy_path() -> void:
	var p := String(view.net.get("desync_dump", ""))
	if p != "":
		DisplayServer.clipboard_set(p)
		copy_button.text = "Copied"


## Rects for the current mode and HUD size; places the buttons.
func _layout() -> void:
	var m: Dictionary = view.hud.m
	var w := size.x
	var tb: float = m.tb
	var g: float = m.gutter
	# Free band under the top bar: right of the minimap (when it is up
	# there), left of the feed.
	var left := g
	if view.minimap.dock != "bottom_left":
		left += float(m.mm) + 16.0
	var right := w - g - float(m.feed_w) - 16.0
	var bw := minf(BANNER_W, right - left)
	banner_rect = Rect2(roundf((left + right - bw) * 0.5), tb + 8.0, bw, BANNER_H)
	var ph := 118.0 if mode == "waiting" else 104.0
	panel_rect = Rect2(roundf((w - PANEL_W) * 0.5), roundf(size.y * 0.30), PANEL_W, ph)
	copy_button.visible = mode == "desync"
	leave_button.visible = mode == "desync"
	resume_button.visible = mode == "paused"
	if mode == "desync":
		var bh := 26.0
		leave_button.size = Vector2(104, bh)
		leave_button.position = Vector2(banner_rect.end.x - 12.0 - 104.0, banner_rect.position.y + 8.0)
		copy_button.size = Vector2(104, bh)
		copy_button.position = Vector2(banner_rect.end.x - 12.0 - 104.0, banner_rect.end.y - 8.0 - bh)
		if String(view.net.get("desync_dump", "")) == "":
			copy_button.visible = false
	elif mode == "paused":
		resume_button.size = Vector2(110, 30)
		resume_button.position = panel_rect.end - Vector2(14.0 + 110.0, 14.0 + 30.0)


func _draw() -> void:
	lines = PackedStringArray()
	if view == null or mode == "":
		return
	match mode:
		"desync":
			_draw_desync()
		"waiting":
			_draw_waiting()
		"paused":
			_draw_paused()


func _draw_desync() -> void:
	var r := banner_rect
	var bold := HudStyle.bold()
	var font := get_theme_default_font()
	HudStyle.box(self, r, Color(UiPalette.DANGER.darkened(0.25), 0.92), 4, Color(1, 0.8, 0.75, 0.5), 1)
	var tick := int(view.net.get("desync_tick", -1))
	var head := "DESYNC at tick %d" % tick if tick >= 0 else "DESYNC"
	var x := r.position.x + 14.0
	var text_w := r.size.x - 14.0 - 12.0 - 104.0 - 14.0
	HudStyle.text(self, bold, Vector2(x, r.position.y + 24.0), head, 17, Color.WHITE)
	var hw := HudStyle.text_w(bold, head, 17) + 12.0
	var why := HudStyle.fit(font, "The two games disagree. The match is stopped.", text_w - hw, HudStyle.FONT)
	HudStyle.text(self, font, Vector2(x + hw, r.position.y + 23.0), why, HudStyle.FONT, Color(1, 1, 1, 0.9))
	var dump := String(view.net.get("desync_dump", ""))
	var line2 := "Dump: " + _fit_tail(font, dump, text_w - HudStyle.text_w(font, "Dump: ", HudStyle.FONT), HudStyle.FONT) if dump != "" else "No dump was written."
	HudStyle.text(self, font, Vector2(x, r.end.y - 14.0), line2, HudStyle.FONT, Color(1, 1, 1, 0.85))
	lines = PackedStringArray([head, why, line2])


func _draw_waiting() -> void:
	var r := panel_rect
	var bold := HudStyle.bold()
	var font := get_theme_default_font()
	HudStyle.box(self, r, HudStyle.BG_TAB_ON, 4, Color(HudStyle.ACCENT, 0.6), 1)
	var who := String(view.net.get("waiting_for", ""))
	var left := maxf(float(view.net.get("waiting_s_left", 0.0)), 0.0)
	var self_drop := _is_local(who, int(view.net.get("waiting_slot", -1)))
	var head: String
	var body: String
	if self_drop:
		head = "RECONNECTING (%d s)" % ceili(left)
		body = "Your connection to the relay dropped. The match is paused while this game catches up."
	else:
		var n := who if who != "" else "your opponent"
		head = "WAITING FOR %s (%d s)" % [n.to_upper(), ceili(left)]
		body = "%s's connection dropped. If they are not back in time, they surrender." % n
	var x := r.position.x + 16.0
	var tw := r.size.x - 32.0
	HudStyle.text(self, bold, Vector2(x, r.position.y + 30.0), HudStyle.fit(bold, head, tw, 18), 18, HudStyle.ACCENT)
	var wrapped := _wrap(font, body, tw, HudStyle.FONT)
	for i in wrapped.size():
		HudStyle.text(self, font, Vector2(x, r.position.y + 54.0 + i * 19.0), wrapped[i], HudStyle.FONT, HudStyle.TEXT)
	var bar := Rect2(x, r.end.y - 18.0, tw, 6.0)
	HudStyle.box(self, bar, Color(0, 0, 0, 0.45), 2)
	HudStyle.box(self, Rect2(bar.position, Vector2(tw * clampf(left / WAIT_TOTAL_S, 0.0, 1.0), 6.0)), HudStyle.ACCENT if left > 10.0 else UiPalette.DANGER, 2)
	lines = PackedStringArray([head])
	lines.append_array(wrapped)


func _draw_paused() -> void:
	var r := panel_rect
	var bold := HudStyle.bold()
	var font := get_theme_default_font()
	HudStyle.box(self, r, HudStyle.BG_TAB_ON, 4, Color(HudStyle.ACCENT, 0.6), 1)
	var who := String(view.net.get("paused_by", ""))
	var by_me := _is_local(who, int(view.net.get("paused_slot", -1)))
	var head := "PAUSED BY YOU" if by_me else ("PAUSED BY %s" % who.to_upper() if who != "" else "PAUSED")
	var body := "Either player can resume. Orders wait until then."
	var x := r.position.x + 16.0
	var tw := r.size.x - 32.0
	HudStyle.text(self, bold, Vector2(x, r.position.y + 30.0), HudStyle.fit(bold, head, tw, 18), 18, HudStyle.ACCENT)
	var wrapped := _wrap(font, body, tw - 130.0, HudStyle.FONT)
	for i in wrapped.size():
		HudStyle.text(self, font, Vector2(x, r.position.y + 56.0 + i * 19.0), wrapped[i], HudStyle.FONT, HudStyle.TEXT)
	lines = PackedStringArray([head])
	lines.append_array(wrapped)


## True when `who` (a name) or `slot` names the local player.
func _is_local(who: String, slot: int) -> bool:
	if slot >= 0:
		return slot == view.local_player
	var mine := String(view.player(view.local_player).get("name", ""))
	return who != "" and who == mine


## `s` shortened from the front with an ellipsis (keeps a path's end).
static func _fit_tail(font: Font, s: String, width: float, fs: int) -> String:
	if HudStyle.text_w(font, s, fs) <= width:
		return s
	var t := s
	while t.length() > 1 and HudStyle.text_w(font, "…" + t, fs) > width:
		t = t.substr(1)
	return "…" + t


## Greedy word wrap to `width` px.
static func _wrap(font: Font, s: String, width: float, fs: int) -> PackedStringArray:
	var out := PackedStringArray()
	var line := ""
	for word in s.split(" "):
		var t := word if line == "" else line + " " + word
		if line != "" and HudStyle.text_w(font, t, fs) > width:
			out.append(line)
			line = word
		else:
			line = t
	if line != "":
		out.append(line)
	return out
