class_name SettingsPanel
extends VBoxContainer
## Settings form shared by the main menu and the pause menu. Every control
## writes through the Settings autoload, which applies and saves at once.

signal closed

var _res: OptionButton
var _full: CheckButton
var _scale: HSlider
var _scale_lbl: Label
var _master: HSlider
var _master_lbl: Label
var _sfx: HSlider
var _sfx_lbl: Label
var _mm_rotate: CheckButton
var _mm_dock: OptionButton


func _ready() -> void:
	add_theme_constant_override("separation", 12)
	var head := Label.new()
	head.text = "SETTINGS"
	head.theme_type_variation = &"HeaderLabel"
	add_child(head)

	var grid := GridContainer.new()
	grid.columns = 3
	grid.add_theme_constant_override("h_separation", 16)
	grid.add_theme_constant_override("v_separation", 10)
	add_child(grid)

	_res = OptionButton.new()
	_res.custom_minimum_size.x = 220
	for r in Settings.RESOLUTIONS:
		_res.add_item("%d × %d" % [r.x, r.y])
	_res.item_selected.connect(func(i: int) -> void: Settings.set_value("resolution", Settings.RESOLUTIONS[i]))
	_row(grid, "Resolution", _res, _dim("windowed"))

	_full = CheckButton.new()
	_full.toggled.connect(func(on: bool) -> void: Settings.set_value("fullscreen", on))
	_row(grid, "Fullscreen", _full, Control.new())

	_scale_lbl = _dim("")
	_scale = _slider(0.5, 1.0, 0.05)
	_scale.value_changed.connect(func(v: float) -> void:
		Settings.set_value("render_scale", v)
		_scale_lbl.text = "%d %%" % roundi(v * 100.0))
	_row(grid, "Render scale", _scale, _scale_lbl)

	_master_lbl = _dim("")
	_master = _slider(0.0, 1.0, 0.01)
	_master.value_changed.connect(func(v: float) -> void:
		Settings.set_value("master_volume", v)
		_master_lbl.text = "%d %%" % roundi(v * 100.0))
	_row(grid, "Master volume", _master, _master_lbl)

	_sfx_lbl = _dim("")
	_sfx = _slider(0.0, 1.0, 0.01)
	_sfx.value_changed.connect(func(v: float) -> void:
		Settings.set_value("sfx_volume", v)
		_sfx_lbl.text = "%d %%" % roundi(v * 100.0))
	_row(grid, "Effects volume", _sfx, _sfx_lbl)

	_mm_rotate = CheckButton.new()
	_mm_rotate.toggled.connect(func(on: bool) -> void: Settings.set_value("minimap_rotate", on))
	_row(grid, "Rotate minimap", _mm_rotate, _dim("off = north-up"))

	_mm_dock = OptionButton.new()
	_mm_dock.custom_minimum_size.x = 220
	_mm_dock.add_item("Top left")
	_mm_dock.add_item("Bottom left")
	_mm_dock.item_selected.connect(func(i: int) -> void: Settings.set_value("minimap_dock", Settings.MINIMAP_DOCKS[i]))
	_row(grid, "Minimap position", _mm_dock, Control.new())

	var note := _dim("Saved to user://settings.cfg")
	add_child(note)

	var back := Button.new()
	back.text = "Back"
	back.custom_minimum_size.x = 140
	back.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	back.pressed.connect(func() -> void: closed.emit())
	add_child(back)
	refresh()


## Re-reads the controls from Settings (call before showing the panel).
func refresh() -> void:
	var r: Vector2i = Settings.get_value("resolution")
	var idx := Settings.RESOLUTIONS.find(r)
	_res.select(idx if idx >= 0 else 2)
	_full.set_pressed_no_signal(Settings.get_value("fullscreen"))
	_scale.set_value_no_signal(Settings.get_value("render_scale"))
	_scale_lbl.text = "%d %%" % roundi(_scale.value * 100.0)
	_master.set_value_no_signal(Settings.get_value("master_volume"))
	_master_lbl.text = "%d %%" % roundi(_master.value * 100.0)
	_sfx.set_value_no_signal(Settings.get_value("sfx_volume"))
	_sfx_lbl.text = "%d %%" % roundi(_sfx.value * 100.0)
	_mm_rotate.set_pressed_no_signal(Settings.get_value("minimap_rotate"))
	_mm_dock.select(maxi(Settings.MINIMAP_DOCKS.find(Settings.get_value("minimap_dock")), 0))


func _row(grid: GridContainer, label: String, control: Control, suffix: Control) -> void:
	var l := Label.new()
	l.text = label
	l.custom_minimum_size.x = 150
	grid.add_child(l)
	control.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	grid.add_child(control)
	suffix.custom_minimum_size.x = 70
	grid.add_child(suffix)


func _slider(lo: float, hi: float, step: float) -> HSlider:
	var s := HSlider.new()
	s.min_value = lo
	s.max_value = hi
	s.step = step
	s.custom_minimum_size = Vector2(220, 20)
	return s


func _dim(text: String) -> Label:
	var l := Label.new()
	l.text = text
	l.theme_type_variation = &"DimLabel"
	return l
