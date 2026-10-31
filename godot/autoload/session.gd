extends Node
## Session (autoload): the skirmish chosen in the menu, scene changes, and
## the screenshot/capture helper used by docs and CI.
##
## `setup` is what the match scene (and later the Rust bridge) reads to
## build a MatchSetup: faction, AI difficulty, map id and seed. It is plain
## data, so the integrator can map it straight onto the content resolver.
##
## Capture (windowed run, not --headless):
##   Godot --path godot --resolution 1600x900 [res://scene.tscn] -- \
##       --capture /abs/out.png [--capture_after 2.5] [--capture_setup NAME] \
##       [--capture_size 1600x900]
## saves the viewport after the delay and quits. --capture_setup is read by
## the scene (e.g. match.gd: "hud", "throw", "dig", "pause", "end");
## --capture_size forces the window to that many pixels first.
##
## Autoplay (windowed; S1 "Done when" 4):
##   Godot --path godot --resolution 1600x900 -- --autoplay 6 \
##       [--autoplay_speed 3] [--autoplay_shots /abs/dir] [--seed N] \
##       [--faction usa] [--difficulty hard] [--tickets N]
## shows the menu, starts a skirmish, and lets match/autoplay.gd play the
## local side for that many game minutes, saving screenshots, then quits.

const MENU_SCENE := "res://ui/menu/main_menu.tscn"
const MATCH_SCENE := "res://match/match.tscn"

const FACTIONS := ["germany", "usa"]
const DIFFICULTIES := ["easy", "normal", "hard"]
const MAPS := {
	"kessel_ridge": {
		"name": "Kessel Ridge",
		"size_m": 1024,
		"points": 5,
		"blurb": "Rolling farmland split by a wooded ridge. Five points, the middle one on the crest.",
	},
}

var setup := {
	"faction": "germany",
	"difficulty": "normal",
	"map": "kessel_ridge",
	"seed": 1,
}
## Filled by the match when it ends, for anything shown after it.
var last_result := {}

## Autoplay: game minutes to play (0 = off), sim speed, screenshot folder.
var autoplay_minutes := 0.0
var autoplay_speed := 3.0
var autoplay_shots := ""
## Match overrides for scripted runs (0 = rules default).
var tickets_override := 0

var _capture_path := ""
var _capture_after := 2.5
var _capture_setup := ""
## Window size in pixels to force before a capture (macOS sometimes opens
## the window zoomed to the whole screen); zero = keep.
var _capture_size := Vector2i.ZERO


func _ready() -> void:
	process_mode = Node.PROCESS_MODE_ALWAYS
	# Closing the window goes through quit_game() too (audio shutdown).
	get_tree().auto_accept_quit = false
	var args := OS.get_cmdline_user_args()
	var i := 0
	while i < args.size():
		var a := args[i]
		var v := args[i + 1] if i + 1 < args.size() else ""
		match a:
			"--capture":
				_capture_path = v
				i += 1
			"--capture_after":
				_capture_after = v.to_float()
				i += 1
			"--capture_setup":
				_capture_setup = v
				i += 1
			"--capture_size":
				var wh := v.split("x")
				if wh.size() == 2:
					_capture_size = Vector2i(wh[0].to_int(), wh[1].to_int())
				i += 1
			"--faction":
				if v in FACTIONS:
					setup.faction = v
				i += 1
			"--difficulty":
				if v in DIFFICULTIES:
					setup.difficulty = v
				i += 1
			"--seed":
				setup.seed = v.to_int()
				i += 1
			"--autoplay":
				autoplay_minutes = maxf(v.to_float(), 0.5) if v.is_valid_float() else 5.0
				if v.is_valid_float():
					i += 1
			"--autoplay_speed":
				autoplay_speed = clampf(v.to_float(), 0.25, 8.0)
				i += 1
			"--autoplay_shots":
				autoplay_shots = v
				i += 1
			"--tickets":
				tickets_override = maxi(v.to_int(), 0)
				i += 1
		i += 1
	if _capture_path != "":
		_run_capture.call_deferred()
	elif autoplay_enabled():
		_run_autoplay_menu.call_deferred()


func capture_setup() -> String:
	return _capture_setup


func autoplay_enabled() -> bool:
	return autoplay_minutes > 0.0


## Folder for autoplay screenshots (absolute): --autoplay_shots, else the
## repository's docs/slices/screens.
func autoplay_dir() -> String:
	if autoplay_shots != "":
		return autoplay_shots
	return ProjectSettings.globalize_path("res://").path_join("../docs/slices/screens").simplify_path()


## What FrontlineMatch.start_match() takes: the menu choice plus overrides.
func match_setup() -> Dictionary:
	var out := setup.duplicate()
	if tickets_override > 0:
		out.tickets = tickets_override
	return out


## Saves the current frame as PNG (after it is drawn). Returns OK or an error.
func save_screenshot(path: String) -> int:
	await RenderingServer.frame_post_draw
	var img := get_viewport().get_texture().get_image()
	# HiDPI or maximised windows: keep docs screenshots at 1600 px wide.
	if img.get_width() > 1600:
		img.resize(1600, int(1600.0 * img.get_height() / img.get_width()), Image.INTERPOLATE_LANCZOS)
	DirAccess.make_dir_recursive_absolute(path.get_base_dir())
	var err := img.save_png(path)
	if err != OK:
		push_error("Session: screenshot failed (%s): %s" % [error_string(err), path])
	else:
		print("Session: saved %s (%dx%d)" % [path, img.get_width(), img.get_height()])
	return err


func start_skirmish(new_setup: Dictionary) -> void:
	setup.merge(new_setup, true)
	get_tree().paused = false
	get_tree().change_scene_to_file(MATCH_SCENE)


func goto_menu() -> void:
	get_tree().paused = false
	get_tree().change_scene_to_file(MENU_SCENE)


func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		quit_game()


## Quits after silencing audio (see AudioManager.shutdown).
func quit_game(code := 0) -> void:
	AudioManager.shutdown()
	# A few mixer periods, so the audio thread releases the playbacks.
	await get_tree().create_timer(0.15, true, false, true).timeout
	get_tree().quit(code)


func map_info(id: String) -> Dictionary:
	return MAPS.get(id, {})


func _run_autoplay_menu() -> void:
	# The menu is the main scene: let it settle, record it, start the match.
	await get_tree().create_timer(1.5, true, false, true).timeout
	await save_screenshot(autoplay_dir().path_join("s1_autoplay_menu.png"))
	print("autoplay: starting skirmish %s" % [match_setup()])
	start_skirmish({})


func _run_capture() -> void:
	if _capture_size != Vector2i.ZERO and DisplayServer.get_name() != "headless":
		var win := get_window()
		win.mode = Window.MODE_WINDOWED
		win.size = _capture_size
	await get_tree().create_timer(_capture_after, true, false, true).timeout
	await RenderingServer.frame_post_draw
	var img := get_viewport().get_texture().get_image()
	var err := img.save_png(_capture_path)
	if err != OK:
		push_error("Session: capture failed (%s): %s" % [error_string(err), _capture_path])
	else:
		print("Session: captured %s (%dx%d)" % [_capture_path, img.get_width(), img.get_height()])
	quit_game(0 if err == OK else 1)
