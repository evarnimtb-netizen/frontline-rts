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
## real mods/ are never edited. S4: a copy with the `extends` variant
## `ger_grenadiergruppe_7` edited (checks clean), then broken (an unknown
## parent, a legacy key over [[members]]).

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
	# S4: an `extends` variant edited and checked again (F5), then broken
	# two ways: an unknown parent (did-you-mean) and a legacy soldier key
	# over a [[members]] parent.
	var variant_mod := ProjectSettings.globalize_path("user://s4_variant_mod")
	_copy_dir(ProjectSettings.globalize_path("res://").path_join("../mods/ww2").simplify_path(), variant_mod)
	var v_path := variant_mod.path_join("defs/units/ger_grenadiergruppe_7.toml")
	var v_text := FileAccess.get_file_as_string(v_path)
	_check(v_text.contains("extends = \"ww2:ger_grenadiergruppe\""), "S4: the field-strength card is an extends variant")
	FileAccess.open(v_path, FileAccess.WRITE).store_string(v_text.replace("cost = 230", "cost = 250"))
	m2.source.mods_dir = variant_mod
	_check(m2.source.check_mods(session.match_setup()) == "", "S4: an edited extends variant checks clean")
	FileAccess.open(v_path, FileAccess.WRITE).store_string(v_text.replace("extends = \"ww2:ger_grenadiergruppe\"", "extends = \"ww2:ger_grenadiergrupe\""))
	var v_err: String = m2.source.check_mods(session.match_setup())
	_check(v_err.contains("ger_grenadiergruppe_7") and v_err.contains("ww2:ger_grenadiergruppe"), "S4: an unknown parent names the file and suggests the parent: %s" % v_err.left(200))
	FileAccess.open(v_path, FileAccess.WRITE).store_string(v_text + "\nmen = 5\n")
	v_err = m2.source.check_mods(session.match_setup())
	_check(v_err.contains("members"), "S4: a legacy key over a [[members]] parent is refused: %s" % v_err.left(200))
	m2._reload_data()
	await _frames(10)
	_check(current_scene == m2 and m2.view.data_error.visible, "S4: the broken variant shows the banner, the match goes on")
	_remove_dir(variant_mod)
	# S5a: a cartridge's weight and a member's rounds edited (F5 checks
	# clean), then a rare_cap above the rules' rare_cap_max refused with
	# the key and the fix.
	var ammo_mod := ProjectSettings.globalize_path("user://s5a_ammo_mod")
	_copy_dir(ProjectSettings.globalize_path("res://").path_join("../mods/ww2").simplify_path(), ammo_mod)
	var c_path := ammo_mod.path_join("defs/cartridges/792x57_clip.toml")
	var c_text := FileAccess.get_file_as_string(c_path)
	_check(c_text.contains("g = 28"), "S5a: the 7.92 clip round weighs 28 g")
	FileAccess.open(c_path, FileAccess.WRITE).store_string(c_text.replace("g = 28", "g = 30"))
	var u_path := ammo_mod.path_join("defs/units/ger_grenadiergruppe.toml")
	var u_text := FileAccess.get_file_as_string(u_path)
	FileAccess.open(u_path, FileAccess.WRITE).store_string(u_text.replace("\"ww2:792x57_belt\" = 600", "\"ww2:792x57_belt\" = 500"))
	m2.source.mods_dir = ammo_mod
	_check(m2.source.check_mods(session.match_setup()) == "", "S5a: an edited cartridge weight and member rounds check clean")
	var t_path := ammo_mod.path_join("defs/vehicles/ger_pz4_h.toml")
	var t_text := FileAccess.get_file_as_string(t_path)
	FileAccess.open(t_path, FileAccess.WRITE).store_string(t_text.replace("rare_cap = 3", "rare_cap = 12"))
	var a_err: String = m2.source.check_mods(session.match_setup())
	_check(a_err.contains("rare_cap") and a_err.contains("10"), "S5a: a rare_cap above rare_cap_max is refused with the fix: %s" % a_err.left(200))
	m2._reload_data()
	await _frames(10)
	_check(current_scene == m2 and m2.view.data_error.visible, "S5a: the broken vehicle shows the banner, the match goes on")
	_remove_dir(ammo_mod)
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
