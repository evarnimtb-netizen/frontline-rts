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
##
## Multiplayer (S2): `net` is the lobby/link object, the bridge's
## FrontlineNet when it is registered, else MockNet (godot/net/mock_net.gd,
## no networking; the menu still works against it). A net match runs at 1x
## only: net mode forces the speed and ignores --autoplay_speed.
##   -- --autoplay_net host:PORT | join:IP:PORT   (IPv6: join:[::1]:PORT)
##      --autoplay N  [--net_name NAME] [--debug_inject_desync TICK]
##      [--net_delay D] [--net_bind_all]
## hosts or joins, picks a faction (host Germany, guest USA), readies, and
## plays the match with match/autoplay.gd like --autoplay. Two instances on
## one machine: one host, one join 127.0.0.1. The autoplay host's relay
## binds 127.0.0.1 only (FrontlineNet.host_loopback: no macOS firewall
## prompt); add --net_bind_all to bind every interface like the menu's Host
## does. --debug_inject_desync makes this client step TICK with one extra
## local-only Stop (the bridge reads it from match_setup()).
##
## S3 staged scenes (windowed; S3 "Done when" 7):
##   Godot --path godot --resolution 1600x900 -- \
##       --autoplay_scene armour_<name> [--autoplay_shots /abs/dir]
## starts the match scene straight on a staged world of the sim's
## `scenarios` (<name>: callin_locked, vehicle_card, tank_firing,
## penetration_marker, bogged_in_trench, at_ditch, at_team_firing,
## burning, escort; S3c bailout), lets match/autoplay_scene.gd wait for its condition,
## saves s3_<name>.png and quits (1 if the condition never held).
## --test_unlock plays the test rules override (vehicles unlocked at once)
## in a skirmish or, set on both instances, a net match; --start_manpower N
## likewise sets both players' starting manpower.

const MENU_SCENE := "res://ui/menu/main_menu.tscn"
const MATCH_SCENE := "res://match/match.tscn"
const MOCK_MATCH_SCENE := "res://match/match_mock.tscn"
## Wall-clock seconds --autoplay_net waits for the lobby to start.
const AUTOPLAY_NET_WAIT_S := 90.0

const FACTIONS := ["germany", "usa"]
const DIFFICULTIES := ["easy", "normal", "hard"]
const MAPS := {
	"kessel_ridge": {
		"name": "Kessel Ridge",
		"size_m": 1024,
		"points": 5,
		"blurb": "Rolling farmland split by a wooded ridge. Five points, the middle one on the crest.",
		"bases": 2,
	},
	# S17: the four-base team map (2v2 only: player i takes base i).
	"harrow_cross": {
		"name": "Harrow Cross",
		"size_m": 1536,
		"points": 7,
		"blurb": "Two roads cross on a low rise. Two bases on each edge; you and an AI ally hold the west.",
		"bases": 4,
	},
	"linden_brook": {
		"name": "Linden Brook",
		"size_m": 1024,
		"points": 5,
		"blurb": "A brook valley between two low ridges, bases west and east. The middle point lies in the valley, and country roads run between the points: wheels and tracks are faster on them.",
		"bases": 2,
	},
}
## S17: the skirmish modes (1v1, 2v2: you and an AI ally against two AIs).
const MODES := ["1v1", "2v2"]

var setup := {
	"faction": "germany",
	"difficulty": "normal",
	"map": "kessel_ridge",
	"seed": 1,
	"mode": "1v1",
}
## Filled by the match when it ends, for anything shown after it.
var last_result := {}

## Autoplay: game minutes to play (0 = off), sim speed, screenshot folder.
var autoplay_minutes := 0.0
var autoplay_speed := 3.0
var autoplay_shots := ""
## Match overrides for scripted runs (0 = rules default).
var tickets_override := 0
## --start_manpower N (0 = rules default); in a net match both instances
## must pass the same N.
var start_manpower_override := 0

## Multiplayer: the FrontlineNet (or MockNet) object, null until first
## used; net_mode while a net match is being played.
var net: Object = null
var net_mode := false
var player_name := "Player"
## Join field text from the last join (restored in the menu).
var last_join := ""
## Which menu page to show when the menu loads ("" = skirmish).
var menu_page := ""
## --autoplay_net: "host" or "join", and the port or address.
var autoplay_net_role := ""
var autoplay_net_target := ""
## --debug_inject_desync tick (-1 = off).
var debug_inject_desync := -1
## --net_bind_all: the autoplay host binds every interface, not loopback.
var net_bind_all := false
## --net_delay D: the autoplay host sets input delay D (2-6) in the lobby
## instead of keeping the suggestion (0 = keep).
var net_delay := 0
## --autoplay_scene armour_<name>: the staged scene's <name> ("" = off).
var autoplay_scene := ""
## --test_unlock: the S3 test rules override (vehicles unlocked at once).
var test_unlock := false
## Why the last match could not start (the menu shows it once; S3b).
var start_error := ""

var _capture_path := ""
var _capture_after := 2.5
var _capture_timer: SceneTreeTimer
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
			"--map":
				if v in MAPS:
					setup.map = v
				i += 1
			"--seed":
				setup.seed = v.to_int()
				i += 1
			"--mode":
				# S17: 2v2 plays a four-base map (Harrow Cross).
				if v in MODES:
					setup.mode = v
					if not setup.map in maps_for_mode(v):
						setup.map = map_for_mode(v)
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
			"--start_manpower":
				start_manpower_override = maxi(v.to_int(), 0)
				i += 1
			"--autoplay_net":
				_parse_autoplay_net(v)
				i += 1
			"--net_name":
				if not v.strip_edges().is_empty():
					player_name = v.strip_edges().left(24)
				i += 1
			"--net_delay":
				net_delay = clampi(v.to_int(), 2, 6) if v.is_valid_int() else 0
				i += 1
			"--net_bind_all":
				net_bind_all = true
			"--autoplay_scene":
				autoplay_scene = v.trim_prefix("armour_")
				i += 1
			"--test_unlock":
				test_unlock = true
			"--debug_inject_desync":
				# Dev builds only: a release export ignores it.
				if OS.is_debug_build():
					debug_inject_desync = v.to_int() if v.is_valid_int() and v.to_int() > 0 else -1
				else:
					push_warning("--debug_inject_desync is ignored in a release build")
				i += 1
		i += 1
	if autoplay_net_role != "":
		# Net mode is 1x only (S2 "Speed").
		autoplay_speed = 1.0
		if autoplay_minutes <= 0.0:
			autoplay_minutes = 3.0
	if _capture_path != "":
		_run_capture.call_deferred()
	elif autoplay_scene != "":
		_run_autoplay_scene.call_deferred()
	elif autoplay_net_role != "":
		_run_autoplay_net.call_deferred()
	elif autoplay_enabled():
		_run_autoplay_menu.call_deferred()


## "host:PORT" or "join:ADDR:PORT"; anything else is reported and ignored.
func _parse_autoplay_net(v: String) -> void:
	if v.begins_with("host:"):
		var p := NetUi.parse_port(v.substr(5), true)
		if p.ok:
			autoplay_net_role = "host"
			autoplay_net_target = str(p.port)
			return
		push_error("Session: --autoplay_net %s: %s" % [v, p.error])
	elif v.begins_with("join:"):
		var a := NetUi.parse_join(v.substr(5))
		if a.ok:
			autoplay_net_role = "join"
			autoplay_net_target = a.addr
			return
		push_error("Session: --autoplay_net %s: %s" % [v, a.error])
	else:
		push_error("Session: --autoplay_net wants host:PORT or join:ADDR:PORT, got \"%s\"" % v)


func capture_setup() -> String:
	return _capture_setup


## Seconds until the --capture screenshot is taken (INF when no capture is
## running). Staging uses it to time short effects into the shot (S3).
func capture_left_s() -> float:
	return _capture_timer.time_left if _capture_timer != null else INF


func autoplay_enabled() -> bool:
	return autoplay_minutes > 0.0 and autoplay_scene == ""


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
	if start_manpower_override > 0:
		out.start_manpower = start_manpower_override
	if autoplay_scene != "":
		out.scene = autoplay_scene
	if test_unlock:
		out.test_unlock = true
	# Net mode: the bridge takes the link itself from Session.net; these
	# tell it the local slot, D and the debug hook.
	if net_mode and net != null:
		var st: Dictionary = net.status()
		out.net = true
		out.local_player = int(st.get("slot", 0))
		out.input_delay = int(st.get("input_delay", 3))
		out.speed = 1.0
		if debug_inject_desync > 0:
			out.debug_inject_desync = debug_inject_desync
	return out


## The net object, made on first use: FrontlineNet when the bridge
## registers it, else MockNet.
func ensure_net() -> Object:
	if net == null:
		if ClassDB.class_exists(&"FrontlineNet"):
			net = ClassDB.instantiate(&"FrontlineNet")
			if net is Node:
				add_child(net)
			if test_unlock and net.has_method("set_test_unlock"):
				net.set_test_unlock(true)
			if start_manpower_override > 0 and net.has_method("set_start_manpower"):
				net.set_start_manpower(start_manpower_override)
		else:
			net = MockNet.new()
	return net


func is_mock_net() -> bool:
	return net is MockNet


## Lobby → match, once the status says in_match: takes the own slot's
## faction, the seed and the map from the lobby and loads the match scene
## (the mock scene when the net is the mock: no sim behind it).
## Once per match: the lobby page and --autoplay_net both see in_match.
func start_net_match() -> void:
	if net == null or net_mode:
		return
	var st: Dictionary = net.status()
	var slot := int(st.get("slot", 0))
	var slots: Array = st.get("slots", [])
	var me: Dictionary = slots[slot] if slot >= 0 and slot < slots.size() else {}
	setup.faction = me.get("faction", setup.faction)
	setup.seed = int(st.get("seed", setup.seed))
	setup.map = st.get("map", setup.map)
	net_mode = true
	autoplay_speed = 1.0
	get_tree().paused = false
	get_tree().change_scene_to_file(MOCK_MATCH_SCENE if is_mock_net() else MATCH_SCENE)


## Bye and back to idle; after a net match the menu opens on the
## multiplayer page.
func leave_net() -> void:
	if net != null:
		net.leave()
	if net_mode:
		menu_page = "multiplayer"
	net_mode = false


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


## Debug data reload (S3b): the match scene again with the same setup; its
## start reads mods/ from disk afresh.
func restart_match() -> void:
	get_tree().paused = false
	get_tree().reload_current_scene()


func goto_menu() -> void:
	if net_mode:
		leave_net()
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


## S17: the bases a mode needs (one per player).
func bases_for_mode(mode: String) -> int:
	return 4 if mode == "2v2" else 2


## S17: the maps that play `mode` (as many bases as players), in MAPS order.
func maps_for_mode(mode: String) -> Array:
	var out: Array = []
	for id in MAPS:
		if MAPS[id].get("bases", 2) == bases_for_mode(mode):
			out.append(id)
	return out


## S17: the first map that plays `mode`.
func map_for_mode(mode: String) -> String:
	var ids := maps_for_mode(mode)
	return ids[0] if not ids.is_empty() else "kessel_ridge"


func _run_autoplay_menu() -> void:
	# The menu is the main scene: let it settle, record it, start the match.
	await get_tree().create_timer(1.5, true, false, true).timeout
	await save_screenshot(autoplay_dir().path_join("s1_autoplay_menu.png"))
	print("autoplay: starting skirmish %s" % [match_setup()])
	start_skirmish({})


## --autoplay_scene: straight into the match scene (no menu); match.gd
## adds match/autoplay_scene.gd.
func _run_autoplay_scene() -> void:
	await get_tree().create_timer(0.3, true, false, true).timeout
	print("autoplay_scene: starting %s" % autoplay_scene)
	start_skirmish({})


func _run_capture() -> void:
	if _capture_size != Vector2i.ZERO and DisplayServer.get_name() != "headless":
		var win := get_window()
		win.mode = Window.MODE_WINDOWED
		win.size = _capture_size
	_capture_timer = get_tree().create_timer(_capture_after, true, false, true)
	await _capture_timer.timeout
	await RenderingServer.frame_post_draw
	var img := get_viewport().get_texture().get_image()
	var err := img.save_png(_capture_path)
	if err != OK:
		push_error("Session: capture failed (%s): %s" % [error_string(err), _capture_path])
	else:
		print("Session: captured %s (%dx%d)" % [_capture_path, img.get_width(), img.get_height()])
	quit_game(0 if err == OK else 1)


## --autoplay_net: host or join, pick a faction, ready, then the match
## (match.gd adds autoplay.gd as for --autoplay). Quits with 4 if the
## lobby does not start within AUTOPLAY_NET_WAIT_S.
func _run_autoplay_net() -> void:
	await get_tree().create_timer(1.0, true, false, true).timeout
	var n := ensure_net()
	if n.has_method("set_name"):
		n.set_name(player_name if player_name != "Player" else autoplay_net_role.capitalize())
	var ok: bool
	if autoplay_net_role == "join":
		ok = n.join(autoplay_net_target)
	elif not net_bind_all and n.has_method("host_loopback"):
		ok = n.host_loopback(autoplay_net_target.to_int())
	else:
		ok = n.host(autoplay_net_target.to_int())
	print("autoplay_net: %s %s -> %s" % [autoplay_net_role, autoplay_net_target, "ok" if ok else "failed"])
	if not ok:
		push_error("autoplay_net: %s" % n.status().get("error", "?"))
		quit_game(4)
		return
	# The menu shows the lobby, so the screenshot below has it.
	var menu := get_tree().current_scene
	if menu != null and menu.has_method("show_page"):
		menu.show_page("multiplayer")
	var want := "germany" if autoplay_net_role == "host" else "usa"
	var readied := false
	var shot := false
	var waited := 0.0
	while waited < AUTOPLAY_NET_WAIT_S:
		var st: Dictionary = n.status()
		match String(st.get("state", "")):
			"lobby":
				var slots: Array = st.get("slots", [])
				var slot := int(st.get("slot", -1))
				var full := slots.size() == 2 and not String(slots[0].get("name", "")).is_empty() and not String(slots[1].get("name", "")).is_empty()
				if slot < 0 or slot >= slots.size():
					pass
				elif slots[slot].get("faction", "") != want:
					n.set_choice({"faction": want})
				elif slot == 0 and int(st.get("seed", 0)) != int(setup.seed):
					n.set_choice({"seed": int(setup.seed)})
				elif slot == 0 and net_delay > 0 and int(st.get("input_delay", 0)) != net_delay:
					n.set_choice({"input_delay": net_delay})
				elif full and not readied:
					if not shot:
						shot = true
						await save_screenshot(autoplay_dir().path_join("s2_autoplay_lobby_%s.png" % autoplay_net_role))
					n.ready()
					readied = true
				elif full and readied and not slots[slot].get("ready", false):
					# A lobby change took us out of Ready: ready again.
					readied = false
			"in_match":
				if is_mock_net():
					# No FrontlineNet in this build: the lobby ran on the
					# mock, and the mock match has no autoplay. Say so and stop.
					push_error("autoplay_net: FrontlineNet is not registered (bridge without S2 networking); the lobby ran on MockNet only")
					quit_game(5)
					return
				print("autoplay_net: lobby done, match as slot %d%s" % [int(st.get("slot", 0)), " (the lobby page started it)" if net_mode else ""])
				start_net_match()
				return
			"disconnected":
				push_error("autoplay_net: disconnected: %s" % st.get("error", "?"))
				quit_game(4)
				return
		await get_tree().create_timer(0.1, true, false, true).timeout
		waited += 0.1
	push_error("autoplay_net: the lobby did not start within %d s" % int(AUTOPLAY_NET_WAIT_S))
	quit_game(4)
