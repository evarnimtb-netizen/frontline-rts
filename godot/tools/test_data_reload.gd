extends SceneTree
## Headless test of the S3b debug data reload on the real bridge
## (FrontlineMatch): the match scene starts from mods/, "Reload data &
## restart" (match.gd _reload_data, also F5) restarts it with the same
## settings, and broken mod data shows the DataErrorBanner while the running
## match goes on. Needs a debug build of Godot (the editor binary) and the
## bridge in godot/bin.
##
##   Godot --headless --path godot -s res://tools/test_data_reload.gd
##
## The broken mod is a copy of mods/ww2 in user:// with one bad value; the
## real mods/ are never edited.

var _fails := 0
var _checks := 0


func _initialize() -> void:
	_run.call_deferred()
	create_timer(120.0).timeout.connect(func() -> void:
		printerr("test_data_reload: timed out")
		quit(2))


func _run() -> void:
	if not ClassDB.class_exists(&"FrontlineMatch"):
		print("test_data_reload: skipped (bridge not built)")
		quit(0)
		return
	var session: Node = root.get_node(^"/root/Session")
	session.setup.seed = 3
	change_scene_to_file("res://match/match.tscn")
	await _frames(30)
	var m: Node = current_scene
	_check(m != null and m.started and m.source.is_running(), "the match starts from mods/")
	_check(m.source.check_mods(session.match_setup()) == "", "the mod data checks clean")
	# Reload: the same scene again, a new bridge, the same setup.
	var first_id := m.get_instance_id()
	m._reload_data()
	await _frames(30)
	var m2: Node = current_scene
	_check(m2 != null and m2.get_instance_id() != first_id, "reload restarts the match scene")
	_check(m2.started and m2.source.is_running(), "the restarted match runs")
	_check(session.match_setup().seed == 3, "with the same settings")
	# Broken data: a copy of the mod with one value out of range.
	var broken := ProjectSettings.globalize_path("user://s3b_broken_mod")
	_copy_dir(ProjectSettings.globalize_path("res://").path_join("../mods/ww2").simplify_path(), broken)
	var rules_path := broken.path_join("defs/rules/skirmish.toml")
	var text := FileAccess.get_file_as_string(rules_path)
	FileAccess.open(rules_path, FileAccess.WRITE).store_string(text.replace("near_miss_cm = 200", "near_miss_cm = 9000"))
	m2.source.mods_dir = broken
	var err: String = m2.source.check_mods(session.match_setup())
	_check(err.contains("near_miss_cm") and err.contains("2000"), "broken data names the key and its range: %s" % err.left(160))
	m2._reload_data()
	await _frames(10)
	_check(current_scene == m2 and m2.source.is_running(), "broken data does not end the running match")
	_check(m2.view.data_error.visible and m2.view.data_error.message == err, "broken data shows the banner")
	_remove_dir(broken)
	print("test_data_reload: %d/%d checks passed" % [_checks - _fails, _checks])
	m2.source.stop_match()
	quit(1 if _fails > 0 else 0)


func _frames(n: int) -> void:
	for i in n:
		await process_frame


func _check(ok: bool, what: String) -> void:
	_checks += 1
	if ok:
		print("ok: %s" % what)
	else:
		_fails += 1
		printerr("FAIL: %s" % what)


func _copy_dir(from: String, to: String) -> void:
	DirAccess.make_dir_recursive_absolute(to)
	var d := DirAccess.open(from)
	for f in d.get_files():
		d.copy(from.path_join(f), to.path_join(f))
	for sub in d.get_directories():
		_copy_dir(from.path_join(sub), to.path_join(sub))


func _remove_dir(path: String) -> void:
	var d := DirAccess.open(path)
	if d == null:
		return
	for f in d.get_files():
		d.remove(f)
	for sub in d.get_directories():
		_remove_dir(path.path_join(sub))
	DirAccess.remove_absolute(path)
