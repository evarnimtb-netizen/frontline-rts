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
##   charges a thin olive pipe from the carrier's spot toward the target;
##           S10b: a demolition charge (`piece` >= 0) is a satchel box at
##           its spot until it bursts.
## S10b (match_state.md "S10b"):
##   mines   small flat discs, AP grey, AT dark green, only when the mine's
##           owner is the local player or its `known` flag is set (the
##           bridge already drops the others; design 7.6);
##   hedgehog three crossed steel bars in the piece's box;
##   logs    brown boxes (timber pieces, as drawn above);
##   bunker  timber walls, an earth-brown roof and a dark slit box under
##           the roof's front edge;
##   sweep   an own `clear_mines` squad's lane as a dashed line with a
##           cross at `clear_along`.
## One MeshInstance3D per box, pooled; pipes and discs in their own pools.

const SANDBAG := Color(0.62, 0.55, 0.38)
const TIMBER := Color(0.40, 0.29, 0.17)
const POST := Color(0.30, 0.24, 0.17)
const WIRE := Color(0.35, 0.35, 0.33)
const GHOST := Color(0.85, 0.85, 0.80)
const WAIT := Color(0.95, 0.70, 0.20)
const PIPE := Color(0.33, 0.36, 0.22)
const STEEL := Color(0.30, 0.30, 0.32)
const EARTH := Color(0.36, 0.27, 0.18)
const SLIT := Color(0.06, 0.06, 0.06)
const SATCHEL := Color(0.42, 0.40, 0.26)
const MINE_AP := Color(0.55, 0.55, 0.55)
const MINE_AT := Color(0.18, 0.30, 0.16)
const LANE := Color(0.95, 0.85, 0.30)
## Mine discs (m): radius and height, AP and AT.
const MINE_R := {"ap": 0.10, "at": 0.16}
const MINE_H := 0.05
## Bunker slit (m): height and width; sweep lane dashes (m).
const SLIT_SIZE := Vector2(0.4, 0.79)
const DASH_M := 1.0
const RING_TICKS := 12
const RING_R := 2.2
## Ghost box of a site step (m): along the facing, up, across.
const GHOST_SIZE := Vector3(2.0, 0.5, 0.8)

var _pool: Array[MeshInstance3D] = []
var _pipes: Array[MeshInstance3D] = []
var _discs: Array[MeshInstance3D] = []
var _mats := {}
## Boxes and pipes drawn last frame (tests).
var drawn := 0
var drawn_pipes := 0
## S10b: mine discs drawn last frame (tests).
var drawn_discs := 0


func update_state(s: Dictionary) -> void:
	var f: Dictionary = s.get("fort", {})
	var local := int(s.get("local_player", 0))
	var n := 0
	for q in f.get("pieces", []):
		var size: Vector3 = q.size
		var part: String = q.get("part", "sandbag")
		var bp: String = q.get("blueprint", "")
		var base := TIMBER if part != "sandbag" else SANDBAG
		if part == "steel":
			base = STEEL
		elif part == "roof" and bp == "bunker":
			base = EARTH
		var hp := clampf(float(q.get("hp", 1.0)), 0.0, 1.0)
		var col := _tint(base, int(q.owner), local).darkened(0.45 * (1.0 - hp))
		var yaw := float(q.get("yaw", 0.0))
		if part == "steel":
			# S10b hedgehog: three crossed bars filling the box.
			var bar := Vector3(maxf(size.x, size.z) * 1.1, 0.12, 0.12)
			for k in 3:
				var b := _box(n, q.center, bar, yaw + PI * float(k) / 3.0, col, 1.0)
				b.rotation.z = 0.6 if k != 1 else -0.6
				n += 1
			continue
		_box(n, q.center, size, yaw, col, 1.0)
		n += 1
		if part == "roof" and bp == "bunker":
			# The slit: under the roof's front edge (+x along the facing).
			var fwd := Vector3(cos(yaw), 0, -sin(yaw))
			var at: Vector3 = q.center + fwd * (size.x * 0.5) - Vector3(0, size.y * 0.5 + SLIT_SIZE.x * 0.5, 0)
			_box(n, at, Vector3(0.06, SLIT_SIZE.x, SLIT_SIZE.y), yaw, SLIT, 1.0)
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
		if int(c.get("piece", -1)) >= 0:
			# S10b: a demolition satchel at its spot.
			_box(n, c.to + Vector3(0, 0.12, 0), Vector3(0.35, 0.24, 0.25), 0.0, SATCHEL, 1.0)
			n += 1
			continue
		_pipe(m, c.from + Vector3(0, 0.08, 0), c.to + Vector3(0, 0.08, 0))
		m += 1
	# S10b: known mines.
	var dn := 0
	for mine in f.get("mines", []):
		if int(mine.get("owner", -1)) != local and not bool(mine.get("known", false)):
			continue
		var at_kind: bool = mine.get("kind", "ap") == "at"
		_disc(dn, mine.pos + Vector3(0, MINE_H * 0.5, 0), MINE_R["at" if at_kind else "ap"], MINE_AT if at_kind else MINE_AP)
		dn += 1
	# S10b: own sweep lanes, dashed, with the sweepers' progress mark.
	for sq in s.get("squads", []):
		if int(sq.get("owner", -1)) != local or not sq.has("clear_lane"):
			continue
		var lane: PackedVector3Array = sq.clear_lane
		if lane.size() < 2:
			continue
		var a := lane[0]
		var d := lane[1] - lane[0]
		var length := Vector2(d.x, d.z).length()
		if length < 0.1:
			continue
		var yaw := atan2(-d.z, d.x)
		var dashes := int(ceil(length / (2.0 * DASH_M)))
		for k in dashes:
			var t0 := (2.0 * DASH_M * k) / length
			var t1 := minf((2.0 * DASH_M * k + DASH_M) / length, 1.0)
			var mid := a + d * ((t0 + t1) * 0.5) + Vector3(0, 0.05, 0)
			_box(n, mid, Vector3((t1 - t0) * length, 0.04, 0.12), yaw, LANE, 0.8)
			n += 1
		var along := clampf(float(sq.get("clear_along", 0.0)) / length, 0.0, 1.0)
		_box(n, a + d * along + Vector3(0, 0.06, 0), Vector3(0.12, 0.05, 1.2), yaw, LANE, 1.0)
		n += 1
	for k in range(n, _pool.size()):
		_pool[k].visible = false
	for k in range(m, _pipes.size()):
		_pipes[k].visible = false
	for k in range(dn, _discs.size()):
		_discs[k].visible = false
	drawn = n
	drawn_pipes = m
	drawn_discs = dn


func _tint(base: Color, owner: int, local: int) -> Color:
	return base.lerp(UiPalette.owner_color(owner, local), 0.15)


func _box(k: int, p: Vector3, size: Vector3, yaw: float, col: Color, alpha: float) -> MeshInstance3D:
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
	return mi


func _disc(k: int, p: Vector3, r: float, col: Color) -> void:
	while _discs.size() <= k:
		var mi := MeshInstance3D.new()
		var cyl := CylinderMesh.new()
		cyl.height = MINE_H
		cyl.radial_segments = 12
		mi.mesh = cyl
		add_child(mi)
		_discs.append(mi)
	var mi := _discs[k]
	var cyl := mi.mesh as CylinderMesh
	cyl.top_radius = r
	cyl.bottom_radius = r
	mi.position = p
	mi.material_override = _mat(col, 1.0)
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
