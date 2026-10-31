extends Node3D
## A few cheap billboard smoke emitters (GPUParticles3D) around the battle.
## Unshaded, alpha-blended quads with a procedural radial texture: no assets.

@export var sim_path: NodePath = ^"../FrontlineSim"
@export var emitters := 6
@export var ring_radius := 150.0
@export var particles_per_emitter := 48


func _ready() -> void:
	var sim := get_node_or_null(sim_path)
	var quad := QuadMesh.new()
	quad.size = Vector2(9.0, 9.0)
	quad.material = _billboard_material()
	var pm := _process_material()
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	for i in emitters:
		var a := TAU * (float(i) + 0.5 * rng.randf()) / float(emitters)
		var r := ring_radius * (0.3 + 0.7 * rng.randf())
		var p := Vector3(cos(a) * r, 0.0, sin(a) * r)
		if sim != null and sim.has_method("get_height_at"):
			p.y = sim.get_height_at(p.x, p.z)
		var e := GPUParticles3D.new()
		# Assign the process material before touching any other emitter
		# property: otherwise Godot 4.7.2 reports a leaked particles shader
		# at exit.
		e.process_material = pm
		e.name = "SmokeEmitter%d" % i
		e.amount = particles_per_emitter
		e.lifetime = 14.0
		e.preprocess = 14.0
		e.draw_pass_1 = quad
		e.position = p
		e.visibility_aabb = AABB(Vector3(-50, -5, -50), Vector3(100, 70, 100))
		e.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		add_child(e)


func _process_material() -> ParticleProcessMaterial:
	var pm := ParticleProcessMaterial.new()
	pm.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_SPHERE
	pm.emission_sphere_radius = 5.0
	pm.direction = Vector3(0.2, 1.0, 0.0)
	pm.spread = 30.0
	pm.initial_velocity_min = 0.5
	pm.initial_velocity_max = 1.5
	pm.gravity = Vector3(0.5, 0.3, 0.1)
	pm.angle_min = -180.0
	pm.angle_max = 180.0
	pm.scale_min = 0.6
	pm.scale_max = 1.2
	var curve := Curve.new()
	curve.max_value = 3.0
	curve.add_point(Vector2(0.0, 0.5))
	curve.add_point(Vector2(1.0, 2.5))
	var curve_tex := CurveTexture.new()
	curve_tex.curve = curve
	pm.scale_curve = curve_tex
	var g := Gradient.new()
	g.set_color(0, Color(0.55, 0.55, 0.53, 0.0))
	g.set_color(1, Color(0.62, 0.62, 0.60, 0.0))
	g.add_point(0.15, Color(0.50, 0.50, 0.48, 0.55))
	g.add_point(0.6, Color(0.58, 0.58, 0.56, 0.35))
	var ramp := GradientTexture1D.new()
	ramp.gradient = g
	pm.color_ramp = ramp
	return pm


func _billboard_material() -> StandardMaterial3D:
	var g := Gradient.new()
	g.set_color(0, Color(1, 1, 1, 1))
	g.set_color(1, Color(1, 1, 1, 0))
	var tex := GradientTexture2D.new()
	tex.gradient = g
	tex.fill = GradientTexture2D.FILL_RADIAL
	tex.fill_from = Vector2(0.5, 0.5)
	tex.fill_to = Vector2(1.0, 0.5)
	tex.width = 64
	tex.height = 64
	var m := StandardMaterial3D.new()
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	m.billboard_mode = BaseMaterial3D.BILLBOARD_PARTICLES
	m.vertex_color_use_as_albedo = true
	m.albedo_texture = tex
	return m
