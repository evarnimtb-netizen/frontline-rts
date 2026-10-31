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
##       --capture /abs/out.png [--capture_after 2.5] [--capture_setup NAME]
## saves the viewport after the delay and quits. --capture_setup is read by
## the scene (e.g. match.gd: "hud", "dig", "pause", "end").

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

var _capture_path := ""
var _capture_after := 2.5
var _capture_setup := ""


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
		i += 1
	if _capture_path != "":
		_run_capture.call_deferred()


func capture_setup() -> String:
	return _capture_setup


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


func _run_capture() -> void:
	await get_tree().create_timer(_capture_after, true, false, true).timeout
	await RenderingServer.frame_post_draw
	var img := get_viewport().get_texture().get_image()
	var err := img.save_png(_capture_path)
	if err != OK:
		push_error("Session: capture failed (%s): %s" % [error_string(err), _capture_path])
	else:
		print("Session: captured %s (%dx%d)" % [_capture_path, img.get_width(), img.get_height()])
	quit_game(0 if err == OK else 1)
