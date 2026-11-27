extends SceneTree
## Headless smoke check for the presenter: loads every GDScript and scene
## under res:// (skipping .godot/ and bin/), instantiates the menu and the
## match scene, runs them for a few frames, and exits non-zero on any
## failure. Script errors also print to stderr.
##
##   Godot --headless --path godot -s res://tools/check_project.gd
##
## main.tscn and match/match.tscn need the Rust bridge (godot/bin); they
## are loaded and run only when its classes are registered. The real match
## runs a few seconds of the sim against the AI, then (S17) a 2v2 on Harrow
## Cross: you and an AI ally against two AIs.

const RUN_SCENES := ["res://ui/menu/main_menu.tscn", "res://match/match_mock.tscn", "res://match/match.tscn", "res://match/match.tscn#2v2"]
const FRAMES := 240

var _failures := 0
var _scene_i := 0
var _frames := 0


func _initialize() -> void:
	for path in _files("res://", ["gd", "tscn", "tres"]):
		if path in ["res://main.tscn", "res://match/match.tscn"] and not ClassDB.class_exists(&"FrontlineMatch"):
			print("skip %s (bridge not built)" % path)
			continue
		var res := ResourceLoader.load(path, "", ResourceLoader.CACHE_MODE_REUSE)
		if res == null:
			_fail("load failed: %s" % path)
		elif res is GDScript and not (res as GDScript).can_instantiate():
			_fail("script does not compile: %s" % path)
	_next_scene()


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames < FRAMES:
		return false
	_frames = 0
	_scene_i += 1
	if _scene_i < RUN_SCENES.size():
		_next_scene()
		return false
	print("check_project: %s (%d failures)" % ["OK" if _failures == 0 else "FAILED", _failures])
	quit(0 if _failures == 0 else 1)
	return true


func _next_scene() -> void:
	var path: String = RUN_SCENES[_scene_i]
	# S17: "#2v2" runs the match scene with the 2v2 menu choice.
	var mode := "1v1"
	if path.ends_with("#2v2"):
		path = path.trim_suffix("#2v2")
		mode = "2v2"
	var session := root.get_node_or_null(^"Session")
	if session != null:
		session.setup.mode = mode
		session.setup.map = session.map_for_mode(mode)
	if path == "res://match/match.tscn" and not ClassDB.class_exists(&"FrontlineMatch"):
		print("skip %s (bridge not built)" % path)
		return
	var packed: PackedScene = load(path)
	if packed == null:
		_fail("scene missing: %s" % path)
		return
	for c in root.get_children():
		if not c.name in ["AudioManager", "Settings", "Session"]:
			c.queue_free()
	root.add_child(packed.instantiate())
	print("running %s (%s) for %d frames" % [path, mode, FRAMES])


func _fail(msg: String) -> void:
	_failures += 1
	printerr("check_project: " + msg)


func _files(dir: String, exts: Array) -> PackedStringArray:
	var out := PackedStringArray()
	var d := DirAccess.open(dir)
	if d == null:
		return out
	d.list_dir_begin()
	var name := d.get_next()
	while name != "":
		if not name.begins_with("."):
			var full := dir.path_join(name)
			if d.current_is_dir():
				if name != "bin":
					out.append_array(_files(full, exts))
			elif name.get_extension() in exts:
				out.append(full)
		name = d.get_next()
	out.sort()
	return out
