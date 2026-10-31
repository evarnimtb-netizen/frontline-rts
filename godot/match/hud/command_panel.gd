extends PanelContainer
## Command panel: the S1 orders with their hotkeys. A button is disabled
## when the selection cannot take the order (dig needs a rifle or engineer
## squad, barrage needs a mortar); the armed tool stays highlighted.

const ORDERS := [
	[&"move", "Move", "Move to a point. Right-click on the ground does the same."],
	[&"attack_move", "Attack-move", "Advance and engage spotted enemies on the way."],
	[&"stop", "Stop", "Halt and hold the current position."],
	[&"retreat", "Retreat", "Fall back to base, faster, ignoring enemies."],
	[&"dig", "Dig trench", "Click points along the ground (max 60 m), right-click to dig. Rifle and engineer squads dig; engineers twice as fast."],
	[&"barrage", "Barrage", "Mortars fire on a position (60–600 m). Right-clicking the ground in range with mortars selected also fires."],
]

var view: MatchView
var _buttons := {}
var _hint: Label


func bind(v: MatchView) -> void:
	view = v
	theme_type_variation = &"HudPanel"
	mouse_filter = Control.MOUSE_FILTER_STOP
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 4)
	add_child(box)
	var head := Label.new()
	head.text = "ORDERS"
	head.theme_type_variation = &"HeaderLabel"
	head.add_theme_font_size_override("font_size", 18)
	box.add_child(head)
	var grid := GridContainer.new()
	grid.columns = 3
	grid.add_theme_constant_override("h_separation", 4)
	grid.add_theme_constant_override("v_separation", 4)
	box.add_child(grid)
	for o in ORDERS:
		var order: StringName = o[0]
		var b := Button.new()
		b.theme_type_variation = &"HudButton"
		b.focus_mode = Control.FOCUS_NONE
		b.toggle_mode = order in [&"move", &"attack_move", &"dig", &"barrage"]
		b.custom_minimum_size = Vector2(104, 46)
		b.text = o[1]
		b.tooltip_text = "%s  [%s]\n%s" % [o[1], MatchInput.LABELS[StringName("order_" + order)], o[2]]
		b.set_meta("no_click_sound", true)
		b.pressed.connect(_on_pressed.bind(order))
		var key := Label.new()
		key.text = MatchInput.LABELS[StringName("order_" + order)]
		key.add_theme_font_size_override("font_size", 11)
		key.add_theme_color_override("font_color", UiPalette.ACCENT)
		key.position = Vector2(5, 2)
		key.mouse_filter = Control.MOUSE_FILTER_IGNORE
		b.add_child(key)
		grid.add_child(b)
		_buttons[order] = b
	_hint = Label.new()
	_hint.theme_type_variation = &"DimLabel"
	_hint.add_theme_font_size_override("font_size", 12)
	_hint.text = "Ctrl+1–9 group · 1–9 select · Space last event"
	box.add_child(_hint)


func _on_pressed(order: StringName) -> void:
	var sel: Node = view.selection
	if sel.tool == order:
		sel.cancel_tool()
	else:
		sel.activate(order)
	AudioManager.play_ui(&"click")


func update_state(_s: Dictionary) -> void:
	var sel: Node = view.selection
	for order in _buttons:
		var b: Button = _buttons[order]
		b.disabled = view.input_blocked() or not sel.can(order)
		b.set_pressed_no_signal(sel.tool == order)
