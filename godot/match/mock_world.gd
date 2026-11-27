extends Node3D
## Placeholder 3D world for the mock match: a ground mesh from MockMap's
## height function, woods as cone trees, soldiers as team-coloured capsules
## from state.soldiers (stance-posed), smoke clouds from state.smokes and
## thrown items from state.thrown. The integrator replaces this node with
## the FrontlineSim terrain and MultiMesh presenter (§4.1); it exists so the
## HUD can be run and screenshotted on its own.
##
## Each batch is one MultiMesh filled with one buffer upload per frame.

const GRID_M := 8.0
const MAX_SOLDIERS := 512
const MAX_SMOKE := 64
const MAX_THROWN := 64
const STRIDE := 16

var _soldiers: MultiMeshInstance3D
var _smoke: MultiMeshInstance3D
var _thrown: MultiMeshInstance3D
var _buf := PackedFloat32Array()


func _ready() -> void:
	# The match source may not have run yet: follow the session's map.
	if has_node("/root/Session"):
		MockMap.use(String(Session.setup.get("map", "kessel_ridge")))
	_build_ground()
	_build_trees()
	_build_roads()
	var cap := CapsuleMesh.new()
	cap.radius = 0.32
	cap.height = 1.75
	cap.radial_segments = 8
	cap.rings = 2
	_soldiers = _batch(cap, MAX_SOLDIERS, false)
	var puff := SphereMesh.new()
	puff.radius = 1.0
	puff.height = 1.2
	puff.radial_segments = 16
	puff.rings = 6
	_smoke = _batch(puff, MAX_SMOKE, true)
	var ball := SphereMesh.new()
	ball.radius = 0.18
	ball.height = 0.36
	ball.radial_segments = 6
	ball.rings = 3
	_thrown = _batch(ball, MAX_THROWN, false)


func _batch(mesh: Mesh, count: int, translucent: bool) -> MultiMeshInstance3D:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	mm.mesh = mesh
	mm.instance_count = count
	mm.visible_instance_count = 0
	var inst := MultiMeshInstance3D.new()
	inst.multimesh = mm
	var mat := StandardMaterial3D.new()
	mat.vertex_color_use_as_albedo = true
	mat.vertex_color_is_srgb = true
	mat.roughness = 0.9
	if translucent:
		mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
		mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
		inst.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	inst.material_override = mat
	inst.custom_aabb = AABB(Vector3(-1024, -128, -1024), Vector3(2048, 512, 2048))
	add_child(inst)
	return inst


func update_state(s: Dictionary) -> void:
	var local: int = s.get("local_player", 0)
	# Soldiers: capsules posed by stance.
	var items: Array = []
	for so in s.get("soldiers", []):
		var col := Color(0.30, 0.45, 0.62) if int(so.owner) == local else Color(0.62, 0.30, 0.25)
		var b := Basis()
		var lift := 0.875
		match so.get("stance", "stand"):
			"prone":
				b = Basis(Vector3.RIGHT, PI * 0.5)
				lift = 0.35
			"crouch":
				b = Basis().scaled(Vector3(1.0, 0.68, 1.0))
				lift = 0.6
		items.append([Transform3D(b, so.pos + Vector3(0, lift, 0)), col])
	# S12: a downed man lies flat, in a darker coat (the real match draws
	# him from its corpse layer).
	for c in s.get("casualties", []):
		var dcol := Color(0.20, 0.30, 0.42) if int(c.owner) == local else Color(0.42, 0.20, 0.17)
		items.append([Transform3D(Basis(Vector3.RIGHT, PI * 0.5).rotated(Vector3.UP, float(int(c.id) % 628) * 0.01), c.pos + Vector3(0, 0.2, 0)), dcol])
	_upload(_soldiers, items, MAX_SOLDIERS)
	# Smoke: flattened grey spheres, alpha from density.
	items = []
	for c in s.get("smokes", []):
		var r := maxf(float(c.radius), 0.3)
		var b := Basis().scaled(Vector3(r, r * 0.55, r))
		items.append([Transform3D(b, c.pos + Vector3(0, r * 0.35, 0)), Color(0.78, 0.78, 0.74, 0.82 * float(c.density))])
	_upload(_smoke, items, MAX_SMOKE)
	# Items in flight (and fused on the ground).
	items = []
	for th in s.get("thrown", []):
		var col := Color(0.85, 0.85, 0.80) if th.kind == "smoke" else (Color(0.55, 0.42, 0.25) if th.kind == "satchel" else Color(0.25, 0.30, 0.22))
		items.append([Transform3D(Basis(), th.pos), col])
	_upload(_thrown, items, MAX_THROWN)


## One buffer upload: [Transform3D, Color] per instance.
func _upload(inst: MultiMeshInstance3D, items: Array, cap: int) -> void:
	var mm := inst.multimesh
	var n := mini(items.size(), cap)
	_buf.resize(cap * STRIDE)
	for i in n:
		var t: Transform3D = items[i][0]
		var c: Color = items[i][1]
		var o := i * STRIDE
		_buf[o + 0] = t.basis.x.x
		_buf[o + 1] = t.basis.y.x
		_buf[o + 2] = t.basis.z.x
		_buf[o + 3] = t.origin.x
		_buf[o + 4] = t.basis.x.y
		_buf[o + 5] = t.basis.y.y
		_buf[o + 6] = t.basis.z.y
		_buf[o + 7] = t.origin.y
		_buf[o + 8] = t.basis.x.z
		_buf[o + 9] = t.basis.y.z
		_buf[o + 10] = t.basis.z.z
		_buf[o + 11] = t.origin.z
		_buf[o + 12] = c.r
		_buf[o + 13] = c.g
		_buf[o + 14] = c.b
		_buf[o + 15] = c.a
	mm.buffer = _buf
	mm.visible_instance_count = n


func _build_ground() -> void:
	var cells := int(MockMap.SIZE_M / GRID_M)
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	for j in cells + 1:
		for i in cells + 1:
			var x := -MockMap.HALF + i * GRID_M
			var z := -MockMap.HALF + j * GRID_M
			var h := MockMap.height(x, z)
			var c := Color(0.36, 0.42, 0.26).lerp(Color(0.55, 0.52, 0.36), clampf((h + 4.0) / 22.0, 0.0, 1.0))
			if MockMap.in_woods(Vector2(x, z)):
				c = Color(0.22, 0.27, 0.16)
			st.set_color(c)
			st.set_uv(Vector2(float(i) / cells, float(j) / cells))
			st.add_vertex(Vector3(x, h, z))
	var row := cells + 1
	for j in cells:
		for i in cells:
			var a := j * row + i
			st.add_index(a)
			st.add_index(a + 1)
			st.add_index(a + row)
			st.add_index(a + 1)
			st.add_index(a + row + 1)
			st.add_index(a + row)
	st.generate_normals()
	var mesh := st.commit()
	var mat := StandardMaterial3D.new()
	mat.vertex_color_use_as_albedo = true
	mat.vertex_color_is_srgb = true
	mat.roughness = 1.0
	var ground := MeshInstance3D.new()
	ground.name = "Ground"
	ground.mesh = mesh
	ground.material_override = mat
	add_child(ground)


func _build_roads() -> void:
	var inst := RoadStyle.build(MockMap.ROADS, MockMap.height)
	if inst != null:
		add_child(inst)


func _build_trees() -> void:
	var rng := RandomNumberGenerator.new()
	rng.seed = 4242
	var xforms: Array[Transform3D] = []
	for w in MockMap.WOODS:
		var count := int(w.r * w.r / 110.0)
		for k in count:
			var a := rng.randf() * TAU
			var r: float = sqrt(rng.randf()) * w.r
			var p: Vector2 = w.c + Vector2(cos(a), sin(a)) * r
			var s := rng.randf_range(0.8, 1.3)
			var h := MockMap.height(p.x, p.y)
			xforms.append(Transform3D(Basis().scaled(Vector3(s, s, s)), Vector3(p.x, h + 3.5 * s, p.y)))
	var cone := CylinderMesh.new()
	cone.top_radius = 0.0
	cone.bottom_radius = 2.2
	cone.height = 7.0
	cone.radial_segments = 7
	cone.rings = 1
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.mesh = cone
	mm.instance_count = xforms.size()
	for i in xforms.size():
		mm.set_instance_transform(i, xforms[i])
	var trees := MultiMeshInstance3D.new()
	trees.name = "Trees"
	trees.multimesh = mm
	var mat := StandardMaterial3D.new()
	mat.albedo_color = Color(0.17, 0.25, 0.15)
	mat.roughness = 1.0
	trees.material_override = mat
	add_child(trees)
