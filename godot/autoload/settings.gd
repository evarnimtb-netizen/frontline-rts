extends Node
## Settings (autoload): client options saved to user://settings.cfg and
## applied to the window, the 3D viewport and the audio buses. Presentation
## only; nothing here reaches the sim.
##
## Values: resolution (windowed size), fullscreen, render_scale (3D scaling,
## FSR 1 below 1.0), master_volume and sfx_volume (linear 0..1),
## minimap_rotate (turn the minimap with the camera; default on) and
## minimap_dock ("top_left" or "bottom_left"). Change one with set_value();
## it applies at once and saves.

signal changed(key: String, value: Variant)

const PATH := "user://settings.cfg"
const SECTION := "settings"
const RESOLUTIONS: Array[Vector2i] = [
	Vector2i(1280, 720),
	Vector2i(1280, 800),
	Vector2i(1600, 900),
	Vector2i(1920, 1080),
	Vector2i(2560, 1440),
]
const DEFAULTS := {
	"resolution": Vector2i(1600, 900),
	"fullscreen": false,
	"render_scale": 1.0,
	"master_volume": 0.8,
	"sfx_volume": 0.8,
	"minimap_rotate": true,
	"minimap_dock": "top_left",
}
const MINIMAP_DOCKS: Array[String] = ["top_left", "bottom_left"]

var values := DEFAULTS.duplicate()
## True once the file had a resolution: until then the window keeps the
## project / command-line size.
var _has_resolution := false


func _ready() -> void:
	process_mode = Node.PROCESS_MODE_ALWAYS
	load_settings()
	apply_all()


func load_settings() -> void:
	var cfg := ConfigFile.new()
	if cfg.load(PATH) != OK:
		return
	for key in DEFAULTS:
		if cfg.has_section_key(SECTION, key):
			var v: Variant = cfg.get_value(SECTION, key)
			if typeof(v) == typeof(DEFAULTS[key]):
				values[key] = v
	_has_resolution = cfg.has_section_key(SECTION, "resolution")
	values.render_scale = clampf(values.render_scale, 0.5, 1.0)
	values.master_volume = clampf(values.master_volume, 0.0, 1.0)
	values.sfx_volume = clampf(values.sfx_volume, 0.0, 1.0)
	if not values.minimap_dock in MINIMAP_DOCKS:
		values.minimap_dock = DEFAULTS.minimap_dock


func save_settings() -> void:
	var cfg := ConfigFile.new()
	for key in values:
		cfg.set_value(SECTION, key, values[key])
	var err := cfg.save(PATH)
	if err != OK:
		push_warning("Settings: could not save %s (%s)" % [PATH, error_string(err)])


func get_value(key: String) -> Variant:
	return values.get(key, DEFAULTS.get(key))


func set_value(key: String, value: Variant) -> void:
	if not DEFAULTS.has(key):
		push_warning("Settings: unknown key %s" % key)
		return
	values[key] = value
	if key == "resolution":
		_has_resolution = true
	_apply(key)
	save_settings()
	changed.emit(key, value)


func apply_all() -> void:
	# The render benchmark (bench.tscn) and runs with an explicit
	# --resolution keep their own display setup; only audio applies.
	var args := OS.get_cmdline_args()
	var keep_display := args.has("res://bench.tscn") or args.has("--resolution")
	for key in values:
		if keep_display and key in ["render_scale", "fullscreen", "resolution"]:
			continue
		_apply(key)


func _apply(key: String) -> void:
	match key:
		"master_volume":
			AudioManager.set_bus_linear(&"Master", values.master_volume)
		"sfx_volume":
			AudioManager.set_bus_linear(&"SFX", values.sfx_volume)
		"render_scale":
			var vp := get_viewport()
			var s: float = values.render_scale
			vp.scaling_3d_scale = s
			vp.scaling_3d_mode = Viewport.SCALING_3D_MODE_FSR if s < 0.99 else Viewport.SCALING_3D_MODE_BILINEAR
		"fullscreen", "resolution":
			_apply_window()


func _apply_window() -> void:
	if DisplayServer.get_name() == "headless":
		return
	var win := get_window()
	if values.fullscreen:
		win.mode = Window.MODE_FULLSCREEN
		return
	if win.mode == Window.MODE_FULLSCREEN or win.mode == Window.MODE_EXCLUSIVE_FULLSCREEN:
		win.mode = Window.MODE_WINDOWED
	if not _has_resolution:
		return
	var size: Vector2i = values.resolution
	var screen := DisplayServer.window_get_current_screen()
	var usable := DisplayServer.screen_get_usable_rect(screen)
	size = size.clamp(Vector2i(960, 540), usable.size)
	win.size = size
	win.position = usable.position + (usable.size - size) / 2
