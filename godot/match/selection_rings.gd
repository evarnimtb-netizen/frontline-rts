class_name SelectionRings
extends MultiMeshInstance3D
## Rings under soldiers (S1b: one under each selected man, plus the hovered
## one), drawn as ONE MultiMesh batch with per-instance colour: one buffer
## upload per frame (§4.1 rules 2–3). The ring is a flat annulus mesh lying
## a little above the ground.

const INNER := 0.42
const OUTER := 0.62
const SEGMENTS := 20
const LIFT := 0.12
## Floats per instance: 12 (3x4 transform, row-major) + 4 (colour).
const STRIDE := 16

var _buf := PackedFloat32Array()
var _cap := 0


func _ready() -> void:
	cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	mm.mesh = _annulus()
	multimesh = mm
	var mat := StandardMaterial3D.new()
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mat.vertex_color_use_as_albedo = true
	mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	mat.cull_mode = BaseMaterial3D.CULL_DISABLED
	mat.render_priority = 2
	material_override = mat
	custom_aabb = AABB(Vector3(-4096, -512, -4096), Vector3(8192, 1024, 8192))
	_reserve(128)


func _reserve(n: int) -> void:
	if n <= _cap:
		return
	_cap = maxi(n, _cap * 2)
	multimesh.instance_count = _cap
	_buf.resize(_cap * STRIDE)
	_buf.fill(0.0)


## Rings at these points (the soldiers' feet, already on the ground) with
## these colours; one upload.
func set_rings(points: PackedVector3Array, colors: PackedColorArray) -> void:
	var n := points.size()
	_reserve(n)
	for i in n:
		var p := points[i]
		var y := p.y
		var o := i * STRIDE
		_buf[o + 0] = 1.0
		_buf[o + 1] = 0.0
		_buf[o + 2] = 0.0
		_buf[o + 3] = p.x
		_buf[o + 4] = 0.0
		_buf[o + 5] = 1.0
		_buf[o + 6] = 0.0
		_buf[o + 7] = y + LIFT
		_buf[o + 8] = 0.0
		_buf[o + 9] = 0.0
		_buf[o + 10] = 1.0
		_buf[o + 11] = p.z
		var c := colors[i]
		_buf[o + 12] = c.r
		_buf[o + 13] = c.g
		_buf[o + 14] = c.b
		_buf[o + 15] = c.a
	multimesh.buffer = _buf
	multimesh.visible_instance_count = n


static func _annulus() -> ArrayMesh:
	var verts := PackedVector3Array()
	var idx := PackedInt32Array()
	for i in SEGMENTS:
		var a := TAU * i / SEGMENTS
		var d := Vector3(cos(a), 0.0, sin(a))
		verts.append(d * INNER)
		verts.append(d * OUTER)
	for i in SEGMENTS:
		var j := (i + 1) % SEGMENTS
		idx.append_array([i * 2, i * 2 + 1, j * 2 + 1, i * 2, j * 2 + 1, j * 2])
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = verts
	arrays[Mesh.ARRAY_INDEX] = idx
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	return mesh
