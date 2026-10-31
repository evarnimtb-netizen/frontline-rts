extends PanelContainer
## Top bar: own manpower and income, population, both sides' tickets (with
## a drain marker on the side that is bleeding), match clock, menu button.

var view: MatchView
var _faction: Label
var _manpower: Label
var _income: Label
var _pop: Label
var _own_name: Label
var _own_tickets: Label
var _own_bar: ProgressBar
var _enemy_name: Label
var _enemy_tickets: Label
var _enemy_bar: ProgressBar
var _own_drain: Label
var _enemy_drain: Label
var _clock: Label
var _blink := 0.0


func bind(v: MatchView) -> void:
	view = v
	theme_type_variation = &"HudPanel"
	mouse_filter = Control.MOUSE_FILTER_STOP
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 18)
	add_child(row)

	_faction = _label(row, "", 18)
	_faction.add_theme_font_override("font", load("res://ui/fonts/BarlowCondensed-SemiBold.ttf"))
	row.add_child(_sep())
	_manpower = _stat(row, "MANPOWER")
	_income = _label(row, "", 15)
	_income.add_theme_color_override("font_color", UiPalette.OK)
	row.add_child(_sep())
	_pop = _stat(row, "POP")

	var fill_l := Control.new()
	fill_l.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	row.add_child(fill_l)

	_own_name = _label(row, "", 16)
	_own_drain = _label(row, "", 16)
	_own_drain.add_theme_color_override("font_color", UiPalette.DANGER)
	_own_tickets = _label(row, "", 20)
	_own_bar = _bar(row, UiPalette.OWN, ProgressBar.FILL_END_TO_BEGIN)
	var vs := _label(row, "TICKETS", 12)
	vs.add_theme_color_override("font_color", UiPalette.TEXT_DIM)
	_enemy_bar = _bar(row, UiPalette.ENEMY, ProgressBar.FILL_BEGIN_TO_END)
	_enemy_tickets = _label(row, "", 20)
	_enemy_drain = _label(row, "", 16)
	_enemy_drain.add_theme_color_override("font_color", UiPalette.DANGER)
	_enemy_name = _label(row, "", 16)

	var fill_r := Control.new()
	fill_r.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	row.add_child(fill_r)

	_clock = _label(row, "0:00", 20)
	var menu := Button.new()
	menu.text = "Menu  [Esc]"
	menu.theme_type_variation = &"HudButton"
	menu.focus_mode = Control.FOCUS_NONE
	menu.pressed.connect(func() -> void: view.pause_menu.open())
	row.add_child(menu)


func update_state(s: Dictionary) -> void:
	var me := view.player(view.local_player)
	var them := view.player(1 - view.local_player)
	if me.is_empty() or them.is_empty():
		return
	_blink += get_process_delta_time()
	var rules := view.rules()
	var tmax := float(rules.get("tickets_max", 500))
	_faction.text = String(UiPalette.FACTION_NAMES.get(me.faction, me.faction)).to_upper()
	_faction.add_theme_color_override("font_color", UiPalette.faction_color(me.faction).lightened(0.25))
	_manpower.text = "%d" % int(me.manpower)
	_income.text = "+%.1f/s" % float(me.income)
	_pop.text = "%d / %d" % [int(me.pop), int(rules.get("pop_cap", 120))]
	_own_name.text = UiPalette.FACTION_SHORT.get(me.faction, "YOU")
	_enemy_name.text = UiPalette.FACTION_SHORT.get(them.faction, "AI")
	_own_tickets.text = "%d" % int(me.tickets)
	_enemy_tickets.text = "%d" % int(them.tickets)
	_own_bar.value = float(me.tickets) / tmax * 100.0
	_enemy_bar.value = float(them.tickets) / tmax * 100.0
	var on := fmod(_blink, 1.0) < 0.6
	_own_drain.text = "▼" if float(me.get("ticket_drain", 0.0)) > 0.0 and on else " "
	_enemy_drain.text = "▼" if float(them.get("ticket_drain", 0.0)) > 0.0 and on else " "
	_clock.text = UiPalette.mmss(s.get("time_s", 0.0))
	if s.get("paused", false):
		_clock.text += "  PAUSED"


func _label(parent: Control, text: String, font_size: int) -> Label:
	var l := Label.new()
	l.text = text
	l.add_theme_font_size_override("font_size", font_size)
	l.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	parent.add_child(l)
	return l


func _stat(parent: Control, caption: String) -> Label:
	var c := _label(parent, caption, 12)
	c.add_theme_color_override("font_color", UiPalette.TEXT_DIM)
	return _label(parent, "", 20)


func _sep() -> Control:
	var r := ColorRect.new()
	r.color = UiPalette.BORDER
	r.custom_minimum_size = Vector2(1, 22)
	r.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	return r


func _bar(parent: Control, col: Color, mode: int) -> ProgressBar:
	var b := ProgressBar.new()
	b.custom_minimum_size = Vector2(170, 12)
	b.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	b.show_percentage = false
	b.fill_mode = mode
	var fill := StyleBoxFlat.new()
	fill.bg_color = col
	b.add_theme_stylebox_override("fill", fill)
	parent.add_child(b)
	return b
