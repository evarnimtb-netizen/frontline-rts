extends Node3D
## S10a renderer: the state's `fort` (godot/docs/match_state.md "S10a").
## Blockouts only (no selection: S6-18 stays open):
##   pieces  plain boxes, sandbag tan, roof and timber brown, in the side
##           colour's tint; a damaged piece darkens toward its hp;
##   wire    a post at each end of every 2 m section and a translucent
##           grey band (the tangle) between them, as high as the wire;
##   sites   a faint ghost box on the current step's spot and a ring of
##           twelve ticks around it, lit up to the step's progress (amber
##           while the site waits for materials);
##   charges a thin olive pipe from the carrier's spot toward the target.
## One MeshInstance3D per box, pooled; pipes in their own pool.

const SANDBAG := Color(0.62, 0.55, 0.38)
const TIMBER := Color(0.40, 0.29, 0.17)
const POST := Color(0.30, 0.24, 0.17)
const WIRE := Color(0.35, 0.35, 0.33)
const GHOST := Color(0.85, 0.85, 0.80)
const WAIT := Color(0.95, 0.70, 0.20)
const PIPE := Color(0.33, 0.36, 0.22)
const RING_TICKS := 12
const RING_R := 2.2
## Ghost box of a site step (m): along the facing, up, across.
const GHOST_SIZE := Vector3(2.0, 0.5, 0.8)

var _pool: Array[MeshInstance3D] = []
var _pipes: Array[MeshInstance3D] = []
var _mats := {}
## Boxes and pipes drawn last frame (tests).
var drawn := 0
var drawn_pipes := 0


func update_state(s: Dictionary) -> void:
	var f: Dictionary = s.get("fort", {})
	var local := int(s.get("local_player", 0))
	var n := 0
	for q in f.get("pieces", []):
		var size: Vector3 = q.size
		var base := TIMBER if q.get("part", "sandbag") != "sandbag" else SANDBAG
		var hp := clampf(float(q.get("hp", 1.0)), 0.0, 1.0)
		var col := _tint(base, int(q.owner), local).darkened(0.45 * (1.0 - hp))
		_box(n, q.center, size, float(q.get("yaw", 0.0)), col, 1.0)
		n += 1
	for w in f.get("wire", []):
		var a: Vector3 = w.a
		var b: Vector3 = w.b
		var h := float(w.get("height_m", 0.9))
		var band := float(w.get("band_m", 1.5))
		for p in [a, b]:
			_box(n, p + Vector3(0, h * 0.5, 0), Vector3(0.08, h, 0.08), 0.0, POST, 1.0)
			n += 1
		var d := b - a
		var flat := Vector2(d.x, -d.z)
		var yaw := atan2(flat.y, flat.x)
		var mid := (a + b) * 0.5 + Vector3(0, h * 0.5, 0)
		_box(n, mid, Vector3(maxf(flat.length(), 0.1), h * 0.8, band * 0.5), yaw, _tint(WIRE, int(w.owner), local), 0.35)
		n += 1
	for site in f.get("sites", []):
		var p: Vector3 = site.pos
		var yaw := float(site.get("yaw", 0.0))
		var waiting: bool = site.get("step", "") == "materials"
		_box(n, p + Vector3(0, GHOST_SIZE.y * 0.5, 0), GHOST_SIZE, yaw, GHOST, 0.25)
		n += 1
		var lit := int(round(clampf(float(site.get("progress", 0.0)), 0.0, 1.0) * RING_TICKS))
		for k in RING_TICKS:
			var ang := TAU * float(k) / RING_TICKS
			var at := p + Vector3(cos(ang) * RING_R, 0.08, -sin(ang) * RING_R)
			var on := k < lit
			var col := WAIT if waiting else UiPalette.owner_color(int(site.owner), local)
			_box(n, at, Vector3(0.5, 0.06, 0.12), ang + PI * 0.5, col, 0.9 if on else 0.2)
			n += 1
	var m := 0
	for c in f.get("charges", []):
		_pipe(m, c.from + Vector3(0, 0.08, 0), c.to + Vector3(0, 0.08, 0))
		m += 1
	for k in range(n, _pool.size()):
		_pool[k].visible = false
	for k in range(m, _pipes.size()):
		_pipes[k].visible = false
	drawn = n
	drawn_pipes = m


func _tint(base: Color, owner: int, local: int) -> Color:
	return base.lerp(UiPalette.owner_color(owner, local), 0.15)


func _box(k: int, p: Vector3, size: Vector3, yaw: float, col: Color, alpha: float) -> void:
	while _pool.size() <= k:
		var mi := MeshInstance3D.new()
		mi.mesh = BoxMesh.new()
		add_child(mi)
		_pool.append(mi)
	var mi := _pool[k]
	(mi.mesh as BoxMesh).size = size
	mi.position = p
	# Sim yaw from +x counter-clockwise seen from above; Godot's ground is
	# x right, -z the sim's +y, so the same angle about +y.
	mi.rotation = Vector3(0, yaw, 0)
	mi.material_override = _mat(col, alpha)
	mi.visible = true


func _pipe(k: int, a: Vector3, b: Vector3) -> void:
	while _pipes.size() <= k:
		var mi := MeshInstance3D.new()
		var cyl := CylinderMesh.new()
		cyl.top_radius = 0.035
		cyl.bottom_radius = 0.035
		cyl.radial_segments = 8
		mi.mesh = cyl
		add_child(mi)
		_pipes.append(mi)
	var mi := _pipes[k]
	var d := b - a
	var length := maxf(d.length(), 0.05)
	(mi.mesh as CylinderMesh).height = length
	mi.position = (a + b) * 0.5
	# The cylinder's axis is +y: turn it onto the pipe.
	var axis := d / length
	var up := Vector3.UP
	var rot_axis := up.cross(axis)
	if rot_axis.length() < 1e-4:
		mi.basis = Basis()
	else:
		mi.basis = Basis(rot_axis.normalized(), up.angle_to(axis))
	mi.material_override = _mat(PIPE, 1.0)
	mi.visible = true


func _mat(col: Color, alpha: float) -> StandardMaterial3D:
	var key := "%s/%.2f" % [col.to_html(), alpha]
	if _mats.has(key):
		return _mats[key]
	var m := StandardMaterial3D.new()
	m.albedo_color = Color(col, alpha)
	m.roughness = 0.9
	if alpha < 1.0:
		m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	_mats[key] = m
	return m
