extends Node3D
## Placeholder 3D world for the mock match: a ground mesh from MockMap's
## height function, woods as cone trees, and soldiers as team-coloured
## capsules from state squads[].members. The integrator replaces this node
## with the FrontlineSim terrain and MultiMesh presenter (§4.1); it exists
## so the HUD can be run and screenshotted on its own.

const GRID_M := 8.0
const MAX_SOLDIERS := 512

var _soldiers: MultiMeshInstance3D


func _ready() -> void:
	_build_ground()
	_build_trees()
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	var cap := CapsuleMesh.new()
	cap.radius = 0.32
	cap.height = 1.75
	cap.radial_segments = 8
	cap.rings = 2
	mm.mesh = cap
	mm.instance_count = MAX_SOLDIERS
	mm.visible_instance_count = 0
	_soldiers = MultiMeshInstance3D.new()
	_soldiers.multimesh = mm
	var mat := StandardMaterial3D.new()
	mat.vertex_color_use_as_albedo = true
	mat.vertex_color_is_srgb = true
	mat.roughness = 0.9
	_soldiers.material_override = mat
	add_child(_soldiers)


func update_state(s: Dictionary) -> void:
	var mm := _soldiers.multimesh
	var local: int = s.get("local_player", 0)
	var n := 0
	for sq in s.get("squads", []):
		var col := Color(0.30, 0.45, 0.62) if int(sq.owner) == local else Color(0.62, 0.30, 0.25)
		var pinned: bool = sq.get("status", "") == "pinned"
		for p in sq.get("members", PackedVector3Array()):
			if n >= MAX_SOLDIERS:
				break
			var b := Basis()
			var lift := 0.875
			if pinned:
				b = Basis(Vector3.RIGHT, PI * 0.5)  # prone
				lift = 0.35
			mm.set_instance_transform(n, Transform3D(b, p + Vector3(0, lift, 0)))
			mm.set_instance_color(n, col)
			n += 1
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
