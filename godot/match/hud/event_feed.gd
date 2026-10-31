extends VBoxContainer
## Event messages ("Point B lost"), newest at the top, fading after a few
## seconds. A message with a position is clickable (camera jump) and Space
## jumps to the latest one.

const LIFETIME := 7.0
const FADE := 1.0
const MAX_LINES := 6
## kind -> [stripe colour, ui sound or &""]
const KINDS := {
	"point_captured": [UiPalette.OWN, &"confirm"],
	"point_lost": [UiPalette.ENEMY, &"error"],
	"squad_lost": [UiPalette.DANGER, &""],
	"squad_arrived": [UiPalette.OK, &""],
	"under_attack": [UiPalette.WARN, &""],
	"tickets_low": [UiPalette.DANGER, &"error"],
	"info": [UiPalette.TEXT_DIM, &""],
}

var view: MatchView
var _last_pos: Variant = null


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_theme_constant_override("separation", 4)
	alignment = BoxContainer.ALIGNMENT_BEGIN


func push(e: Dictionary) -> void:
	var kind: String = e.get("kind", "info")
	var def: Array = KINDS.get(kind, KINDS.info)
	var panel := PanelContainer.new()
	panel.theme_type_variation = &"HudPanel"
	panel.size_flags_horizontal = Control.SIZE_SHRINK_END
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 8)
	panel.add_child(row)
	var stripe := ColorRect.new()
	stripe.color = def[0]
	stripe.custom_minimum_size = Vector2(4, 0)
	row.add_child(stripe)
	var l := Label.new()
	l.text = "%s   %s" % [UiPalette.mmss(e.get("time_s", view.state.get("time_s", 0.0))), e.get("text", kind)]
	l.add_theme_font_size_override("font_size", 15)
	row.add_child(l)
	panel.set_meta("born", Time.get_ticks_msec())
	if e.has("pos"):
		_last_pos = e.pos
		panel.mouse_filter = Control.MOUSE_FILTER_STOP
		panel.mouse_default_cursor_shape = Control.CURSOR_POINTING_HAND
		panel.tooltip_text = "Click (or Space) to look"
		var pos: Vector3 = e.pos
		panel.gui_input.connect(func(ev: InputEvent) -> void:
			if ev is InputEventMouseButton and ev.pressed and ev.button_index == MOUSE_BUTTON_LEFT:
				view.jump_camera(pos))
	else:
		panel.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(panel)
	move_child(panel, 0)
	while get_child_count() > MAX_LINES:
		var old := get_child(get_child_count() - 1)
		remove_child(old)
		old.queue_free()
	if def[1] != &"":
		AudioManager.play_ui(def[1])


func last_position() -> Variant:
	return _last_pos


func _process(_delta: float) -> void:
	var now := Time.get_ticks_msec()
	for c in get_children():
		var age := (now - int(c.get_meta("born", now))) / 1000.0
		c.modulate.a = clampf((LIFETIME - age) / FADE, 0.0, 1.0)
		if age > LIFETIME:
			c.queue_free()
