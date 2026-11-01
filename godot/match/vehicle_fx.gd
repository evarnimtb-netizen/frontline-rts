class_name VehicleFx
extends Node3D
## S3 vehicle effects and engine sounds from the match state (spec
## "Presenter"): muzzle flash and dust (effects `cannon`), backblast
## (`rocket_launch`), sparks on ricochets and non-penetrations and a flash
## on penetrations (`armour_hit`), cook-offs (`cook_off`), bright shell
## tracers and rocket trails (state.shells, ONE MultiMesh batch, one upload
## per frame), fires on burning vehicles (at most MAX_FIRES), smoke over
## the nearest MAX_SMOKES knocked-out wrecks and dust behind the nearest
## MAX_DUST vehicles driving faster than DUST_SPEED. No track decals.
##
## Everything comes from fixed pools made in _ready and reused oldest
## first (one-shot effects) or reassigned each frame to the nearest
## vehicles (fires, smoke, dust): no Node per vehicle, nothing created
## during play. Cosmetic and non-deterministic (§12); it never feeds the
## sim. Engine loops: the 4 nearest moving vehicles through
## AudioManager.set_engine_loops, a vehicle keeping its voice slot.

const FLASH_POOL := 8
const SPARK_POOL := 8
const PEN_POOL := 6
const COOK_POOL := 3
const MAX_FIRES := 8
const MAX_SMOKES := 16
const MAX_DUST := 8
const DUST_SPEED := 2.0
const ENGINE_VOICES := 4
## Tracer instances: one per shell, two per rocket (trail and head).
const MAX_STREAKS := 96
const STREAK_STRIDE := 16
## Tracer length per m/s of shell speed (a 0.04 s streak) and width.
const STREAK_S := 0.04
const STREAK_W := 0.22
const ROCKET_TRAIL_M := 14.0

var _flashes: Array[Dictionary] = []
var _sparks: Array[Dictionary] = []
var _pens: Array[Dictionary] = []
var _cooks: Array[Dictionary] = []
var _fires: Array[Dictionary] = []
var _smokes: Array[GPUParticles3D] = []
var _dust: Array[GPUParticles3D] = []
var _next := {"flash": 0, "spark": 0, "pen": 0, "cook": 0}
var _streaks: MultiMeshInstance3D
var _streak_buf := PackedFloat32Array()
var _streak_n := 0
var _engine_slots: Array[int] = [-1, -1, -1, -1]
var _disc: GradientTexture2D
## What was drawn last frame (tests): fires, smokes, dust, streaks.
var counts := {"fires": 0, "smokes": 0, "dust": 0, "streaks": 0, "engines": 0}


func _ready() -> void:
	_disc = _soft_disc()
	for i in FLASH_POOL:
		_flashes.append(_make_flash())
	for i in SPARK_POOL:
		_sparks.append(_make_sparks())
	for i in PEN_POOL:
		_pens.append(_make_pen())
	for i in COOK_POOL:
		_cooks.append(_make_cook())
	for i in MAX_FIRES:
		_fires.append(_make_fire())
	for i in MAX_SMOKES:
		_smokes.append(_continuous(20, 7.0, _smoke_material(1.6, 2.2), _puff_mesh(Color(0.16, 0.15, 0.14, 0.55)), 0.0))
	for i in MAX_DUST:
		_dust.append(_continuous(14, 1.8, _dust_material(), _puff_mesh(Color(0.52, 0.45, 0.34, 0.5)), 0.0))
	_streaks = _make_streaks()


func update_state(s: Dictionary) -> void:
	var dt := get_process_delta_time()
	for fx in s.get("effects", []):
		match String(fx.get("kind", "")):
			"cannon":
				_fire_flash(fx.pos, fx.get("dir", Vector3.FORWARD), 1.0)
			"rocket_launch":
				# Backblast: a small flash and dust behind the launcher.
				_fire_flash(fx.pos, -Vector3(fx.get("dir", Vector3.FORWARD)), 0.45)
			"armour_hit":
				if fx.get("result", "") == "pen":
					_pen_flash(fx.pos)
				else:
					_spark(fx.pos)
			"cook_off":
				_cook_off(fx.pos)
	_tick_one_shots(dt)
	_update_streaks(s.get("shells", []))
	_update_vehicles(s, dt)


# --- one-shot effects -------------------------------------------------------

func _take(pool: Array, key: String) -> Dictionary:
	var e: Dictionary = pool[_next[key]]
	_next[key] = (_next[key] + 1) % pool.size()
	return e


func _fire_flash(pos: Vector3, dir: Vector3, scale: float) -> void:
	var e := _take(_flashes, "flash")
	var d := dir.normalized() if dir.length() > 0.01 else Vector3.FORWARD
	var root: Node3D = e.root
	root.global_position = pos
	# The flash ball stretches along the barrel.
	var ball: MeshInstance3D = e.ball
	ball.global_basis = _basis_along(d) * Basis.from_scale(Vector3(0.9, 0.9, 2.6) * scale)
	ball.global_position = pos + d * 1.0 * scale
	e.scale = scale
	e.t = 0.0
	var dust: GPUParticles3D = e.dust
	dust.global_position = Vector3(pos.x, pos.y - 1.8 * scale, pos.z) + d * 2.0
	dust.restart()
	dust.emitting = true


func _spark(pos: Vector3) -> void:
	var e := _take(_sparks, "spark")
	var p: GPUParticles3D = e.sparks
	p.global_position = pos
	p.restart()
	p.emitting = true
	var puff: GPUParticles3D = e.puff
	puff.global_position = pos
	puff.restart()
	puff.emitting = true


func _pen_flash(pos: Vector3) -> void:
	var e := _take(_pens, "pen")
	var root: Node3D = e.root
	root.global_position = pos
	e.t = 0.0
	var p: GPUParticles3D = e.sparks
	p.restart()
	p.emitting = true


func _cook_off(pos: Vector3) -> void:
	var e := _take(_cooks, "cook")
	var root: Node3D = e.root
	root.global_position = pos
	e.t = 0.0
	for k in ["debris", "jet", "smoke"]:
		var p: GPUParticles3D = e[k]
		p.restart()
		p.emitting = true


func _tick_one_shots(dt: float) -> void:
	for e in _flashes:
		if e.t < 0.0:
			continue
		e.t += dt
		var k: float = e.t
		var light: OmniLight3D = e.light
		light.light_energy = 9.0 * float(e.scale) * maxf(1.0 - k / 0.12, 0.0)
		light.visible = k < 0.12
		var ball: MeshInstance3D = e.ball
		ball.visible = k < 0.07
		if k > 3.0:
			e.t = -1.0
	for e in _pens:
		if e.t < 0.0:
			continue
		e.t += dt
		var k: float = e.t
		var light: OmniLight3D = e.light
		light.light_energy = 7.0 * maxf(1.0 - k / 0.25, 0.0)
		light.visible = k < 0.25
		var ball: MeshInstance3D = e.ball
		ball.visible = k < 0.12
		ball.scale = Vector3.ONE * (0.4 + 6.0 * k)
		if k > 2.0:
			e.t = -1.0
	for e in _cooks:
		if e.t < 0.0:
			continue
		e.t += dt
		var k: float = e.t
		var light: OmniLight3D = e.light
		light.light_energy = 22.0 * maxf(1.0 - k / 0.9, 0.0)
		light.visible = k < 0.9
		var ball: MeshInstance3D = e.ball
		ball.visible = k < 0.6
		ball.scale = Vector3.ONE * (1.0 + 4.5 * clampf(k / 0.25, 0.0, 1.0))
		(ball.material_override as StandardMaterial3D).albedo_color = Color(1.0, 0.62, 0.25) * clampf(1.0 - k / 0.6, 0.0, 1.0)
		if k > 25.0:
			e.t = -1.0


# --- tracers ----------------------------------------------------------------

## Shells as bright streaks behind their heads (AP, HE); rockets as a long
## pale trail plus a short bright head. One upload.
func _update_streaks(shells: Array) -> void:
	var n := 0
	for sh in shells:
		if n >= MAX_STREAKS - 1:
			break
		var pos: Vector3 = sh.pos
		var vel: Vector3 = sh.vel
		var speed := vel.length()
		if speed < 0.1:
			continue
		var d := vel / speed
		if String(sh.get("kind", "ap")) == "heat":
			_streak(n, pos, d, minf(ROCKET_TRAIL_M, speed * 0.15), STREAK_W * 2.2, Color(0.55, 0.53, 0.5, 0.55))
			n += 1
			_streak(n, pos, d, 1.2, STREAK_W * 1.6, Color(1.0, 0.75, 0.35, 1.0))
			n += 1
		else:
			var col := Color(1.0, 0.85, 0.55, 1.0) if sh.get("kind", "ap") == "ap" else Color(1.0, 0.6, 0.3, 1.0)
			_streak(n, pos, d, maxf(speed * STREAK_S, 3.0), STREAK_W, col)
			n += 1
	counts.streaks = n
	if n == 0 and _streak_n == 0:
		return
	_streak_n = n
	var mm := _streaks.multimesh
	mm.buffer = _streak_buf
	mm.visible_instance_count = n


func _streak(i: int, head: Vector3, d: Vector3, length: float, width: float, col: Color) -> void:
	# Local z 0 (head) .. 1 (tail) runs back along −d.
	var z := -d * length
	var helper := Vector3.UP if absf(d.y) < 0.95 else Vector3.RIGHT
	var x := helper.cross(z).normalized() * width
	var y := z.cross(x).normalized() * width
	var o := i * STREAK_STRIDE
	_streak_buf[o + 0] = x.x
	_streak_buf[o + 1] = y.x
	_streak_buf[o + 2] = z.x
	_streak_buf[o + 3] = head.x
	_streak_buf[o + 4] = x.y
	_streak_buf[o + 5] = y.y
	_streak_buf[o + 6] = z.y
	_streak_buf[o + 7] = head.y
	_streak_buf[o + 8] = x.z
	_streak_buf[o + 9] = y.z
	_streak_buf[o + 10] = z.z
	_streak_buf[o + 11] = head.z
	_streak_buf[o + 12] = col.r
	_streak_buf[o + 13] = col.g
	_streak_buf[o + 14] = col.b
	_streak_buf[o + 15] = col.a


# --- per-vehicle effects ------------------------------------------------------

## Assigns fires, wreck smoke, dust and engine voices to the nearest
## vehicles that need them; spare rigs stop emitting.
func _update_vehicles(s: Dictionary, _dt: float) -> void:
	var cam := get_viewport().get_camera_3d()
	var eye := cam.global_position if cam != null else Vector3.ZERO
	var vtypes: Dictionary = s.get("rules", {}).get("vehicle_types", {})
	var burning: Array = []
	var wrecks: Array = []
	var moving: Array = []
	var engines: Array = []
	for v in s.get("vehicles", []):
		var d: float = eye.distance_squared_to(v.pos)
		var f := int(v.get("flags", 0))
		if f & Vehicles.BURNING and not f & Vehicles.WRECK:
			burning.append([d, v])
		if f & Vehicles.KNOCKED_OUT:
			wrecks.append([d, v])
		if not Vehicles.is_wreck(v):
			var sp := absf(float(v.get("speed", 0.0)))
			if sp > DUST_SPEED:
				moving.append([d, v])
			if sp > 1.0 or f & Vehicles.MOVING:
				engines.append([d, v])
	var by_d := func(a: Array, b: Array) -> bool: return a[0] < b[0]
	for list in [burning, wrecks, moving, engines]:
		list.sort_custom(by_d)
	# Fires: flames and dark smoke on the engine deck (the rear).
	for i in MAX_FIRES:
		var rig: Dictionary = _fires[i]
		var on := i < burning.size()
		if on:
			var v: Dictionary = burning[i][1]
			var vt: Dictionary = vtypes.get(String(v.type), {})
			var root: Node3D = rig.root
			root.global_position = _rear_deck(v, vt)
			var light: OmniLight3D = rig.light
			light.light_energy = 2.2 + 0.8 * sin(Time.get_ticks_msec() * 0.023 + i)
		_set_emitting(rig.flames, on)
		_set_emitting(rig.smoke, on)
		(rig.light as OmniLight3D).visible = on
	counts.fires = mini(burning.size(), MAX_FIRES)
	# Smoke over fresh wrecks.
	for i in MAX_SMOKES:
		var p := _smokes[i]
		var on := i < wrecks.size()
		if on:
			var v: Dictionary = wrecks[i][1]
			p.global_position = v.pos + Vector3(0, 2.0, 0)
		_set_emitting(p, on)
	counts.smokes = mini(wrecks.size(), MAX_SMOKES)
	# Dust behind moving vehicles.
	for i in MAX_DUST:
		var p := _dust[i]
		var on := i < moving.size()
		if on:
			var v: Dictionary = moving[i][1]
			var vt: Dictionary = vtypes.get(String(v.type), {})
			var back := Vector3(sin(v.yaw), 0.0, cos(v.yaw)) * (float(vt.get("hull_length", 5.8)) * 0.5 * signf(float(v.speed)))
			p.global_position = v.pos + back + Vector3(0, 0.3, 0)
		_set_emitting(p, on)
	counts.dust = mini(moving.size(), MAX_DUST)
	_update_engines(engines)


## Keeps each audible vehicle on its voice slot; frees slots of vehicles
## that dropped out of the nearest ENGINE_VOICES.
func _update_engines(engines: Array) -> void:
	var near := {}
	for i in mini(engines.size(), ENGINE_VOICES):
		near[int(engines[i][1].id)] = engines[i][1]
	for k in ENGINE_VOICES:
		if not near.has(_engine_slots[k]):
			_engine_slots[k] = -1
	for id in near:
		if not _engine_slots.has(id):
			_engine_slots[_engine_slots.find(-1)] = id
	var pts := PackedVector3Array()
	var speeds := PackedFloat32Array()
	pts.resize(ENGINE_VOICES)
	speeds.resize(ENGINE_VOICES)
	var n := 0
	for k in ENGINE_VOICES:
		var id := _engine_slots[k]
		if id == -1:
			speeds[k] = -1.0
			continue
		pts[k] = near[id].pos + Vector3(0, 1.2, 0)
		speeds[k] = float(near[id].speed)
		n += 1
	counts.engines = n
	AudioManager.set_engine_loops(pts, speeds)


func _rear_deck(v: Dictionary, vt: Dictionary) -> Vector3:
	var back := Vector3(sin(v.yaw), 0.0, cos(v.yaw)) * float(vt.get("hull_length", 5.8)) * 0.3
	return v.pos + back + Vector3(0, float(vt.get("hull_ground", 0.4)) + float(vt.get("hull_height", 1.36)), 0)


static func _set_emitting(p: GPUParticles3D, on: bool) -> void:
	if p.emitting != on:
		p.emitting = on


# --- rigs -----------------------------------------------------------------------

func _make_flash() -> Dictionary:
	var root := Node3D.new()
	add_child(root)
	var light := OmniLight3D.new()
	light.light_color = Color(1.0, 0.7, 0.35)
	light.omni_range = 16.0
	light.shadow_enabled = false
	light.visible = false
	root.add_child(light)
	var ball := _additive_ball(Color(1.0, 0.8, 0.45))
	add_child(ball)
	var dust := _one_shot(18, 1.6, _ring_dust_material(), _puff_mesh(Color(0.55, 0.48, 0.36, 0.7)))
	add_child(dust)
	return {"root": root, "light": light, "ball": ball, "dust": dust, "t": -1.0, "scale": 1.0}


func _make_sparks() -> Dictionary:
	var sparks := _one_shot(26, 0.55, _spark_material(), _spark_mesh())
	sparks.explosiveness = 1.0
	add_child(sparks)
	var puff := _one_shot(4, 1.2, _smoke_material(0.4, 0.6), _puff_mesh(Color(0.6, 0.58, 0.55, 0.45)))
	add_child(puff)
	return {"sparks": sparks, "puff": puff}


func _make_pen() -> Dictionary:
	var root := Node3D.new()
	add_child(root)
	var light := OmniLight3D.new()
	light.light_color = Color(1.0, 0.85, 0.6)
	light.omni_range = 12.0
	light.visible = false
	root.add_child(light)
	var ball := _additive_ball(Color(1.0, 0.9, 0.7))
	root.add_child(ball)
	var sparks := _one_shot(14, 0.4, _spark_material(), _spark_mesh())
	root.add_child(sparks)
	return {"root": root, "light": light, "ball": ball, "sparks": sparks, "t": -1.0}


func _make_cook() -> Dictionary:
	var root := Node3D.new()
	add_child(root)
	var light := OmniLight3D.new()
	light.light_color = Color(1.0, 0.6, 0.28)
	light.omni_range = 40.0
	light.visible = false
	root.add_child(light)
	var ball := _additive_ball(Color(1.0, 0.62, 0.25))
	root.add_child(ball)
	var debris := _one_shot(30, 2.2, _debris_material(), _box_mesh(0.3, Color(0.18, 0.16, 0.14)))
	var jet := _one_shot(40, 2.5, _flame_material(9.0), _flame_mesh())
	jet.explosiveness = 0.3
	var smoke := _one_shot(24, 18.0, _smoke_material(2.0, 3.5), _puff_mesh(Color(0.12, 0.11, 0.1, 0.6)))
	smoke.explosiveness = 0.2
	for p in [debris, jet, smoke]:
		root.add_child(p)
	return {"root": root, "light": light, "ball": ball, "debris": debris, "jet": jet, "smoke": smoke, "t": -1.0}


func _make_fire() -> Dictionary:
	var root := Node3D.new()
	add_child(root)
	var flames := _continuous(26, 0.8, _flame_material(2.2), _flame_mesh(), 0.0)
	var smoke := _continuous(18, 6.0, _smoke_material(1.2, 1.8), _puff_mesh(Color(0.1, 0.09, 0.09, 0.6)), 0.0)
	flames.reparent(root)
	smoke.reparent(root)
	smoke.position = Vector3(0, 0.8, 0)
	var light := OmniLight3D.new()
	light.light_color = Color(1.0, 0.55, 0.22)
	light.omni_range = 9.0
	light.visible = false
	light.position = Vector3(0, 1.0, 0)
	root.add_child(light)
	return {"root": root, "flames": flames, "smoke": smoke, "light": light}


func _make_streaks() -> MultiMeshInstance3D:
	var mat := StandardMaterial3D.new()
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mat.blend_mode = BaseMaterial3D.BLEND_MODE_ADD
	mat.vertex_color_use_as_albedo = true
	mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	mat.cull_mode = BaseMaterial3D.CULL_DISABLED
	mat.disable_receive_shadows = true
	var box := BoxMesh.new()
	box.size = Vector3.ONE
	var arrays := box.get_mesh_arrays()
	var verts: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	for i in verts.size():
		verts[i] += Vector3(0, 0, 0.5)
	arrays[Mesh.ARRAY_VERTEX] = verts
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	mesh.surface_set_material(0, mat)
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	mm.mesh = mesh
	mm.instance_count = MAX_STREAKS
	mm.visible_instance_count = 0
	var inst := MultiMeshInstance3D.new()
	inst.name = "ShellTracers"
	inst.multimesh = mm
	inst.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	inst.custom_aabb = AABB(Vector3(-2048, -256, -2048), Vector3(4096, 1024, 4096))
	add_child(inst)
	_streak_buf.resize(MAX_STREAKS * STREAK_STRIDE)
	return inst


func _one_shot(amount: int, life: float, mat: ParticleProcessMaterial, mesh: Mesh) -> GPUParticles3D:
	var p := GPUParticles3D.new()
	p.amount = amount
	p.lifetime = life
	p.one_shot = true
	p.explosiveness = 0.9
	p.emitting = false
	p.local_coords = false
	p.process_material = mat
	p.draw_pass_1 = mesh
	p.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	p.visibility_aabb = AABB(Vector3(-30, -6, -30), Vector3(60, 60, 60))
	return p


func _continuous(amount: int, life: float, mat: ParticleProcessMaterial, mesh: Mesh, _x: float) -> GPUParticles3D:
	var p := GPUParticles3D.new()
	p.amount = amount
	p.lifetime = life
	p.emitting = false
	p.local_coords = false
	p.process_material = mat
	p.draw_pass_1 = mesh
	p.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	p.visibility_aabb = AABB(Vector3(-30, -6, -30), Vector3(60, 60, 60))
	add_child(p)
	return p


func _additive_ball(col: Color) -> MeshInstance3D:
	var ball := MeshInstance3D.new()
	var sphere := SphereMesh.new()
	sphere.radius = 0.5
	sphere.height = 1.0
	sphere.radial_segments = 10
	sphere.rings = 5
	ball.mesh = sphere
	var m := StandardMaterial3D.new()
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	m.blend_mode = BaseMaterial3D.BLEND_MODE_ADD
	m.albedo_color = col
	m.disable_receive_shadows = true
	ball.material_override = m
	ball.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	ball.visible = false
	return ball


static func _basis_along(d: Vector3) -> Basis:
	var z := -d
	var helper := Vector3.UP if absf(d.y) < 0.95 else Vector3.RIGHT
	var x := helper.cross(z).normalized()
	return Basis(x, z.cross(x), z)


# --- materials and meshes ---------------------------------------------------------

func _ramp(c: Color) -> GradientTexture1D:
	var g := Gradient.new()
	g.set_color(0, c)
	g.set_color(1, Color(c, 0.0))
	var t := GradientTexture1D.new()
	t.gradient = g
	return t


func _curve(a: float, b: float) -> CurveTexture:
	var c := Curve.new()
	c.add_point(Vector2(0, a))
	c.add_point(Vector2(1, b))
	var t := CurveTexture.new()
	t.curve = c
	return t


func _ring_dust_material() -> ParticleProcessMaterial:
	var m := ParticleProcessMaterial.new()
	m.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_RING
	m.emission_ring_axis = Vector3.UP
	m.emission_ring_radius = 1.5
	m.emission_ring_inner_radius = 0.5
	m.emission_ring_height = 0.2
	m.direction = Vector3(0, 0.3, 0)
	m.spread = 180.0
	m.flatness = 0.8
	m.initial_velocity_min = 3.0
	m.initial_velocity_max = 7.0
	m.damping_min = 3.0
	m.damping_max = 5.0
	m.gravity = Vector3(0, 0.2, 0)
	m.scale_min = 1.5
	m.scale_max = 2.8
	m.scale_curve = _curve(0.6, 1.6)
	m.color_ramp = _ramp(Color(1, 1, 1, 1))
	return m


func _dust_material() -> ParticleProcessMaterial:
	var m := ParticleProcessMaterial.new()
	m.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_BOX
	m.emission_box_extents = Vector3(1.4, 0.1, 0.4)
	m.direction = Vector3.UP
	m.spread = 50.0
	m.initial_velocity_min = 0.5
	m.initial_velocity_max = 1.6
	m.gravity = Vector3(0, 0.15, 0)
	m.scale_min = 1.4
	m.scale_max = 2.4
	m.scale_curve = _curve(0.5, 1.8)
	m.color_ramp = _ramp(Color(1, 1, 1, 1))
	return m


func _smoke_material(s0: float, s1: float) -> ParticleProcessMaterial:
	var m := ParticleProcessMaterial.new()
	m.direction = Vector3.UP
	m.spread = 14.0
	m.initial_velocity_min = 1.2
	m.initial_velocity_max = 2.4
	m.gravity = Vector3(0.3, 0.35, 0.1)
	m.damping_min = 0.2
	m.damping_max = 0.5
	m.scale_min = s0
	m.scale_max = s1
	m.scale_curve = _curve(0.5, 2.8)
	m.color_ramp = _ramp(Color(1, 1, 1, 0.9))
	m.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_SPHERE
	m.emission_sphere_radius = 0.6
	return m


func _flame_material(speed: float) -> ParticleProcessMaterial:
	var m := ParticleProcessMaterial.new()
	m.direction = Vector3.UP
	m.spread = 12.0
	m.initial_velocity_min = speed * 0.5
	m.initial_velocity_max = speed
	m.gravity = Vector3(0, 1.0, 0)
	m.damping_min = 1.0
	m.damping_max = 2.0
	m.scale_min = 0.6
	m.scale_max = 1.2
	m.scale_curve = _curve(1.0, 0.2)
	var g := Gradient.new()
	g.set_color(0, Color(1.0, 0.9, 0.5, 1.0))
	g.set_color(1, Color(0.8, 0.2, 0.05, 0.0))
	g.add_point(0.4, Color(1.0, 0.5, 0.15, 0.9))
	var t := GradientTexture1D.new()
	t.gradient = g
	m.color_ramp = t
	m.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_BOX
	m.emission_box_extents = Vector3(0.8, 0.1, 0.6)
	return m


func _spark_material() -> ParticleProcessMaterial:
	var m := ParticleProcessMaterial.new()
	m.direction = Vector3.UP
	m.spread = 80.0
	m.initial_velocity_min = 6.0
	m.initial_velocity_max = 15.0
	m.gravity = Vector3(0, -9.8, 0)
	m.scale_min = 0.6
	m.scale_max = 1.2
	m.color_ramp = _ramp(Color(1.0, 0.85, 0.5, 1.0))
	return m


func _debris_material() -> ParticleProcessMaterial:
	var m := ParticleProcessMaterial.new()
	m.direction = Vector3.UP
	m.spread = 45.0
	m.initial_velocity_min = 8.0
	m.initial_velocity_max = 18.0
	m.gravity = Vector3(0, -9.8, 0)
	m.angular_velocity_min = -360.0
	m.angular_velocity_max = 360.0
	m.scale_min = 0.5
	m.scale_max = 1.5
	return m


func _puff_mesh(col: Color) -> QuadMesh:
	var q := QuadMesh.new()
	q.size = Vector2.ONE
	var mat := StandardMaterial3D.new()
	mat.billboard_mode = BaseMaterial3D.BILLBOARD_PARTICLES
	mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	mat.vertex_color_use_as_albedo = true
	mat.albedo_color = col
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mat.albedo_texture = _disc
	mat.cull_mode = BaseMaterial3D.CULL_DISABLED
	q.material = mat
	return q


func _flame_mesh() -> QuadMesh:
	var q := QuadMesh.new()
	q.size = Vector2(0.9, 1.3)
	var mat := StandardMaterial3D.new()
	mat.billboard_mode = BaseMaterial3D.BILLBOARD_PARTICLES
	mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	mat.blend_mode = BaseMaterial3D.BLEND_MODE_ADD
	mat.vertex_color_use_as_albedo = true
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mat.albedo_texture = _disc
	mat.cull_mode = BaseMaterial3D.CULL_DISABLED
	q.material = mat
	return q


func _spark_mesh() -> QuadMesh:
	var q := QuadMesh.new()
	q.size = Vector2(0.12, 0.12)
	var mat := StandardMaterial3D.new()
	mat.billboard_mode = BaseMaterial3D.BILLBOARD_PARTICLES
	mat.blend_mode = BaseMaterial3D.BLEND_MODE_ADD
	mat.vertex_color_use_as_albedo = true
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	q.material = mat
	return q


func _box_mesh(size: float, col: Color) -> BoxMesh:
	var b := BoxMesh.new()
	b.size = Vector3(size, size * 0.6, size)
	var mat := StandardMaterial3D.new()
	mat.albedo_color = col
	mat.roughness = 1.0
	b.material = mat
	return b


static func _soft_disc() -> GradientTexture2D:
	var g := Gradient.new()
	g.set_color(0, Color(1, 1, 1, 1))
	g.set_color(1, Color(1, 1, 1, 0))
	var t := GradientTexture2D.new()
	t.gradient = g
	t.fill = GradientTexture2D.FILL_RADIAL
	t.fill_from = Vector2(0.5, 0.5)
	t.fill_to = Vector2(1.0, 0.5)
	t.width = 64
	t.height = 64
	return t
