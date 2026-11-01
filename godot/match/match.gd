extends Node3D
## Match scene root. Pulls one state Dictionary per frame from the source
## and hands it to MatchView (HUD, overlays, audio, input) and the world
## effects; forwards the view's intents back to the source.
##
## S3: both scenes get a VehicleView (vehicle MultiMesh batches) and a
## VehicleFx (muzzle flashes, tracers, sparks, fires, wreck smoke, dust,
## engine loops), made here so neither scene file needs them; both draw
## from the state's `vehicles`, `shells` and `effects`.
##
## Two sources share the interface (get_state, handle_intent, height_at):
## - match.tscn: FrontlineMatch, the Rust bridge running the real sim
##   (started here from Session.match_setup()), with WorldFx for woods,
##   flags and explosions;
## - match_mock.tscn: MockMatchSource and MockWorld (HUD work and tests).
##
## --capture_setup (see Session) stages a screenshot on the mock: "hud",
## "throw", "rotated" / "rotated_45" (minimap turning with the camera,
## yaw 30° / 45°), "north_up" (minimap locked north-up), "docked" (minimap
## bottom-left), "dig", "pause" or "end". --autoplay adds match/autoplay.gd.
##
## Net mode (S2, Session.net_mode): the bridge source gets the lobby's link
## through attach_net(Session.net) (the bridge's FrontlineNet) before
## start_match, runs at 1x, and puts `net`, `pending_orders` and the
## players' names in its own state (docs/net_api.md,
## crates/godot-bridge/README.md). With the mock net (the screenshot
## stages and the UI tests) this script merges MockNet.match_state() into
## the mock source's state and reports orders and pauses to it, so the whole
## net presenter also runs on match_mock.tscn. Net screenshot stages: "net_ok",
## "net_unstable", "net_pending", "net_desync", "net_waiting",
## "net_reconnecting", "net_paused", "net_paused_by_them", "net_disconnect".

const AUTOPLAY := preload("res://match/autoplay.gd")

@onready var view: MatchView = $View
@onready var camera: Camera3D = $Camera
@onready var source: Node = $Source
@onready var mock_world: Node3D = get_node_or_null(^"MockWorld")
@onready var world_fx: Node3D = get_node_or_null(^"WorldFx")
var autoplay: Node
var started := false
var vehicle_view: VehicleView
var vehicle_fx: VehicleFx
## Screenshot staging: [seconds before the capture, Callable] still to run.
var _capture_hooks: Array = []


func _ready() -> void:
	vehicle_view = VehicleView.new()
	vehicle_view.name = "Vehicles"
	add_child(vehicle_view)
	vehicle_fx = VehicleFx.new()
	vehicle_fx.name = "VehicleFx"
	add_child(vehicle_fx)
	var h: Callable = source.height_at
	camera.height_fn = h
	view.height_fn = h
	view.camera = camera
	view.overlay = $GroundOverlay
	# The camera moves first each frame, so the bridge culls and the HUD
	# projects with this frame's view.
	camera.process_priority = -20
	view.intent.connect(_on_intent)
	if source.has_method("start_match"):
		if Session.net_mode and source.has_method("attach_net"):
			source.attach_net(Session.net)
		if not source.start_match(Session.match_setup()):
			push_error("Match: could not start the skirmish: %s" % source.get_last_error())
			Session.goto_menu.call_deferred()
			return
		var info: Dictionary = source.get_map_info()
		camera.bounds_half_m = float(info.get("half", 512.0))
		var bases: Array = info.get("bases", [])
		var own := int(info.get("own_base", 0))
		if own < bases.size():
			var b: Vector3 = bases[own].pos
			# Look from the own base toward the map centre.
			camera.yaw_deg = rad_to_deg(atan2(b.x, b.z))
			camera.jump_to(b.lerp(Vector3.ZERO, 0.05))
		if world_fx != null:
			world_fx.setup(info, h)
		if Session.net_mode and source.has_method("set_speed"):
			source.set_speed(1.0)
	else:
		if source.has_method("set_minimap_texture"):
			source.set_minimap_texture(ImageTexture.create_from_image(MockMap.make_image(256)))
		camera.bounds_half_m = MockMap.HALF
		camera.jump_to(Vector3(-200, 0, 120))
	started = true
	AudioManager.start_ambient(&"wind")
	if Session.autoplay_enabled() and source.has_method("start_match"):
		autoplay = AUTOPLAY.new()
		autoplay.name = "Autoplay"
		add_child(autoplay)
		autoplay.begin(self)
	var setup := Session.capture_setup()
	if setup.begins_with("net_") and source is MockMatchSource:
		_stage_net(setup)
	elif setup != "" and source is MockMatchSource:
		_stage(setup)
	elif setup == "top":
		# Straight down on point C (checks terrain, woods, flags, rings).
		camera.height = 150.0
		camera.pitch_deg = -89.0
		camera.jump_to(Vector3.ZERO)


func _process(_delta: float) -> void:
	if not started:
		return
	for hk in _capture_hooks.duplicate():
		if Session.capture_left_s() <= float(hk[0]):
			_capture_hooks.erase(hk)
			hk[1].call()
	var s: Dictionary = source.get_state()
	if s.is_empty():
		return
	if _mock_net():
		_merge_mock_net(s)
	view.update_state(s)
	vehicle_view.update_state(s)
	vehicle_fx.update_state(s)
	if mock_world != null:
		mock_world.update_state(s)
	if world_fx != null:
		world_fx.update_state(s)
	if autoplay != null:
		autoplay.on_state(s)


func _on_intent(kind: StringName, data: Dictionary) -> void:
	if kind == &"quit_to_menu":
		Session.goto_menu()
		return
	if _mock_net():
		_mock_net_intent(kind, data)
	source.handle_intent(kind, data)


# --- mock net glue (match_mock.tscn with MockNet) -----------------------------

func _mock_net() -> bool:
	return Session.net_mode and Session.is_mock_net()


## What the bridge will put in its own state in net mode: `net`,
## `pending_orders` and the players' names.
func _merge_mock_net(s: Dictionary) -> void:
	var extra: Dictionary = Session.net.match_state()
	s.net = extra.net
	s.pending_orders = extra.pending_orders
	var names: Array = extra.get("player_names", [])
	var players: Array = s.get("players", [])
	for i in mini(names.size(), players.size()):
		if String(names[i]) != "":
			players[i].name = names[i]
			players[i].is_ai = false


func _mock_net_intent(kind: StringName, data: Dictionary) -> void:
	var mock: MockNet = Session.net
	if kind == &"pause":
		mock.mock_pause(data.paused)
		return
	if not MatchView.PENDING_KINDS.has(kind):
		return
	var pos: Variant = data.get("target")
	if pos == null and data.has("polyline") and not data.polyline.is_empty():
		pos = data.polyline[data.polyline.size() / 2]
	if pos is Vector3:
		mock.mock_order(MatchView.PENDING_KINDS[kind], pos)


func _own_ids(type := "") -> PackedInt32Array:
	return source.squad_ids(source.local_player, type)


func _line(pts: Array) -> PackedVector3Array:
	var out := PackedVector3Array()
	for p in pts:
		out.append(Vector3(p.x, source.height_at(p.x, p.y), p.y))
	return out


## Screenshot staging (Session --capture_setup).
func _stage(what: String) -> void:
	match what:
		"hud", "pause", "throw", "rotated", "rotated_45", "north_up", "docked":
			# A firefight in the open south-west of D: four of ours facing
			# three of theirs about 110 m away.
			source.prewarm(60.0)
			var ids := _own_ids()
			var foes: PackedInt32Array = source.squad_ids(1 - source.local_player, "rifle")
			foes.append_array(source.squad_ids(1 - source.local_player, "mg"))
			for i in mini(ids.size(), 4):
				source.place_squad(ids[i], Vector2(120 + i * 32, 300 - i * 14), Vector2(1, -1))
			for i in mini(foes.size(), 3):
				source.place_squad(foes[i], Vector2(205 + i * 30, 205 - i * 12), Vector2(-1, 1))
			source.prewarm(12.0)
			_stage_soldiers(ids)
			camera.height = 76.0
			camera.jump_to(Vector3(176, 0, 276))
			view.update_state(source.get_state())
			source.handle_intent(&"call_in", {"type": "rifle"})
			source.handle_intent(&"call_in", {"type": "mg"})
			if what == "throw":
				# Frag tool armed for the first rifle squad, cursor on the
				# nearest enemy man (in range or not, the screenshot shows it).
				view.selection.select_squads(ids.slice(0, 1))
				view.selection.activate(&"frag")
				var c := _centroid(ids.slice(0, 1))
				view.selection.cursor_override = camera.unproject_position(c + Vector3(16, 0, -16))
			if what == "pause":
				view.pause_menu.open()
			elif what == "rotated" or what == "rotated_45":
				# Minimap turning with the camera (not saved as a setting).
				view.minimap.rotate_with_camera = true
				camera.yaw_deg = 45.0 if what == "rotated_45" else 30.0
				camera.jump_to(Vector3(176, 0, 276))
			elif what == "north_up":
				# Minimap locked north-up (not saved as a setting).
				view.minimap.rotate_with_camera = false
				camera.yaw_deg = 30.0
				camera.jump_to(Vector3(176, 0, 276))
			elif what == "docked":
				view.minimap.dock = "bottom_left"
		"dig":
			var eng := _own_ids("engineer")
			var rifles := _own_ids("rifle")
			var e: Vector3 = _centroid(eng)
			# One rifle squad already digging north of the engineers, and the
			# engineers' trench being laid out with the dig tool.
			source.handle_intent(&"dig", {"squads": rifles.slice(0, 1), "polyline": _line([Vector2(e.x - 10, e.z - 46), Vector2(e.x + 8, e.z - 52), Vector2(e.x + 24, e.z - 50)])})
			source.prewarm(30.0)
			camera.height = 46.0
			camera.pitch_deg = -55.0
			camera.yaw_deg = 0.0
			camera.jump_to(e + Vector3(4, 0, -22))
			view.update_state(source.get_state())
			view.selection.select_squads(eng)
			view.selection.activate(&"dig")
			view.selection.dig_points.append_array(_line([Vector2(e.x - 16, e.z - 20), Vector2(e.x - 2, e.z - 27)]))
			var cur: Vector3 = _line([Vector2(e.x + 14, e.z - 24)])[0]
			view.selection.cursor_override = camera.unproject_position(cur)
		"end":
			source.prewarm(150.0)
			source.force_end(0, "tickets")
		"s3_vehicles", "s3_card", "s3_fit", "s3_locked", "s3_escort":
			_stage_s3(what)


## S3 screenshot staging on the mock: the "hud" firefight plus four own
## tanks in different states (moving with an escort, bogged, burning,
## repairing), two seen enemy tanks and two wrecks. "s3_card": the first
## tank selected (its card and detail, ammo choice HE). "s3_fit": 4 tanks
## and 8 squads for the 1280x800 roster. "s3_locked": no tanks yet, the
## call-in tile locked with its countdown. "s3_escort": the escort file
## seen close up.
func _stage_s3(what: String) -> void:
	_stage("hud")
	if what == "s3_locked":
		view.selection.select_only(PackedInt32Array())
		return
	var ids := _own_ids()
	if what == "s3_fit":
		source._spawn(source.local_player, "at", Vector2(70, 330))
		source._spawn(source.local_player, "rifle", Vector2(40, 350))
	elif what == "s3_vehicles":
		# An AT team within launcher range of the nearer enemy tank.
		source._spawn(source.local_player, "at", Vector2(176, 268))
		# Time shots into the picture: own tanks fire 0.3 s before it (the
		# hits land: sparks, markers), the enemy and the AT team just before
		# (muzzle flashes, tracers, the rocket trail).
		_capture_hooks = [
			[0.3, func() -> void: source.veh.volley(source.local_player)],
			[0.05, func() -> void:
				source.veh.volley(1 - source.local_player)
				source.at_volley()],
		]
	var tanks: PackedInt32Array = source.stage_vehicles(Vector2(150, 318), Vector2(1, -1), ids[0] if not ids.is_empty() else -1)
	source.prewarm(4.0)
	var s: Dictionary = source.get_state()
	view.update_state(s)
	match what:
		"s3_card", "s3_fit":
			view.selection.select_only(tanks.slice(0, 1))
			view.selection.focus_squad = int(view.vehicles_by_id[tanks[0]].squad)
			view.selection.groups[4] = tanks.duplicate()
		"s3_escort":
			view.selection.select_squads(ids.slice(0, 1))
			var v: Vector3 = source.squad_pos(ids[0])
			camera.height = 38.0
			camera.jump_to(v)
			return
		_:
			view.selection.select_only(tanks.slice(0, 2))
	# Look at the tanks from behind them (south-west), the enemy beyond:
	# the screen centre a little ahead of their centroid.
	var c := Vector3.ZERO
	for id in tanks:
		c += view.unit_pos(id)
	c /= maxi(tanks.size(), 1)
	camera.height = 55.0
	camera.yaw_deg = -45.0
	camera.jump_to(c + Vector3(1, 0, -1).normalized() * 6.0)


## Net screenshot staging (S2) on a MockNet: the "hud" firefight, then the
## net state; "net_pending" adds a move order still in flight.
func _stage_net(what: String) -> void:
	var mock := MockNet.new()
	mock.auto = false
	mock.set_name("Erik")
	Session.net = mock
	Session.net_mode = true
	_stage("hud")
	mock.stage("net_ok" if what == "net_pending" else what)
	if what == "net_pending":
		var ids := _own_ids()
		view.selection.select_squads(ids.slice(0, 1))
		var s0: Dictionary = source.get_state()
		_merge_mock_net(s0)
		view.update_state(s0)
		var c := _centroid(ids.slice(0, 1))
		var g := c + Vector3(-18, 0, -26)
		g.y = source.height_at(g.x, g.z)
		view.selection.order_to_ground(g)
		# Keep the order in flight for the screenshot.
		for p in mock.pending:
			p.exec_at = INF
	var s: Dictionary = source.get_state()
	_merge_mock_net(s)
	view.update_state(s)


## HUD staging for S1b: wounded and pinned men, one man detached and
## sent ahead, two men of the first squad selected and stanced, a group.
func _stage_soldiers(ids: PackedInt32Array) -> void:
	if ids.size() < 3:
		return
	var r1: PackedInt32Array = source.unit_ids(ids[0])
	var r2: PackedInt32Array = source.unit_ids(ids[1])
	var mg: PackedInt32Array = source.unit_ids(ids[2])
	if r1.size() >= 6:
		source.set_soldier(r1[5], 42.0)
	if r2.size() >= 5:
		source.set_soldier(r2[2], 35.0)
		source.set_soldier(r2[3], 80.0, 0.95)
		var ahead: Vector3 = source.soldier_pos(r2[4]) + Vector3(22, 0, -18)
		ahead.y = source.height_at(ahead.x, ahead.z)
		source.handle_intent(&"move_units", {"units": PackedInt32Array([r2[4]]), "target": ahead})
	for i in mini(mg.size(), 2):
		source.set_soldier(mg[i], 90.0, 0.95)
	source.prewarm(2.0)
	view.update_state(source.get_state())
	view.selection.select_squads(ids.slice(0, 3))
	view.selection.groups[1] = view.selection.selected.duplicate()
	view.selection.select_squads(ids.slice(0, 1))
	view.selection.groups[2] = view.selection.selected.duplicate()
	view.selection.set_stance(&"crouch")
	view.selection.groups[3] = source.unit_ids(ids[3])
	if r1.size() >= 6:
		view.selection.select_only(PackedInt32Array([r1[2], r1[5]]))


func _centroid(ids: PackedInt32Array) -> Vector3:
	var c := Vector3.ZERO
	for id in ids:
		c += source.squad_pos(id)
	return c / maxi(ids.size(), 1)
