extends Node3D
## Presenter scenery and effects around the FrontlineMatch node (S1
## "World"): woods, capture point flags, and explosions (light flash,
## fireball, flying earth, dust, a lingering smoke column). Units, rounds,
## muzzle flashes and the terrain are drawn by the Rust bridge.
##
## Everything here is cosmetic and non-deterministic (§12): it reads the
## match state and get_map_info() and never feeds the sim. Explosions come
## from state.effects (kind "explosion") and reuse a fixed pool of effect
## rigs, oldest first, so a barrage never creates nodes.

const POOL := 14
const FLAG_POLE_M := 8.0
const TREES_PER_M2 := 1.0 / 45.0

var _flags: Array[Dictionary] = []
var _pool: Array[Dictionary] = []
var _next := 0
var _t := 0.0
var _local := 0


## Builds the static scenery from FrontlineMatch.get_map_info().
func setup(info: Dictionary, height_fn: Callable) -> void:
	_local = int(info.get("local_player", 0))
	_build_trees(info.get("woods", []), height_fn)
	for p in info.get("points", []):
		_flags.append(_make_flag(p.pos))
	for i in POOL:
		_pool.append(_make_explosion())


func update_state(state: Dictionary) -> void:
	var dt := get_process_delta_time()
	_t += dt
	var pts: Array = state.get("points", [])
	for i in mini(pts.size(), _flags.size()):
		_update_flag(_flags[i], pts[i], dt)
	for fx in state.get("effects", []):
		if fx.get("kind", "") == "explosion":
			_explode(fx.pos)
	for e in _pool:
		if e.t >= 0.0:
			e.t += dt
			var k: float = e.t
			var light: OmniLight3D = e.light
			light.light_energy = 14.0 * maxf(1.0 - k / 0.28, 0.0)
			light.visible = k < 0.3
			var ball: MeshInstance3D = e.ball
			ball.visible = k < 0.2
			var s := 0.8 + 2.6 * clampf(k / 0.1, 0.0, 1.0)
			ball.scale = Vector3(s, s * 0.9, s)
			(ball.material_override as StandardMaterial3D).albedo_color = Color(1.0, 0.7, 0.35) * clampf(1.0 - k / 0.2, 0.0, 1.0)
			if k > 12.0:
				e.t = -1.0


# --- explosions ---------------------------------------------------------------

func _explode(pos: Vector3) -> void:
	var e: Dictionary = _pool[_next]
	_next = (_next + 1) % _pool.size()
	var root: Node3D = e.root
	root.global_position = pos
	e.t = 0.0
	for p in [e.debris, e.dust, e.smoke]:
		var gp: GPUParticles3D = p
		gp.restart()
		gp.emitting = true


func _make_explosion() -> Dictionary:
	var root := Node3D.new()
	add_child(root)
	var light := OmniLight3D.new()
	light.light_color = Color(1.0, 0.62, 0.3)
	light.omni_range = 28.0
	light.light_energy = 0.0
	light.shadow_enabled = false
	light.position = Vector3(0, 2.0, 0)
	light.visible = false
	root.add_child(light)
	var ball := MeshInstance3D.new()
	var sphere := SphereMesh.new()
	sphere.radius = 1.0
	sphere.height = 2.0
	sphere.radial_segments = 12
	sphere.rings = 6
	ball.mesh = sphere
	var bm := StandardMaterial3D.new()
	bm.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	bm.blend_mode = BaseMaterial3D.BLEND_MODE_ADD
	bm.albedo_color = Color(1.0, 0.7, 0.35)
	bm.disable_receive_shadows = true
	ball.material_override = bm
	ball.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	ball.position = Vector3(0, 0.8, 0)
	ball.visible = false
	root.add_child(ball)
	var debris := _particles(28, 1.7, _debris_material(), _box_mesh(0.22, Color(0.22, 0.17, 0.11)))
	var dust := _particles(30, 3.0, _dust_material(), _puff_mesh(Color(0.47, 0.39, 0.29, 0.85), false))
	var smoke := _particles(16, 10.0, _smoke_material(), _puff_mesh(Color(0.36, 0.35, 0.34, 0.55), false))
	smoke.explosiveness = 0.6
	for p in [debris, dust, smoke]:
		root.add_child(p)
	return {"root": root, "light": light, "ball": ball, "debris": debris, "dust": dust, "smoke": smoke, "t": -1.0}


func _particles(amount: int, life: float, mat: ParticleProcessMaterial, mesh: Mesh) -> GPUParticles3D:
	var p := GPUParticles3D.new()
	p.amount = amount
	p.lifetime = life
	p.one_shot = true
	p.explosiveness = 0.92
	p.emitting = false
	p.local_coords = false
	p.process_material = mat
	p.draw_pass_1 = mesh
	p.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	p.visibility_aabb = AABB(Vector3(-30, -4, -30), Vector3(60, 50, 60))
	return p


func _fade_ramp(c: Color) -> GradientTexture1D:
	var g := Gradient.new()
	g.set_color(0, c)
	g.set_color(1, Color(c, 0.0))
	g.add_point(0.25, c)
	var t := GradientTexture1D.new()
	t.gradient = g
	return t


func _grow_curve(a: float, b: float) -> CurveTexture:
	var c := Curve.new()
	c.add_point(Vector2(0, a))
	c.add_point(Vector2(1, b))
	var t := CurveTexture.new()
	t.curve = c
	return t


func _debris_material() -> ParticleProcessMaterial:
	var m := ParticleProcessMaterial.new()
	m.direction = Vector3.UP
	m.spread = 38.0
	m.initial_velocity_min = 7.0
	m.initial_velocity_max = 15.0
	m.gravity = Vector3(0, -9.8, 0)
	m.angular_velocity_min = -360.0
	m.angular_velocity_max = 360.0
	m.scale_min = 0.5
	m.scale_max = 1.4
	m.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_SPHERE
	m.emission_sphere_radius = 0.8
	return m


func _dust_material() -> ParticleProcessMaterial:
	var m := ParticleProcessMaterial.new()
	m.direction = Vector3.UP
	m.spread = 75.0
	m.initial_velocity_min = 4.0
	m.initial_velocity_max = 11.0
	m.gravity = Vector3(0, -2.5, 0)
	m.damping_min = 2.0
	m.damping_max = 3.5
	m.scale_min = 2.5
	m.scale_max = 4.5
	m.scale_curve = _grow_curve(0.6, 1.6)
	m.color_ramp = _fade_ramp(Color(0.5, 0.42, 0.31, 0.8))
	m.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_SPHERE
	m.emission_sphere_radius = 1.2
	return m


func _smoke_material() -> ParticleProcessMaterial:
	var m := ParticleProcessMaterial.new()
	m.direction = Vector3.UP
	m.spread = 25.0
	m.initial_velocity_min = 1.0
	m.initial_velocity_max = 3.0
	m.gravity = Vector3(0.35, 0.45, 0.1)
	m.damping_min = 0.4
	m.damping_max = 0.8
	m.scale_min = 2.5
	m.scale_max = 4.0
	m.scale_curve = _grow_curve(0.5, 2.6)
	m.color_ramp = _fade_ramp(Color(0.33, 0.32, 0.31, 0.5))
	m.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_SPHERE
	m.emission_sphere_radius = 1.5
	return m


func _puff_mesh(col: Color, lit: bool) -> QuadMesh:
	var q := QuadMesh.new()
	q.size = Vector2(1.0, 1.0)
	var mat := StandardMaterial3D.new()
	mat.billboard_mode = BaseMaterial3D.BILLBOARD_PARTICLES
	mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	mat.vertex_color_use_as_albedo = true
	mat.albedo_color = Color(1, 1, 1, col.a)
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_PER_PIXEL if lit else BaseMaterial3D.SHADING_MODE_UNSHADED
	mat.albedo_texture = _soft_disc()
	mat.cull_mode = BaseMaterial3D.CULL_DISABLED
	q.material = mat
	return q


func _box_mesh(size: float, col: Color) -> BoxMesh:
	var b := BoxMesh.new()
	b.size = Vector3(size, size * 0.7, size)
	var mat := StandardMaterial3D.new()
	mat.albedo_color = col
	mat.roughness = 1.0
	b.material = mat
	return b


static var _disc: GradientTexture2D


func _soft_disc() -> GradientTexture2D:
	if _disc == null:
		var g := Gradient.new()
		g.set_color(0, Color(1, 1, 1, 1))
		g.set_color(1, Color(1, 1, 1, 0))
		_disc = GradientTexture2D.new()
		_disc.gradient = g
		_disc.fill = GradientTexture2D.FILL_RADIAL
		_disc.fill_from = Vector2(0.5, 0.5)
		_disc.fill_to = Vector2(1.0, 0.5)
		_disc.width = 64
		_disc.height = 64
	return _disc


# --- flags ----------------------------------------------------------------------

func _make_flag(pos: Vector3) -> Dictionary:
	var root := Node3D.new()
	add_child(root)
	root.global_position = pos
	var pole := MeshInstance3D.new()
	var cyl := CylinderMesh.new()
	cyl.top_radius = 0.06
	cyl.bottom_radius = 0.08
	cyl.height = FLAG_POLE_M
	cyl.radial_segments = 6
	pole.mesh = cyl
	var pm := StandardMaterial3D.new()
	pm.albedo_color = Color(0.32, 0.27, 0.2)
	pole.material_override = pm
	pole.position = Vector3(0, FLAG_POLE_M * 0.5, 0)
	root.add_child(pole)
	var cloth := MeshInstance3D.new()
	var box := BoxMesh.new()
	box.size = Vector3(2.2, 1.3, 0.04)
	cloth.mesh = box
	var cm := StandardMaterial3D.new()
	cm.albedo_color = UiPalette.NEUTRAL
	cm.roughness = 0.9
	cloth.material_override = cm
	root.add_child(cloth)
	return {"root": root, "cloth": cloth, "mat": cm, "raise": 1.0, "phase": randf() * TAU}


func _update_flag(f: Dictionary, p: Dictionary, dt: float) -> void:
	var owner := int(p.get("owner", -1))
	var capturing := int(p.get("capturing", -1))
	var prog := float(p.get("progress", 0.0))
	# Owned: full height, lowered as an enemy takes it. Neutral: raised by
	# whoever is capturing.
	var side := owner
	var want := 1.0
	if owner >= 0 and capturing >= 0 and capturing != owner:
		want = 1.0 - prog
	elif owner < 0:
		side = capturing
		want = prog if capturing >= 0 else 0.0
	f.raise = move_toward(f.raise, want, dt * 0.8)
	var col := UiPalette.owner_color(side, _local)
	var mat: StandardMaterial3D = f.mat
	mat.albedo_color = col
	var cloth: MeshInstance3D = f.cloth
	var h := 1.2 + (FLAG_POLE_M - 2.0) * float(f.raise)
	var sway := sin(_t * 2.1 + float(f.phase)) * 0.18
	cloth.position = Vector3(1.15, h, 0)
	cloth.rotation = Vector3(0, sway, 0)


# --- woods ----------------------------------------------------------------------

func _build_trees(woods: Array, height_fn: Callable) -> void:
	var rng := RandomNumberGenerator.new()
	rng.seed = 1944
	var xf: Array[Transform3D] = []
	for w in woods:
		var c: Vector3 = w.center
		var r: float = w.radius
		var count := int(PI * r * r * TREES_PER_M2)
		for k in count:
			var a := rng.randf() * TAU
			var d := sqrt(rng.randf()) * r
			var x := c.x + cos(a) * d
			var z := c.z + sin(a) * d
			var s := rng.randf_range(0.75, 1.35)
			var y: float = height_fn.call(x, z)
			var b := Basis(Vector3.UP, rng.randf() * TAU).scaled(Vector3(s, s * rng.randf_range(0.9, 1.25), s))
			xf.append(Transform3D(b, Vector3(x, y, z)))
	if xf.is_empty():
		return
	var crown := CylinderMesh.new()
	crown.top_radius = 0.0
	crown.bottom_radius = 2.4
	crown.height = 7.5
	crown.radial_segments = 7
	crown.rings = 2
	var cm := StandardMaterial3D.new()
	cm.albedo_color = Color(0.16, 0.24, 0.14)
	cm.roughness = 1.0
	crown.material = cm
	var trunk := CylinderMesh.new()
	trunk.top_radius = 0.16
	trunk.bottom_radius = 0.22
	trunk.height = 3.0
	trunk.radial_segments = 5
	trunk.rings = 1
	var tm := StandardMaterial3D.new()
	tm.albedo_color = Color(0.27, 0.2, 0.14)
	trunk.material = tm
	for part in [[crown, Vector3(0, 6.0, 0)], [trunk, Vector3(0, 1.5, 0)]]:
		var mm := MultiMesh.new()
		mm.transform_format = MultiMesh.TRANSFORM_3D
		mm.mesh = part[0]
		mm.instance_count = xf.size()
		for i in xf.size():
			var t: Transform3D = xf[i]
			mm.set_instance_transform(i, Transform3D(t.basis, t.origin + t.basis * part[1]))
		var inst := MultiMeshInstance3D.new()
		inst.multimesh = mm
		add_child(inst)
