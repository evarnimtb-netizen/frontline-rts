extends Node
## AudioManager (autoload). Owns the bus layout (Master > SFX / UI / Ambient),
## a pool of AudioStreamPlayer3D for positional effects, a small 2D pool for
## UI sounds and one ambient loop player (SYSTEM_DESIGN.md §12, S1 audio).
##
## The pool is fixed size: when every voice is busy, a new sound steals the
## voice that started earliest. Each sound also has a concurrency cap so a
## firefight cannot fill the pool with one weapon, and sounds farther than
## their `max_distance` from the listener are skipped before a voice is used.
##
## Sounds are named (see SOUNDS); every file under audio/sfx has a
## `<file>.license.toml` manifest (CC0, §12.1).
##
##   AudioManager.play_3d(&"rifle", world_pos)
##   AudioManager.play_ui(&"click")
##   AudioManager.start_ambient(&"wind")
##   AudioManager.set_engine_loops(points, speeds)   # S3, every frame
##
## S3 engines: a pool of ENGINE_VOICES looping AudioStreamPlayer3Ds (never
## one per vehicle); each frame the presenter passes the nearest moving
## vehicles and the voices follow them, pitched by speed.
##
## Buttons anywhere in the tree click automatically (node_added hook); set
## the meta "no_click_sound" on a button to opt out.

const BUSES := [&"SFX", &"UI", &"Ambient"]
const POOL_3D := 32
const POOL_UI := 4

## name: [path, bus, max concurrent voices, unit_size (m), max_distance (m), volume dB]
const SOUNDS := {
	&"rifle": ["res://audio/sfx/rifle_shot.ogg", &"SFX", 10, 18.0, 700.0, -4.0],
	&"mg": ["res://audio/sfx/mg_shot.ogg", &"SFX", 8, 20.0, 800.0, -5.0],
	&"smg": ["res://audio/sfx/mg_shot.ogg", &"SFX", 6, 12.0, 450.0, -9.0],
	&"mortar_launch": ["res://audio/sfx/mortar_launch.ogg", &"SFX", 4, 25.0, 900.0, -2.0],
	&"explosion": ["res://audio/sfx/explosion.ogg", &"SFX", 6, 35.0, 1200.0, 0.0],
	&"dig": ["res://audio/sfx/dig.ogg", &"SFX", 4, 6.0, 120.0, -6.0],
	&"click": ["res://audio/sfx/ui_click.ogg", &"UI", 2, 0.0, 0.0, -6.0],
	&"confirm": ["res://audio/sfx/ui_confirm.ogg", &"UI", 2, 0.0, 0.0, -8.0],
	&"error": ["res://audio/sfx/ui_error.ogg", &"UI", 2, 0.0, 0.0, -8.0],
	&"wind": ["res://audio/sfx/wind_loop.ogg", &"Ambient", 1, 0.0, 0.0, -10.0],
	&"cannon": ["res://audio/sfx/cannon.ogg", &"SFX", 4, 40.0, 1500.0, 0.0],
	&"ap_impact": ["res://audio/sfx/ap_impact.ogg", &"SFX", 4, 20.0, 800.0, -3.0],
	&"rocket_launch": ["res://audio/sfx/rocket_launch.ogg", &"SFX", 3, 15.0, 600.0, -3.0],
	&"engine": ["res://audio/sfx/engine_loop.ogg", &"SFX", 4, 12.0, 300.0, -6.0],
}
## Looping engine voices (S3: the 4 nearest moving vehicles).
const ENGINE_VOICES := 4

var _streams := {}
var _pool: Array[AudioStreamPlayer3D] = []
var _pool_sound: Array[StringName] = []
var _pool_started: PackedInt64Array = PackedInt64Array()
var _ui: Array[AudioStreamPlayer] = []
var _ui_next := 0
var _ambient: AudioStreamPlayer
var _ambient_name: StringName = &""
var _engines: Array[AudioStreamPlayer3D] = []
## Headless runs use the dummy audio driver, which never mixes, so started
## playbacks are never released (leaks at exit). Nothing plays there.
var _silent := false


func _enter_tree() -> void:
	# Buses must exist before Settings applies volumes in its _ready.
	ensure_buses()


func _ready() -> void:
	process_mode = Node.PROCESS_MODE_ALWAYS
	_silent = DisplayServer.get_name() == "headless"
	for i in POOL_3D:
		var p := AudioStreamPlayer3D.new()
		p.name = "Voice3D_%d" % i
		p.attenuation_model = AudioStreamPlayer3D.ATTENUATION_INVERSE_DISTANCE
		p.bus = &"SFX"
		p.doppler_tracking = AudioStreamPlayer3D.DOPPLER_TRACKING_DISABLED
		add_child(p)
		_pool.append(p)
		_pool_sound.append(&"")
		_pool_started.append(0)
	for i in POOL_UI:
		var u := AudioStreamPlayer.new()
		u.name = "VoiceUI_%d" % i
		u.bus = &"UI"
		add_child(u)
		_ui.append(u)
	_ambient = AudioStreamPlayer.new()
	_ambient.name = "Ambient"
	_ambient.bus = &"Ambient"
	add_child(_ambient)
	for i in ENGINE_VOICES:
		var e := AudioStreamPlayer3D.new()
		e.name = "Engine_%d" % i
		e.bus = &"SFX"
		e.attenuation_model = AudioStreamPlayer3D.ATTENUATION_INVERSE_DISTANCE
		e.doppler_tracking = AudioStreamPlayer3D.DOPPLER_TRACKING_DISABLED
		add_child(e)
		_engines.append(e)
	get_tree().node_added.connect(_on_node_added)


## Creates the S1 buses under Master if the project has no bus layout.
static func ensure_buses() -> void:
	for bus in BUSES:
		if AudioServer.get_bus_index(bus) == -1:
			AudioServer.add_bus()
			var idx := AudioServer.bus_count - 1
			AudioServer.set_bus_name(idx, bus)
			AudioServer.set_bus_send(idx, &"Master")


## Sets a bus volume from a linear 0..1 slider value.
func set_bus_linear(bus: StringName, linear: float) -> void:
	var idx := AudioServer.get_bus_index(bus)
	if idx == -1:
		return
	var v := clampf(linear, 0.0, 1.0)
	AudioServer.set_bus_mute(idx, v <= 0.001)
	AudioServer.set_bus_volume_db(idx, linear_to_db(maxf(v, 0.001)))


func _stream(sound: StringName) -> AudioStream:
	if _streams.has(sound):
		return _streams[sound]
	var def: Array = SOUNDS.get(sound, [])
	var s: AudioStream = null
	if not def.is_empty() and ResourceLoader.exists(def[0]):
		s = load(def[0])
	if s == null:
		push_warning("AudioManager: unknown or missing sound %s" % sound)
	_streams[sound] = s
	return s


func _listener_pos() -> Vector3:
	var cam := get_viewport().get_camera_3d()
	return cam.global_position if cam != null else Vector3.ZERO


## Plays a positional sound at `pos` (Godot world space). Returns false
## when it was culled (too far, capped or unknown).
func play_3d(sound: StringName, pos: Vector3, volume_db := 0.0, pitch_jitter := 0.06) -> bool:
	var def: Array = SOUNDS.get(sound, [])
	if def.is_empty() or _silent:
		return false
	var max_dist: float = def[4]
	if max_dist > 0.0 and pos.distance_to(_listener_pos()) > max_dist:
		return false
	var stream := _stream(sound)
	if stream == null:
		return false
	var now := Time.get_ticks_usec()
	var same := 0
	var free_i := -1
	var oldest_i := 0
	var oldest_same_i := -1
	for i in _pool.size():
		var p := _pool[i]
		if not p.playing:
			if free_i == -1:
				free_i = i
			continue
		if _pool_started[i] < _pool_started[oldest_i] or not _pool[oldest_i].playing:
			oldest_i = i
		if _pool_sound[i] == sound:
			same += 1
			if oldest_same_i == -1 or _pool_started[i] < _pool_started[oldest_same_i]:
				oldest_same_i = i
	var slot := free_i
	if same >= int(def[2]):
		slot = oldest_same_i  # capped: restart this sound's oldest voice
	elif slot == -1:
		slot = oldest_i  # pool full: steal the oldest voice
	var v := _pool[slot]
	v.stop()
	v.stream = stream
	v.bus = def[1]
	v.unit_size = def[3]
	v.max_distance = max_dist
	v.volume_db = float(def[5]) + volume_db
	v.pitch_scale = 1.0 + randf_range(-pitch_jitter, pitch_jitter)
	v.global_position = pos
	v.play()
	_pool_sound[slot] = sound
	_pool_started[slot] = now
	return true


## Plays a non-positional UI sound.
func play_ui(sound: StringName, volume_db := 0.0) -> void:
	if _silent:
		return
	var stream := _stream(sound)
	if stream == null:
		return
	var p := _ui[_ui_next]
	_ui_next = (_ui_next + 1) % _ui.size()
	p.stream = stream
	p.volume_db = float(SOUNDS[sound][5]) + volume_db
	p.play()


## Starts (or keeps) a looping ambient bed on the Ambient bus.
func start_ambient(sound: StringName) -> void:
	if _silent or (_ambient_name == sound and _ambient.playing):
		return
	var stream := _stream(sound)
	if stream == null:
		return
	if stream is AudioStreamOggVorbis:
		(stream as AudioStreamOggVorbis).loop = true
	_ambient.stream = stream
	_ambient.volume_db = float(SOUNDS[sound][5])
	_ambient.play()
	_ambient_name = sound


## Engine loops for up to ENGINE_VOICES vehicles, one per voice slot:
## `points` (world) and speeds in m/s (pitch); a negative speed or a slot
## past the list stops that voice. Call every frame; the caller keeps a
## vehicle in the same slot so its voice only moves.
func set_engine_loops(points: PackedVector3Array, speeds: PackedFloat32Array) -> void:
	if _silent:
		return
	var def: Array = SOUNDS[&"engine"]
	for i in _engines.size():
		var e := _engines[i]
		if i >= points.size() or (i < speeds.size() and speeds[i] < 0.0):
			if e.playing:
				e.stop()
			continue
		if e.stream == null:
			var st := _stream(&"engine")
			if st == null:
				return
			if st is AudioStreamOggVorbis:
				(st as AudioStreamOggVorbis).loop = true
			e.stream = st
			e.unit_size = def[3]
			e.max_distance = def[4]
			e.volume_db = def[5]
		e.global_position = points[i]
		var sp := absf(speeds[i]) if i < speeds.size() else 0.0
		e.pitch_scale = 0.85 + 0.09 * minf(sp, 7.0)
		if not e.playing:
			# Start voices at different points of the loop.
			e.play(randf() * 1.8)


## Engine voices playing (tests).
func engine_voices() -> int:
	var n := 0
	for e in _engines:
		if e.playing:
			n += 1
	return n


func stop_ambient() -> void:
	_ambient.stop()
	_ambient_name = &""


## Number of 3D voices playing (for tests and the debug overlay).
func active_voices() -> int:
	var n := 0
	for p in _pool:
		if p.playing:
			n += 1
	return n


## Silences the manager for good before quitting: stops every voice and
## ignores later play calls. Session.quit_game() calls it and lets the mixer
## run briefly, so no playback outlives the AudioServer (otherwise Godot
## reports leaks at exit).
func shutdown() -> void:
	_silent = true
	stop_all()


## Stops every voice and drops the cached streams.
func stop_all() -> void:
	stop_ambient()
	_ambient.stream = null
	for p in _pool:
		p.stop()
		p.stream = null
	for u in _ui:
		u.stop()
		u.stream = null
	for e in _engines:
		e.stop()
		e.stream = null
	_streams.clear()


func _exit_tree() -> void:
	stop_all()


func _on_node_added(node: Node) -> void:
	if node is BaseButton and not node.has_meta("no_click_sound"):
		(node as BaseButton).pressed.connect(play_ui.bind(&"click"))
