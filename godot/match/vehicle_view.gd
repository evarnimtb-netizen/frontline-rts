class_name VehicleView
extends Node3D
## S3 vehicle renderer: state.vehicles (own, seen enemy and wrecks) drawn
## as one MultiMesh per (type, part): hull, turret and gun from the
## procedural meshes in crates/godot-bridge/src/meshgen.rs
## (FrontlineMeshes). Instance transforms are composed in Rust, hull ×
## turret yaw × gun pitch (FrontlineMeshes.vehicle_buffers), and each
## batch gets one buffer upload per frame (§4.1); no Node per vehicle.
##
## Turret yaw follows the state by the shortest arc, hull pitch and roll
## and gun pitch are smoothed (the state's pose is per tick). Team colours
## ride in the instance custom data: the faction's paint mixed toward the
## side colour; wrecks are charred and dark.
##
## Without the GDExtension (a project check with no bridge built) the parts
## fall back to plain boxes composed here, so the scene still runs.

## Instances reserved per (type, part): 4 + 4 alive and up to 32 wrecks.
const CAP := 48
const TURRET_RATE := 14.0
const POSE_RATE := 6.0
const WRECK_TINT := Color(0.20, 0.18, 0.16)
const WRECK_SHADE := 0.55
const POSE_STRIDE := 12

## type key -> {dims, batches: [MultiMeshInstance3D ×3], poses}
var _types := {}
## id -> [turret_yaw, pitch, roll, gun_pitch] as drawn
var _drawn := {}
var _mat: ShaderMaterial
var _rust := false
var _local := 0
var _seen_ids := {}
## Instances drawn last frame per type key (tests, stats).
var counts := {}
## CPU time of the last update in microseconds (tests).
var last_us := 0


func _ready() -> void:
	_rust = ClassDB.class_exists(&"FrontlineMeshes")
	_mat = ShaderMaterial.new()
	_mat.shader = load("res://shaders/vehicle_parts.gdshader")


## Draws one frame of state.vehicles.
func update_state(s: Dictionary) -> void:
	var t0 := Time.get_ticks_usec()
	_local = int(s.get("local_player", 0))
	var vtypes: Dictionary = s.get("rules", {}).get("vehicle_types", {})
	var dt := get_process_delta_time()
	var k_turret := 1.0 - exp(-dt * TURRET_RATE)
	var k_pose := 1.0 - exp(-dt * POSE_RATE)
	# Group by type first (Arrays are shared, so appending is cheap).
	var groups := {}
	for key in _types:
		groups[key] = []
	_seen_ids.clear()
	for v in s.get("vehicles", []):
		var key := String(v.type)
		if not _types.has(key):
			if not vtypes.has(key):
				continue
			_types[key] = _make_type(key, vtypes[key])
			groups[key] = []
		var g: Array = groups[key]
		if g.size() >= CAP:
			continue
		g.append(v)
		var id := int(v.id)
		_seen_ids[id] = true
		var d: Array = _drawn.get(id, [])
		if d.is_empty():
			_drawn[id] = [float(v.turret_yaw), float(v.pitch), float(v.roll), float(v.gun_pitch)]
		else:
			d[0] = lerp_angle(d[0], float(v.turret_yaw), k_turret)
			d[1] = lerpf(d[1], float(v.pitch), k_pose)
			d[2] = lerpf(d[2], float(v.roll), k_pose)
			d[3] = lerpf(d[3], float(v.gun_pitch), k_turret)
	for id in _drawn.keys():
		if not _seen_ids.has(id):
			_drawn.erase(id)
	for key in _types:
		var tp: Dictionary = _types[key]
		var g: Array = groups[key]
		var n := g.size()
		counts[key] = n
		if n == 0 and tp.shown == 0:
			continue
		var poses := PackedFloat32Array()
		poses.resize(n * POSE_STRIDE)
		for i in n:
			var v: Dictionary = g[i]
			var d: Array = _drawn[int(v.id)]
			var wreck := Vehicles.is_wreck(v)
			var col: Color = WRECK_TINT if wreck else tint(tp.faction, int(v.owner), _local)
			var p: Vector3 = v.pos
			var o := i * POSE_STRIDE
			poses[o] = p.x
			poses[o + 1] = p.y
			poses[o + 2] = p.z
			poses[o + 3] = float(v.yaw)
			poses[o + 4] = d[1]
			poses[o + 5] = d[2]
			poses[o + 6] = d[0]
			poses[o + 7] = d[3]
			poses[o + 8] = col.r
			poses[o + 9] = col.g
			poses[o + 10] = col.b
			poses[o + 11] = WRECK_SHADE if wreck else 1.0
		tp.poses = poses
		tp.n = n
		tp.shown = n
		_upload(tp)
	last_us = Time.get_ticks_usec() - t0


## Team tint: the faction's paint mixed toward the side colour.
static func tint(faction: String, owner: int, local: int) -> Color:
	var paint: Color = Vehicles.PAINT.get(faction, Color(0.45, 0.45, 0.4))
	var side := Vehicles.TEAM_OWN if owner == local else Vehicles.TEAM_ENEMY
	return paint.lerp(side, Vehicles.TEAM_MIX)


func _upload(tp: Dictionary) -> void:
	var batches: Array = tp.batches
	if _rust:
		var bufs: Array = ClassDB.class_call_static(&"FrontlineMeshes", &"vehicle_buffers", tp.dims, tp.poses, CAP)
		for k in 3:
			var mm: MultiMesh = batches[k].multimesh
			mm.buffer = bufs[k]
			mm.visible_instance_count = tp.n
		return
	for k in 3:
		var buf := PackedFloat32Array()
		buf.resize(CAP * 16)
		for i in tp.n:
			var xf: Transform3D = _compose_gd(tp.poses, i * POSE_STRIDE, tp.dims)[k]
			var o: int = i * 16
			var b := xf.basis
			var row := [b.x.x, b.y.x, b.z.x, xf.origin.x, b.x.y, b.y.y, b.z.y, xf.origin.y, b.x.z, b.y.z, b.z.z, xf.origin.z]
			for j in 12:
				buf[o + j] = row[j]
			for j in 4:
				buf[o + 12 + j] = tp.poses[i * POSE_STRIDE + 8 + j]
		var mm: MultiMesh = batches[k].multimesh
		mm.buffer = buf
		mm.visible_instance_count = tp.n


func _make_type(key: String, vt: Dictionary) -> Dictionary:
	var dims: Dictionary = vt.duplicate()
	var faction := String(vt.get("faction", "germany"))
	if not dims.has("glacis_deg"):
		dims.glacis_deg = Vehicles.GLACIS_DEG.get(faction, 10.0)
	var batches: Array = []
	for part in ["hull", "turret", "gun"]:
		var mesh: Mesh = null
		if _rust:
			mesh = ClassDB.class_call_static(&"FrontlineMeshes", &"vehicle_part", dims, part)
		if mesh == null:
			mesh = _fallback_mesh(part, dims)
		var mm := MultiMesh.new()
		mm.transform_format = MultiMesh.TRANSFORM_3D
		mm.use_custom_data = true
		mm.mesh = mesh
		mm.instance_count = CAP
		mm.visible_instance_count = 0
		var inst := MultiMeshInstance3D.new()
		inst.name = "%s_%s" % [key, part]
		inst.multimesh = mm
		inst.material_override = _mat
		inst.custom_aabb = AABB(Vector3(-2048, -256, -2048), Vector3(4096, 1024, 4096))
		add_child(inst)
		batches.append(inst)
	return {"dims": dims, "faction": faction, "batches": batches, "poses": PackedFloat32Array(), "n": 0, "shown": 0}


## The MultiMesh of one (type, part), or null (tests).
func batch(key: String, part: int) -> MultiMesh:
	if not _types.has(key):
		return null
	return _types[key].batches[part].multimesh


# --- fallback without the GDExtension ----------------------------------------

static func _fallback_mesh(part: String, d: Dictionary) -> Mesh:
	var b := BoxMesh.new()
	var st := StandardMaterial3D.new()
	match part:
		"hull":
			b.size = Vector3(float(d.get("hull_width", 2.8)), float(d.get("hull_height", 1.36)), float(d.get("hull_length", 5.8)))
		"turret":
			b.size = Vector3(float(d.get("turret_width", 1.9)), float(d.get("turret_height", 0.9)), float(d.get("turret_length", 2.3)))
		_:
			b.size = Vector3(0.18, 0.18, float(d.get("gun_length", 4.5)) * 0.6)
	var arrays := b.get_mesh_arrays()
	var verts: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var lift := Vector3(0, b.size.y * 0.5, 0)
	if part == "hull":
		lift.y += float(d.get("hull_ground", 0.4))
	elif part == "gun":
		lift = Vector3(0, 0, -b.size.z * 0.5)
	for i in verts.size():
		verts[i] += lift
	arrays[Mesh.ARRAY_VERTEX] = verts
	var cols := PackedColorArray()
	cols.resize(verts.size())
	cols.fill(Color(1, 1, 1, 1) if part != "gun" else Color(0.2, 0.2, 0.2, 0))
	arrays[Mesh.ARRAY_COLOR] = cols
	var m := ArrayMesh.new()
	m.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	st.vertex_color_use_as_albedo = true
	return m


## GDScript copy of meshgen::compose_vehicle (fallback only).
static func _compose_gd(poses: PackedFloat32Array, o: int, d: Dictionary) -> Array:
	var pos := Vector3(poses[o], poses[o + 1], poses[o + 2])
	var hull := Basis(Vector3.UP, poses[o + 3]) * Basis(Vector3.RIGHT, poses[o + 4]) * Basis(Vector3.BACK, -poses[o + 5])
	var pivot := pos + hull * Vector3(0, float(d.get("turret_base", 1.76)), -float(d.get("turret_x", 0.2)))
	var turret := hull * Basis(Vector3.UP, poses[o + 6])
	var tf := minf(float(d.get("turret_offset_x", 0.0)) + float(d.get("turret_length", 2.3)) * 0.42, float(d.get("gun_length", 4.5)) * 0.5)
	var trunnion := pivot + turret * Vector3(0, float(d.get("gun_height", 0.45)), -tf)
	var gun := turret * Basis(Vector3.RIGHT, poses[o + 7])
	return [Transform3D(hull, pos), Transform3D(turret, pivot), Transform3D(gun, trunnion)]
