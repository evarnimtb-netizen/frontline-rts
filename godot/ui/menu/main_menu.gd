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
var _difficulty := "normal"
var _map := "kessel_ridge"

@onready var _content: PanelContainer = %Content
@onready var _skirmish_btn: Button = %SkirmishButton
@onready var _mp_btn: Button = %MultiplayerButton
@onready var _settings_btn: Button = %SettingsButton
@onready var _quit_btn: Button = %QuitButton

var _skirmish: Control
var _settings: SettingsPanel
var _mp: MultiplayerPanel
var _faction_btns := {}
var _diff_btns := {}
var _diff_note: Label
var _preview: TextureRect


func _ready() -> void:
	_faction = Session.setup.faction
	_difficulty = Session.setup.difficulty
	_map = Session.setup.map
	_skirmish = _build_skirmish()
	_content.add_child(_skirmish)
	_settings = SettingsPanel.new()
	_settings.closed.connect(_show.bind("skirmish"))
	_content.add_child(_settings)
	_mp = MultiplayerPanel.new()
	_content.add_child(_mp)
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

	box.add_child(_section("Faction"))
	var fac_row := HBoxContainer.new()
	fac_row.add_theme_constant_override("separation", 10)
	var fac_group := ButtonGroup.new()
	for f in Session.FACTIONS:
		var b := _choice(UiPalette.FACTION_NAMES[f], fac_group, f == _faction)
		b.custom_minimum_size = Vector2(170, 48)
		b.add_theme_color_override("font_pressed_color", UiPalette.faction_color(f).lightened(0.35))
		var stripe := ColorRect.new()
		stripe.color = UiPalette.faction_color(f)
		stripe.mouse_filter = Control.MOUSE_FILTER_IGNORE
		stripe.set_anchors_and_offsets_preset(Control.PRESET_LEFT_WIDE)
		stripe.offset_right = 5
		b.add_child(stripe)
		b.pressed.connect(func() -> void: _faction = f)
		_faction_btns[f] = b
		fac_row.add_child(b)
	box.add_child(fac_row)

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
	_preview.texture = ImageTexture.create_from_image(MockMap.make_image(160))
	_preview.custom_minimum_size = Vector2(160, 160)
	_preview.stretch_mode = TextureRect.STRETCH_SCALE
	_preview.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	_preview.draw.connect(_draw_preview_marks)
	map_row.add_child(_preview)
	var map_info := VBoxContainer.new()
	map_info.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	var map_pick := OptionButton.new()
	for id in Session.MAPS:
		map_pick.add_item(Session.MAPS[id].name)
	map_pick.custom_minimum_size.x = 220
	map_pick.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	map_info.add_child(map_pick)
	var info: Dictionary = Session.map_info(_map)
	var stats := _dim("%d × %d m · %d capture points · 2 players" % [info.size_m, info.size_m, info.points])
	map_info.add_child(stats)
	var blurb := Label.new()
	blurb.text = info.blurb
	blurb.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	blurb.custom_minimum_size.x = 300
	map_info.add_child(blurb)
	map_row.add_child(map_info)
	box.add_child(map_row)

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
	Session.start_skirmish({"faction": _faction, "difficulty": _difficulty, "map": _map})


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
