extends Control
## End screen: victory or defeat, why, match length, and per-side stats
## (kills, losses, points held on average, squads called in, trench dug), then
## back to the menu.

const REASONS := {
	"tickets": ["Enemy tickets exhausted.", "Our tickets are exhausted."],
	"annihilation": ["The enemy has no troops left to field.", "We have no troops left to field."],
	"surrender": ["The enemy surrendered.", "We surrendered."],
}
const ROWS := [
	["kills", "Kills", "%d"],
	["losses", "Losses", "%d"],
	# Point-seconds summed over points, shown per second of match: the
	# average number of points held (a sum of times read like a clock).
	["points_held_s", "Avg. points held", "avg"],
	["squads_called", "Squads called in", "%d"],
	["trench_m", "Trench dug", "%d m"],
	# S5a: manpower spent on ammunition, by kind.
	["ammo_mp", "Ammunition", "ammo"],
]

var view: MatchView
var _title: Label
var _reason: Label
var _grid: GridContainer


func bind(v: MatchView) -> void:
	view = v
	visible = false
	mouse_filter = Control.MOUSE_FILTER_STOP
	var dim := ColorRect.new()
	dim.color = Color(0.03, 0.035, 0.04, 0.78)
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(dim)
	var center := CenterContainer.new()
	center.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	center.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(center)
	var panel := PanelContainer.new()
	panel.custom_minimum_size = Vector2(560, 0)
	center.add_child(panel)
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 12)
	panel.add_child(box)
	_title = Label.new()
	_title.theme_type_variation = &"TitleLabel"
	_title.add_theme_font_size_override("font_size", 72)
	_title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	box.add_child(_title)
	_reason = Label.new()
	_reason.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_reason.theme_type_variation = &"DimLabel"
	_reason.add_theme_font_size_override("font_size", 17)
	box.add_child(_reason)
	var rule := ColorRect.new()
	rule.color = Color(UiPalette.ACCENT, 0.4)
	rule.custom_minimum_size.y = 1
	box.add_child(rule)
	_grid = GridContainer.new()
	_grid.columns = 3
	_grid.add_theme_constant_override("h_separation", 28)
	_grid.add_theme_constant_override("v_separation", 8)
	_grid.size_flags_horizontal = Control.SIZE_SHRINK_CENTER
	box.add_child(_grid)
	var back := Button.new()
	back.text = "BACK TO MENU"
	back.theme_type_variation = &"PrimaryButton"
	back.custom_minimum_size = Vector2(240, 50)
	back.size_flags_horizontal = Control.SIZE_SHRINK_CENTER
	back.pressed.connect(func() -> void: view.emit_intent(&"quit_to_menu", {}))
	box.add_child(back)


func show_result(s: Dictionary) -> void:
	var result: Dictionary = s.get("result", {})
	var won: bool = result.get("status", "") == "won"
	_title.text = "VICTORY" if won else "DEFEAT"
	_title.add_theme_color_override("font_color", UiPalette.ACCENT if won else UiPalette.DANGER)
	var reasons: Array = REASONS.get(result.get("reason", ""), ["", ""])
	var why: String = reasons[0] if won else reasons[1]
	# Net mode: a surrender the relay made for a player who stayed silent
	# for the whole 60 s wait.
	var loser := view.local_player if not won else view.foe_slot()
	if view.net_mode and result.get("reason", "") == "surrender" and view.net_timed_out_slot == loser:
		var who := String(view.player(loser).get("name", ""))
		why = "%s lost the connection and timed out." % (who if who != "" and won else ("The enemy" if won else "You"))
	_reason.text = "%s   Match time %s" % [why, UiPalette.mmss(s.get("time_s", 0.0))]
	for c in _grid.get_children():
		c.queue_free()
	var me := view.player(view.local_player)
	var them := view.player(view.foe_slot())
	_cell("", UiPalette.TEXT_DIM)
	_cell("%s (you)" % UiPalette.FACTION_NAMES.get(me.get("faction", ""), "You"), UiPalette.faction_color(me.get("faction", "")).lightened(0.3))
	# Net mode: the other player's name instead of "AI".
	var foe := String(them.get("name", "")) if view.net_mode else ""
	_cell("%s (%s)" % [UiPalette.FACTION_NAMES.get(them.get("faction", ""), "?"), foe if foe != "" else ("opponent" if view.net_mode else "AI")], UiPalette.faction_color(them.get("faction", "")).lightened(0.3))
	var stats: Array = s.get("stats", [])
	for row in ROWS:
		_cell(row[1], UiPalette.TEXT_DIM)
		for p in [view.local_player, view.foe_slot()]:
			var st: Dictionary = stats[p] if p < stats.size() else {}
			if row[2] == "ammo":
				_cell(ammo_text(st.get("ammo_mp", {})), UiPalette.TEXT)
				continue
			var v: float = float(st.get(row[0], 0))
			_cell(_format(row[2], v, float(s.get("time_s", 0.0))), UiPalette.TEXT)
	visible = true
	Session.last_result = {"result": result, "stats": stats, "time_s": s.get("time_s", 0.0)}


## S5a: "small arms 34, explosives 21, shells 188 MP" ("-" without data).
static func ammo_text(am: Variant) -> String:
	if not am is Dictionary or (am as Dictionary).is_empty():
		return "-"
	var d: Dictionary = am
	return "small arms %d, explosives %d, shells %d MP" % [roundi(float(d.get("small_arms", 0.0))), roundi(float(d.get("explosives", 0.0))), roundi(float(d.get("shells", 0.0)))]


func _format(fmt: String, v: float, time_s: float) -> String:
	match fmt:
		"time":
			return UiPalette.mmss(v)
		"avg":
			return "%.1f" % (v / time_s if time_s > 0.0 else 0.0)
	return fmt % int(v)


func _cell(text: String, col: Color) -> void:
	var l := Label.new()
	l.text = text
	l.add_theme_color_override("font_color", col)
	l.add_theme_font_size_override("font_size", 17)
	_grid.add_child(l)


func update_state(_s: Dictionary) -> void:
	pass
