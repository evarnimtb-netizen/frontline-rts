extends PanelContainer
## Call-in panel: one button per squad type from state.callin.catalog (cost,
## men, arrival delay), disabled when manpower, population or the queue
## does not allow it, and the queue with time left for each squad.

var view: MatchView
var _list: VBoxContainer
var _queue: VBoxContainer
var _buttons := {}
var _catalog_sig := ""


func bind(v: MatchView) -> void:
	view = v
	theme_type_variation = &"HudPanel"
	mouse_filter = Control.MOUSE_FILTER_STOP
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 4)
	add_child(box)
	var head := Label.new()
	head.text = "CALL-IN"
	head.theme_type_variation = &"HeaderLabel"
	head.add_theme_font_size_override("font_size", 18)
	box.add_child(head)
	_list = VBoxContainer.new()
	_list.add_theme_constant_override("separation", 3)
	box.add_child(_list)
	_queue = VBoxContainer.new()
	_queue.add_theme_constant_override("separation", 2)
	box.add_child(_queue)


func update_state(s: Dictionary) -> void:
	var ci: Dictionary = s.get("callin", {})
	var catalog: Array = ci.get("catalog", [])
	var sig := str(catalog.map(func(c: Dictionary) -> String: return c.type))
	if sig != _catalog_sig:
		_catalog_sig = sig
		_rebuild(catalog)
	var me := view.player(view.local_player)
	var rules := view.rules()
	var queue: Array = ci.get("queue", [])
	var queue_full := queue.size() >= int(ci.get("queue_max", 5))
	var pop_cap := int(rules.get("pop_cap", 120))
	var queued_men := 0
	for q in queue:
		queued_men += int(q.get("men", 0))
	for c in catalog:
		var b: Button = _buttons.get(c.type)
		if b == null:
			continue
		var afford := float(me.get("manpower", 0.0)) >= float(c.cost)
		var room := int(me.get("pop", 0)) + queued_men + int(c.men) <= pop_cap
		b.disabled = view.input_blocked() or not afford or not room or queue_full
		var why := "" if afford else "\nNot enough manpower."
		if not room:
			why += "\nPopulation cap reached."
		if queue_full:
			why += "\nQueue full."
		b.tooltip_text = "%s: %d men, %d manpower, arrives at base after %d s.%s" % [c.name, c.men, c.cost, c.delay_s, why]
	_update_queue(queue)


func _rebuild(catalog: Array) -> void:
	for c in _list.get_children():
		c.queue_free()
	_buttons.clear()
	for c in catalog:
		var b := Button.new()
		b.theme_type_variation = &"HudButton"
		b.set_meta("no_click_sound", true)  # plays "confirm" instead
		b.focus_mode = Control.FOCUS_NONE
		b.custom_minimum_size = Vector2(250, 34)
		b.text = ""
		var type: String = c.type
		b.pressed.connect(func() -> void:
			view.emit_intent(&"call_in", {"type": type})
			AudioManager.play_ui(&"confirm"))
		var row := HBoxContainer.new()
		row.mouse_filter = Control.MOUSE_FILTER_IGNORE
		row.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
		row.offset_left = 6
		row.offset_right = -8
		row.add_theme_constant_override("separation", 8)
		var glyph := Control.new()
		glyph.custom_minimum_size = Vector2(24, 34)
		glyph.mouse_filter = Control.MOUSE_FILTER_IGNORE
		glyph.draw.connect(func() -> void:
			UiPalette.draw_glyph(glyph, type, Rect2(Vector2(0, 9), Vector2(24, 16)), UiPalette.TEXT_DIM if b.disabled else UiPalette.OWN, 1.5))
		row.add_child(glyph)
		var name_l := Label.new()
		name_l.text = "%s  ·  %d" % [c.name, c.men]
		name_l.add_theme_font_size_override("font_size", 14)
		name_l.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		name_l.mouse_filter = Control.MOUSE_FILTER_IGNORE
		row.add_child(name_l)
		var cost := Label.new()
		cost.text = "%d MP" % c.cost
		cost.add_theme_font_size_override("font_size", 14)
		cost.add_theme_color_override("font_color", UiPalette.ACCENT)
		cost.mouse_filter = Control.MOUSE_FILTER_IGNORE
		row.add_child(cost)
		b.add_child(row)
		b.draw.connect(glyph.queue_redraw)
		_list.add_child(b)
		_buttons[type] = b


func _update_queue(queue: Array) -> void:
	while _queue.get_child_count() < queue.size():
		var row := HBoxContainer.new()
		row.add_theme_constant_override("separation", 6)
		var l := Label.new()
		l.add_theme_font_size_override("font_size", 13)
		l.custom_minimum_size.x = 150
		row.add_child(l)
		var bar := ProgressBar.new()
		bar.show_percentage = false
		bar.custom_minimum_size = Vector2(60, 8)
		bar.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		bar.size_flags_vertical = Control.SIZE_SHRINK_CENTER
		row.add_child(bar)
		_queue.add_child(row)
	while _queue.get_child_count() > queue.size():
		var last := _queue.get_child(_queue.get_child_count() - 1)
		_queue.remove_child(last)
		last.queue_free()
	for i in queue.size():
		var q: Dictionary = queue[i]
		var row := _queue.get_child(i)
		var total := maxf(float(q.get("total_s", 1.0)), 0.01)
		var left := float(q.get("remaining_s", 0.0))
		(row.get_child(0) as Label).text = "%s  %ds" % [UiPalette.SQUAD_NAMES.get(q.type, q.type), ceili(left)]
		(row.get_child(1) as ProgressBar).value = (1.0 - left / total) * 100.0
