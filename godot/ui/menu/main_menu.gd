extends Control
## Main menu (project main scene): title, Skirmish setup, Multiplayer
## (MultiplayerPanel: host/join, lobby), Settings, Quit. Start writes the
## choice into Session.setup and loads the match scene.
##
## --capture_setup for the S2 menu shots (all on MockNet, no network):
## "mp" (host/join), "mp_errors" (rejected inputs), "lobby" (host, two
## players), "lobby_guest", "lobby_same_faction", "lobby_mismatch",
## "countdown", "mp_disconnected".

const DIFFICULTY_TEXT := {
	"easy": "Slower to react, lower income, simple attack plans.",
	"normal": "Plays the points and digs in where it holds ground.",
	"hard": "Fast reactions, extra income, coordinated mortar fire.",
}

const MP_CAPTURES := ["mp", "mp_errors", "lobby", "lobby_guest", "lobby_same_faction", "lobby_mismatch", "countdown", "mp_disconnected"]

var _faction := "germany"
## S15 M3: the AI's faction ("" = the rules' default opponent).
var _opponent := ""
var _difficulty := "normal"
var _map := "kessel_ridge"
var _mode := "1v1"

@onready var _content: PanelContainer = %Content
@onready var _skirmish_btn: Button = %SkirmishButton
@onready var _mp_btn: Button = %MultiplayerButton
@onready var _settings_btn: Button = %SettingsButton
@onready var _quit_btn: Button = %QuitButton

var _skirmish: Control
var _settings: SettingsPanel
var _mp: MultiplayerPanel
var _faction_btns := {}
var _opp_section: Label
var _opp_row: HBoxContainer
var _opp_btns := {}
var _diff_btns := {}
var _diff_note: Label
var _preview: TextureRect
var _map_pick: OptionButton
var _map_ids: Array = []
var _map_stats: Label
var _map_blurb: Label


func _ready() -> void:
	_faction = Session.setup.faction
	# S15 M3: the rules' factions; a stored choice they no longer offer
	# falls back to the first.
	var ids: Array = Session.faction_ids()
	if not ids.has(_faction):
		_faction = ids[0]
	_opponent = String(Session.setup.get("opponent", ""))
	_difficulty = Session.setup.difficulty
	_map = Session.setup.map
	_mode = Session.setup.get("mode", "1v1")
	_skirmish = _build_skirmish()
	_content.add_child(_skirmish)
	_settings = SettingsPanel.new()
	_settings.closed.connect(_show.bind("skirmish"))
	_content.add_child(_settings)
	_mp = MultiplayerPanel.new()
	# Autoplay takes screenshots that get committed: example addresses,
	# not this machine's.
	if Session.autoplay_net_role != "":
		_mp.addresses_override = MultiplayerPanel.CAPTURE_ADDRESSES
	_content.add_child(_mp)
	# Both players need the same build (the relay checks it): show its id.
	var version := get_node_or_null(^"Margin/Columns/Left/Version") as Label
	if version != null and ClassDB.class_exists(&"FrontlineNet"):
		var probe: Object = ClassDB.instantiate(&"FrontlineNet")
		if probe.has_method("build_id"):
			var gv := Engine.get_version_info()
			version.text = "Build %s  ·  Godot %d.%d.%d" % [probe.build_id(), gv.major, gv.minor, gv.patch]
	_skirmish_btn.pressed.connect(_show.bind("skirmish"))
	_mp_btn.pressed.connect(_show.bind("multiplayer"))
	_settings_btn.pressed.connect(_show.bind("settings"))
	_quit_btn.pressed.connect(func() -> void: Session.quit_game())
	var cap := Session.capture_setup()
	if cap == "settings":
		_show("settings")
	elif cap in MP_CAPTURES:
		_stage_mp(cap)
	elif Session.menu_page == "multiplayer" or (Session.net != null and Session.net.status().get("state", "idle") != "idle"):
		Session.menu_page = ""
		_show("multiplayer")
	else:
		_show("skirmish")
	_skirmish_btn.grab_focus()
	AudioManager.start_ambient(&"wind")
	if Session.start_error != "":
		_show_start_error(Session.start_error)
		Session.start_error = ""


## A match that could not start (broken mod data, S3b): the loader's
## message, under the Start button, until the next start.
func _show_start_error(text: String) -> void:
	var l := Label.new()
	l.name = "StartError"
	l.text = "The match could not start:\n%s" % text
	l.add_theme_color_override("font_color", UiPalette.DANGER)
	l.add_theme_font_size_override("font_size", 14)
	l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	l.custom_minimum_size.x = 520
	_skirmish.add_child(l)
	AudioManager.play_ui(&"error")


func _show(which: String) -> void:
	_skirmish.visible = which == "skirmish"
	_settings.visible = which == "settings"
	_mp.visible = which == "multiplayer"
	if which == "settings":
		_settings.refresh()
	elif which == "multiplayer":
		_mp.refresh()
	_skirmish_btn.set_pressed_no_signal(which == "skirmish")
	_mp_btn.set_pressed_no_signal(which == "multiplayer")
	_settings_btn.set_pressed_no_signal(which == "settings")


## Shows a page: "skirmish", "multiplayer" or "settings" (Session's
## --autoplay_net shows the lobby with it).
func show_page(which: String) -> void:
	_show(which)


## Screenshot staging for the multiplayer pages on a MockNet.
func _stage_mp(what: String) -> void:
	var mock := MockNet.new()
	mock.auto = false
	# Frozen clock: the countdown shot shows "3", not wherever it got to.
	mock.manual_clock = true
	mock.set_name("Erik" if what != "lobby_guest" else "Sam")
	Session.net = mock
	_mp.addresses_override = MultiplayerPanel.CAPTURE_ADDRESSES
	_mp.name_edit.text = "Erik"
	_mp.update_host_hint()
	match what:
		"mp_errors":
			_mp.host_port.text = "80"
			_mp.on_host()
			_mp.join_addr.text = "192.168.1.300:47015"
			_mp.on_join()
		"mp_disconnected":
			mock.stage("rejected")
		"lobby", "lobby_guest", "lobby_same_faction", "lobby_mismatch", "countdown":
			_mp.where = "port 47015" if what != "lobby_guest" else "192.168.1.20:47015"
			mock.stage(what)
	_show("multiplayer")


func _build_skirmish() -> Control:
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 14)

	var head := Label.new()
	head.text = "SKIRMISH"
	head.theme_type_variation = &"HeaderLabel"
	box.add_child(head)
	box.add_child(_dim("One map, one AI opponent. Hold the points, dig in, bleed their tickets."))

	# S17: the mode pick (1v1; 2v2 with an AI ally on Harrow Cross).
	box.add_child(_section("Mode"))
	var mode_row := HBoxContainer.new()
	mode_row.add_theme_constant_override("separation", 10)
	var mode_group := ButtonGroup.new()
	for md in Session.MODES:
		var mb := _choice("1 v 1" if md == "1v1" else "2 v 2 (AI ally)", mode_group, md == _mode)
		mb.custom_minimum_size = Vector2(170, 40)
		mb.pressed.connect(_set_mode.bind(md))
		mode_row.add_child(mb)
	box.add_child(mode_row)

	box.add_child(_section("Faction"))
	var fac_row := HBoxContainer.new()
	fac_row.add_theme_constant_override("separation", 10)
	var fac_group := ButtonGroup.new()
	for f in Session.faction_ids():
		var b := _choice(UiPalette.faction_name(f), fac_group, f == _faction)
		b.custom_minimum_size = Vector2(170, 48)
		b.add_theme_color_override("font_pressed_color", UiPalette.faction_color(f).lightened(0.35))
		var stripe := ColorRect.new()
		stripe.color = UiPalette.faction_color(f)
		stripe.mouse_filter = Control.MOUSE_FILTER_IGNORE
		stripe.set_anchors_and_offsets_preset(Control.PRESET_LEFT_WIDE)
		stripe.offset_right = 5
		b.add_child(stripe)
		b.pressed.connect(func() -> void:
			_faction = f
			_rebuild_opponents())
		_faction_btns[f] = b
		fac_row.add_child(b)
	box.add_child(fac_row)

	# S15 M3: the opponent picker; hidden while the faction has one
	# opponent (the shipped rules: today's menu).
	_opp_section = _section("Opponent")
	box.add_child(_opp_section)
	_opp_row = HBoxContainer.new()
	_opp_row.add_theme_constant_override("separation", 10)
	box.add_child(_opp_row)
	_rebuild_opponents()

	box.add_child(_section("AI difficulty"))
	var diff_row := HBoxContainer.new()
	diff_row.add_theme_constant_override("separation", 10)
	var diff_group := ButtonGroup.new()
	for d in Session.DIFFICULTIES:
		var b := _choice(d.capitalize(), diff_group, d == _difficulty)
		b.custom_minimum_size = Vector2(110, 40)
		b.pressed.connect(func() -> void:
			_difficulty = d
			_diff_note.text = DIFFICULTY_TEXT[d])
		_diff_btns[d] = b
		diff_row.add_child(b)
	box.add_child(diff_row)
	_diff_note = _dim(DIFFICULTY_TEXT[_difficulty])
	box.add_child(_diff_note)

	box.add_child(_section("Map"))
	var map_row := HBoxContainer.new()
	map_row.add_theme_constant_override("separation", 16)
	_preview = TextureRect.new()
	MockMap.use(_map)
	_preview.texture = ImageTexture.create_from_image(MockMap.make_image(160))
	_preview.custom_minimum_size = Vector2(160, 160)
	_preview.stretch_mode = TextureRect.STRETCH_SCALE
	_preview.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	_preview.draw.connect(_draw_preview_marks)
	map_row.add_child(_preview)
	var map_info := VBoxContainer.new()
	map_info.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	# S17 (merge with S16): the pick lists the maps of the mode (1v1: two
	# bases; 2v2: four bases) and stays enabled.
	var map_pick := OptionButton.new()
	_map_pick = map_pick
	_fill_map_pick()
	map_pick.item_selected.connect(func(i: int) -> void: _pick_map(_map_ids[i]))
	map_pick.custom_minimum_size.x = 220
	map_pick.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	map_info.add_child(map_pick)
	var info: Dictionary = Session.map_info(_map)
	_map_stats = _dim("")
	map_info.add_child(_map_stats)
	var blurb := Label.new()
	_map_blurb = blurb
	blurb.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	blurb.custom_minimum_size.x = 300
	map_info.add_child(blurb)
	map_row.add_child(map_info)
	box.add_child(map_row)
	_show_map_text(info)

	var spacer := Control.new()
	spacer.custom_minimum_size.y = 6
	box.add_child(spacer)
	var start := Button.new()
	start.text = "START SKIRMISH"
	start.theme_type_variation = &"PrimaryButton"
	start.custom_minimum_size = Vector2(260, 54)
	start.size_flags_horizontal = Control.SIZE_SHRINK_END
	start.pressed.connect(_on_start)
	box.add_child(start)
	return box


func _show_map_text(info: Dictionary) -> void:
	_map_stats.text = "%d × %d m · %d capture points · %d players" % [
		info.size_m, info.size_m, info.points, info.get("bases", 2)]
	_map_blurb.text = info.blurb


func _pick_map(id: String) -> void:
	_map = id
	MockMap.use(id)
	_preview.texture = ImageTexture.create_from_image(MockMap.make_image(160))
	_preview.queue_redraw()
	_show_map_text(Session.map_info(id))


## The opponent row for the chosen faction: the rules' opponents, the default
## first and pressed; one entry or none hides the picker and sends no
## opponent (the bridge then takes the default, as before S15).
func _rebuild_opponents() -> void:
	for c in _opp_row.get_children():
		_opp_row.remove_child(c)
		c.queue_free()
	_opp_btns.clear()
	var list: Array = Session.opponents_of(_faction)
	if not list.has(_opponent):
		_opponent = String(list[0]) if not list.is_empty() else ""
	var picker := list.size() > 1
	_opp_section.visible = picker
	_opp_row.visible = picker
	var group := ButtonGroup.new()
	for o in list:
		var b := _choice(UiPalette.faction_name(o), group, o == _opponent)
		b.custom_minimum_size = Vector2(170, 40)
		b.add_theme_color_override("font_pressed_color", UiPalette.faction_color(o).lightened(0.35))
		var stripe := ColorRect.new()
		stripe.color = UiPalette.faction_color(o)
		stripe.mouse_filter = Control.MOUSE_FILTER_IGNORE
		stripe.set_anchors_and_offsets_preset(Control.PRESET_LEFT_WIDE)
		stripe.offset_right = 5
		b.add_child(stripe)
		b.pressed.connect(func() -> void: _opponent = o)
		_opp_btns[o] = b
		_opp_row.add_child(b)


func _draw_preview_marks() -> void:
	var s := _preview.size
	var font := get_theme_default_font()
	for i in MockMap.BASES.size():
		var uv: Vector2 = (MockMap.BASES[i] + Vector2(MockMap.HALF, MockMap.HALF)) / MockMap.SIZE_M
		var col := UiPalette.OWN if i == 0 else UiPalette.ENEMY
		_preview.draw_rect(Rect2(uv * s - Vector2(7, 7), Vector2(14, 14)), Color(col, 0.85), false, 2.0)
	for p in MockMap.POINTS:
		var uv: Vector2 = (p.pos + Vector2(MockMap.HALF, MockMap.HALF)) / MockMap.SIZE_M
		_preview.draw_circle(uv * s, 7.0, Color(0.08, 0.09, 0.1, 0.8))
		_preview.draw_string(font, uv * s + Vector2(-4, 5), p.id, HORIZONTAL_ALIGNMENT_LEFT, -1, 13, UiPalette.TEXT)
	_preview.draw_rect(Rect2(Vector2.ZERO, s), UiPalette.BORDER, false, 1.0)


func _on_start() -> void:
	AudioManager.play_ui(&"confirm")
	# An empty opponent is the default one (the bridge reads "" as absent).
	var picker := Session.opponents_of(_faction).size() > 1
	Session.start_skirmish({"faction": _faction, "difficulty": _difficulty, "map": _map, "mode": _mode, "opponent": _opponent if picker else ""})


## S17: picks the mode; the map stays if it plays the mode, else the
## first map that does.
func _set_mode(md: String) -> void:
	_mode = md
	if not _map in Session.maps_for_mode(md):
		_map = Session.map_for_mode(md)
	if _map_pick != null:
		_fill_map_pick()
		_pick_map(_map)


## S17: the map pick holds the maps of the current mode.
func _fill_map_pick() -> void:
	_map_ids = Session.maps_for_mode(_mode)
	if not _map in _map_ids:
		_map = Session.map_for_mode(_mode)
	_map_pick.clear()
	for id in _map_ids:
		_map_pick.add_item(Session.MAPS[id].name)
	_map_pick.select(_map_ids.find(_map))


func _section(text: String) -> Label:
	var l := Label.new()
	l.text = text.to_upper()
	l.theme_type_variation = &"DimLabel"
	l.add_theme_font_size_override("font_size", 13)
	return l


func _choice(text: String, group: ButtonGroup, on: bool) -> Button:
	var b := Button.new()
	b.text = text
	b.toggle_mode = true
	b.button_group = group
	b.button_pressed = on
	return b


func _dim(text: String) -> Label:
	var l := Label.new()
	l.text = text
	l.theme_type_variation = &"DimLabel"
	l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	return l


func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed("ui_cancel") and _settings.visible:
		_show("skirmish")
		get_viewport().set_input_as_handled()
