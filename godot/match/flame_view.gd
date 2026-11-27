extends Node3D
## S14 renderer, over the sim's own state: the flame stream (short bright
## boxes, one per particle in flight, orange to yellow by id), burning
## ground (a flickering disc over each patch with a small light under the
## first few), a small flame over each burning man and the strike pose (a
## pale bar over the striker: raised in the wind-up, thrust toward the
## nearest enemy in the recovery). Placeholder quality, cheap: one
## MultiMesh for the stream, pooled meshes for the rest, everything hidden
## while the state carries none of the three fields (no fire yet, or the
## S14 switches off).
## STREAM_MAX is two players' cap (`MAX_FLAME_PER_PLAYER` 256 each).

const STREAM_MAX := 512
const LIGHTS_MAX := 3
const PATCH_COL := Color(1.0, 0.45, 0.10, 0.55)
const MAN_FLAME := Color(1.0, 0.62, 0.15)
const STRIKE_COL := Color(0.92, 0.92, 0.85)

var _stream: MultiMeshInstance3D
var _discs: Array[MeshInstance3D] = []
var _lights: Array[OmniLight3D] = []
var _men: Array[MeshInstance3D] = []
var _bars: Array[MeshInstance3D] = []
var _disc_mat: StandardMaterial3D
var _man_mat: StandardMaterial3D
var _bar_mat: StandardMaterial3D
var _t := 0.0
## Drawn last frame (tests).
var particles := 0
var patches := 0
var burning := 0
var strikes := 0
## Last frame's strike bars by man id: phase (1 wind-up, 2 recovery) (tests).
var strike_phase: Dictionary = {}


func _ready() -> void:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	var box := BoxMesh.new()
	box.size = Vector3(0.28, 0.28, 0.5)
	mm.mesh = box
	mm.instance_count = STREAM_MAX
	mm.visible_instance_count = 0
	_stream = MultiMeshInstance3D.new()
	_stream.multimesh = mm
	_stream.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	var m := StandardMaterial3D.new()
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	m.vertex_color_use_as_albedo = true
	m.billboard_mode = BaseMaterial3D.BILLBOARD_ENABLED
	_stream.material_override = m
	_stream.visible = false
	add_child(_stream)
	_disc_mat = _unshaded(PATCH_COL, true)
	_man_mat = _unshaded(MAN_FLAME, false)
	_bar_mat = _unshaded(STRIKE_COL, false)
	for i in LIGHTS_MAX:
		var l := OmniLight3D.new()
		l.light_color = Color(1.0, 0.5, 0.15)
		l.omni_range = 7.0
		l.shadow_enabled = false
		l.visible = false
		add_child(l)
		_lights.append(l)


func update_state(s: Dictionary) -> void:
	_t += get_process_delta_time()
	var flame: Array = s.get("flame", [])
	var pats: Array = s.get("patches", [])
	var soldiers: Array = s.get("soldiers", [])
	particles = 0
	patches = 0
	burning = 0
	strikes = 0
	strike_phase.clear()
	# Stream.
	var mm := _stream.multimesh
	var n := mini(flame.size(), STREAM_MAX)
	for i in n:
		var p: Vector3 = flame[i].pos
		var id := int(flame[i].id)
		var k := float(id % 7) / 6.0
		mm.set_instance_transform(i, Transform3D(Basis.from_scale(Vector3.ONE * (0.8 + 0.5 * k)), p))
		mm.set_instance_color(i, Color(1.0, 0.35 + 0.55 * k, 0.08 + 0.2 * k, 0.9))
	mm.visible_instance_count = n
	_stream.visible = n > 0
	particles = n
	# Burning ground: a flickering disc, a light under the first few.
	for i in pats.size():
		var d := _disc(i)
		var r := float(pats[i].get("radius", 1.5))
		var flick := 0.85 + 0.15 * sin(_t * 13.0 + float(int(pats[i].id)) * 1.7)
		d.position = (pats[i].pos as Vector3) + Vector3(0, 0.06, 0)
		d.scale = Vector3(r * flick, 1.0, r * flick)
		d.visible = true
		if i < LIGHTS_MAX:
			var l := _lights[i]
			l.position = d.position + Vector3(0, 0.8, 0)
			l.light_energy = 1.2 * flick
			l.visible = true
	for i in range(pats.size(), _discs.size()):
		_discs[i].visible = false
	for i in range(mini(pats.size(), LIGHTS_MAX), LIGHTS_MAX):
		_lights[i].visible = false
	patches = pats.size()
	# Burning men and strike poses.
	var nb := 0
	var nk := 0
	for so in soldiers:
		if bool(so.get("burning", false)):
			var f := _pool(_men, nb, Vector3(0.32, 0.5, 0.32), _man_mat)
			var wob := 0.08 * sin(_t * 17.0 + float(int(so.id)))
			f.position = (so.pos as Vector3) + Vector3(wob, 1.0 if so.get("stance", "stand") == "stand" else 0.6, 0)
			f.scale = Vector3(1.0, 0.8 + 0.4 * absf(sin(_t * 11.0 + float(int(so.id)))), 1.0)
			f.visible = true
			nb += 1
		var ph := int(so.get("strike", 0))
		if ph != 0:
			var bar := _pool(_bars, nk, Vector3(0.1, 0.1, 1.1), _bar_mat)
			var from: Vector3 = so.pos
			var aim := _toward_enemy(so, soldiers)
			var yaw := atan2(-aim.x, -aim.z)
			bar.rotation = Vector3(0, yaw, 0)
			if ph == 1:
				# Wind-up: held up and back.
				bar.rotation.x = deg_to_rad(55.0)
				bar.position = from + Vector3(0, 1.6, 0) - aim * 0.2
			else:
				# Recovery: thrust out level at the chest.
				bar.position = from + Vector3(0, 1.2, 0) + aim * 0.6
			bar.visible = true
			strike_phase[int(so.id)] = ph
			nk += 1
	for i in range(nb, _men.size()):
		_men[i].visible = false
	for i in range(nk, _bars.size()):
		_bars[i].visible = false
	burning = nb
	strikes = nk


## Unit vector (ground plane) from `so` to the nearest man of another
## owner, or forward (-z) when none is in the state.
func _toward_enemy(so: Dictionary, soldiers: Array) -> Vector3:
	var best := INF
	var out := Vector3(0, 0, -1)
	var p: Vector3 = so.pos
	for o in soldiers:
		if int(o.owner) == int(so.owner):
			continue
		var d: Vector3 = (o.pos as Vector3) - p
		d.y = 0.0
		var l := d.length_squared()
		if l > 0.0001 and l < best:
			best = l
			out = d / sqrt(l)
	return out


func _disc(i: int) -> MeshInstance3D:
	while _discs.size() <= i:
		var mi := MeshInstance3D.new()
		var c := CylinderMesh.new()
		c.top_radius = 1.0
		c.bottom_radius = 1.0
		c.height = 0.04
		c.radial_segments = 16
		c.rings = 1
		mi.mesh = c
		mi.material_override = _disc_mat
		mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		add_child(mi)
		_discs.append(mi)
	return _discs[i]


func _pool(arr: Array[MeshInstance3D], i: int, size: Vector3, mat: Material) -> MeshInstance3D:
	while arr.size() <= i:
		var mi := MeshInstance3D.new()
		var b := BoxMesh.new()
		b.size = size
		mi.mesh = b
		mi.material_override = mat
		mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		add_child(mi)
		arr.append(mi)
	return arr[i]


func _unshaded(col: Color, alpha: bool) -> StandardMaterial3D:
	var m := StandardMaterial3D.new()
	m.albedo_color = col
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	if alpha:
		m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	return m


## Everything drawn last frame.
func drawn() -> int:
	return particles + patches + burning + strikes
