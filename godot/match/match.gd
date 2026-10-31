extends Node3D
## Match scene root. Pulls one state Dictionary per frame from the source
## and hands it to MatchView (HUD, overlays, audio, input) and the world
## effects; forwards the view's intents back to the source.
##
## Two sources share the interface (get_state, handle_intent, height_at):
## - match.tscn: FrontlineMatch, the Rust bridge running the real sim
##   (started here from Session.match_setup()), with WorldFx for woods,
##   flags and explosions;
## - match_mock.tscn: MockMatchSource and MockWorld (HUD work and tests).
##
## --capture_setup (see Session) stages a screenshot on the mock: "hud",
## "dig", "pause" or "end". --autoplay adds match/autoplay.gd.

const AUTOPLAY := preload("res://match/autoplay.gd")

@onready var view: MatchView = $View
@onready var camera: Camera3D = $Camera
@onready var source: Node = $Source
@onready var mock_world: Node3D = get_node_or_null(^"MockWorld")
@onready var world_fx: Node3D = get_node_or_null(^"WorldFx")
var autoplay: Node
var started := false


func _ready() -> void:
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
	if setup != "" and source is MockMatchSource:
		_stage(setup)
	elif setup == "top":
		# Straight down on point C (checks terrain, woods, flags, rings).
		camera.height = 150.0
		camera.pitch_deg = -89.0
		camera.jump_to(Vector3.ZERO)


func _process(_delta: float) -> void:
	if not started:
		return
	var s: Dictionary = source.get_state()
	if s.is_empty():
		return
	view.update_state(s)
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
	source.handle_intent(kind, data)


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
		"hud", "pause":
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
			camera.height = 85.0
			camera.jump_to(Vector3(175, 0, 268))
			view.update_state(source.get_state())
			view.selection.select_only(ids.slice(0, 4))
			source.handle_intent(&"call_in", {"type": "rifle"})
			source.handle_intent(&"call_in", {"type": "mg"})
			if what == "pause":
				view.pause_menu.open()
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
			view.selection.select_only(eng)
			view.selection.activate(&"dig")
			view.selection.dig_points.append_array(_line([Vector2(e.x - 16, e.z - 20), Vector2(e.x - 2, e.z - 27)]))
			var cur: Vector3 = _line([Vector2(e.x + 14, e.z - 24)])[0]
			view.selection.cursor_override = camera.unproject_position(cur)
		"end":
			source.prewarm(150.0)
			source.force_end(0, "tickets")


func _centroid(ids: PackedInt32Array) -> Vector3:
	var c := Vector3.ZERO
	for id in ids:
		c += source.squad_pos(id)
	return c / maxi(ids.size(), 1)
