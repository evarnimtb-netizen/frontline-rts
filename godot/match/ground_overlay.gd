extends MeshInstance3D
## World-space overlay lines drawn on the ground from the match state:
## capture point rings, selection rings, order markers, the dig tool's
## ghost trench and the mortar range ring while the barrage tool is armed.
##
## One ImmediateMesh rebuilt per frame (a few hundred vertices; §4.1 keeps
## units out of Nodes, and this is a single node). Lines follow the ground
## through MatchView.ground_y and sit slightly above it.

const LIFT := 0.35
const TRENCH_HALF_W := 1.2
const MARKER_TIME := 0.9

var _im := ImmediateMesh.new()
var _view: MatchView
var _markers: Array[Dictionary] = []
var _flash: PackedVector3Array = PackedVector3Array()
var _flash_t := 0.0
var _t := 0.0


func _ready() -> void:
	mesh = _im
	cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	var mat := StandardMaterial3D.new()
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mat.vertex_color_use_as_albedo = true
	mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	mat.cull_mode = BaseMaterial3D.CULL_DISABLED
	mat.no_depth_test = false
	mat.render_priority = 1
	material_override = mat
	# Never culled: the overlay spans the whole map.
	custom_aabb = AABB(Vector3(-4096, -512, -4096), Vector3(8192, 1024, 8192))


## Short-lived marker at an order target. kind: move, attack_move, barrage, dig.
func marker(pos: Vector3, kind: StringName) -> void:
	_markers.append({"pos": pos, "kind": kind, "t": 0.0})


## Brief highlight of a confirmed dig polyline.
func flash_polyline(points: PackedVector3Array) -> void:
	_flash = points.duplicate()
	_flash_t = 0.8


func update_state(state: Dictionary, view: MatchView) -> void:
	_view = view
	var dt := get_process_delta_time()
	_t += dt
	_im.clear_surfaces()
	_im.surface_begin(Mesh.PRIMITIVE_TRIANGLES)
	_im.surface_set_color(Color.WHITE)
	_im.surface_add_vertex(Vector3.ZERO)  # keeps the surface non-empty
	_im.surface_add_vertex(Vector3.ZERO)
	_im.surface_add_vertex(Vector3.ZERO)
	var local := view.local_player
	for p in state.get("points", []):
		var col := UiPalette.owner_color(p.get("owner", -1), local)
		var r: float = p.get("radius", 15.0)
		_ring(p.pos, r, 0.5, Color(col, 0.75))
		var prog: float = p.get("progress", 0.0)
		if prog > 0.0 and int(p.get("capturing", -1)) >= 0:
			var cc := UiPalette.owner_color(p.capturing, local)
			_arc(p.pos, r - 1.2, 0.9, 0.0, TAU * prog, Color(cc, 0.9))
	# Trench work orders in progress: outline plus the dug share.
	for sq in state.get("squads", []):
		if sq.has("dig_line") and int(sq.owner) == local:
			# The dug trench itself is terrain; mark the work line and the
			# share done with thin lines along its edges.
			var line: PackedVector3Array = sq.dig_line
			_edges(line, Color(UiPalette.TEXT, 0.35), 0.06)
			_edges(_polyline_head(line, float(sq.get("dig_progress", 0.0))), Color(UiPalette.ACCENT, 0.85), 0.1)
	var sel: Node = view.selection
	for id in sel.selected:
		var sq: Dictionary = view.squads_by_id.get(id, {})
		if not sq.is_empty():
			_ring(sq.pos, 5.0, 0.35, Color(UiPalette.TEXT, 0.85))
	if sel.hover_id != -1 and not sel.is_selected(sel.hover_id):
		var hs: Dictionary = view.squads_by_id.get(sel.hover_id, {})
		if not hs.is_empty():
			var hc := UiPalette.owner_color(hs.owner, local)
			_ring(hs.pos, 5.0, 0.25, Color(hc, 0.6))
	if sel.tool == &"barrage":
		var hi := float(view.rules().get("mortar_max_m", 600.0))
		var lo := float(view.rules().get("mortar_min_m", 60.0))
		for id in sel.selected:
			var sq: Dictionary = view.squads_by_id.get(id, {})
			if not sq.is_empty() and sq.type in MatchView.MORTARS:
				_ring(sq.pos, hi, 1.5, Color(UiPalette.WARN, 0.45), 128)
				_ring(sq.pos, lo, 0.8, Color(UiPalette.DANGER, 0.5), 48)
	if sel.tool == &"dig":
		_draw_dig_ghost(sel)
	if _flash_t > 0.0:
		_flash_t -= dt
		_trench(_flash, Color(UiPalette.OK, clampf(_flash_t, 0.0, 0.8)))
	for m in _markers:
		m.t += dt
		_draw_marker(m)
	_markers = _markers.filter(func(m: Dictionary) -> bool: return m.t < MARKER_TIME)
	_im.surface_end()


func _draw_dig_ghost(sel: Node) -> void:
	var pts: PackedVector3Array = sel.dig_points.duplicate()
	var mouse: Vector2 = sel.cursor()
	var cand: Variant = sel.dig_candidate(mouse)
	var full: bool = sel.dig_length() >= sel.dig_max() - 0.5
	if cand != null and not full:
		pts.append(cand)
	var pulse := 0.55 + 0.2 * sin(_t * 6.0)
	if pts.size() >= 2:
		var col := Color(UiPalette.ACCENT, pulse)
		if pts.size() == 2 and pts[0].distance_to(pts[1]) < 2.0:
			col = Color(UiPalette.DANGER, pulse)
		_trench(pts, col)
		# Parapet hint on the far side is the sim's call; mark the axis only.
		for i in range(1, pts.size()):
			_line(pts[i - 1], pts[i], 0.12, Color(UiPalette.TEXT, 0.8))
	for p in sel.dig_points:
		_ring(p, 0.9, 0.3, Color(UiPalette.TEXT, 0.95), 12)
	if cand != null:
		_ring(cand, 0.7, 0.25, Color(UiPalette.ACCENT, 0.9), 12)


func _draw_marker(m: Dictionary) -> void:
	var k: float = m.t / MARKER_TIME
	var a := 1.0 - k
	match m.kind:
		&"attack_move":
			_ring(m.pos, 2.0 + 4.0 * k, 0.5, Color(UiPalette.DANGER, a))
		&"barrage":
			var c := Color(UiPalette.WARN, a)
			_ring(m.pos, 8.0, 0.6, c, 40)
			_line(m.pos + Vector3(-6, 0, -6), m.pos + Vector3(6, 0, 6), 0.4, c)
			_line(m.pos + Vector3(-6, 0, 6), m.pos + Vector3(6, 0, -6), 0.4, c)
		_:
			_ring(m.pos, 1.5 + 3.0 * k, 0.4, Color(UiPalette.OK, a))


# --- primitives (triangles, following the ground) --------------------------

func _gp(x: float, z: float) -> Vector3:
	return Vector3(x, _view.ground_y(x, z) + LIFT, z)


func _quad(a: Vector3, b: Vector3, c: Vector3, d: Vector3, col: Color) -> void:
	_im.surface_set_color(col)
	_im.surface_add_vertex(a)
	_im.surface_add_vertex(b)
	_im.surface_add_vertex(c)
	_im.surface_add_vertex(a)
	_im.surface_add_vertex(c)
	_im.surface_add_vertex(d)


func _arc(center: Vector3, r: float, w: float, a0: float, a1: float, col: Color, segs := 48) -> void:
	var n := maxi(int(segs * (a1 - a0) / TAU), 3)
	for i in n:
		var t0 := a0 + (a1 - a0) * i / n
		var t1 := a0 + (a1 - a0) * (i + 1) / n
		var d0 := Vector2(sin(t0), -cos(t0))
		var d1 := Vector2(sin(t1), -cos(t1))
		var i0 := Vector2(center.x, center.z) + d0 * (r - w * 0.5)
		var o0 := Vector2(center.x, center.z) + d0 * (r + w * 0.5)
		var i1 := Vector2(center.x, center.z) + d1 * (r - w * 0.5)
		var o1 := Vector2(center.x, center.z) + d1 * (r + w * 0.5)
		_quad(_gp(i0.x, i0.y), _gp(o0.x, o0.y), _gp(o1.x, o1.y), _gp(i1.x, i1.y), col)


func _ring(center: Vector3, r: float, w: float, col: Color, segs := 48) -> void:
	_arc(center, r, w, 0.0, TAU, col, segs)


func _line(a: Vector3, b: Vector3, half_w: float, col: Color) -> void:
	var d := Vector2(b.x - a.x, b.z - a.z)
	var seg_len := d.length()
	if seg_len < 0.01:
		return
	var n := Vector2(-d.y, d.x) / seg_len * half_w
	var steps := maxi(int(seg_len / 2.0), 1)
	for i in steps:
		var p0 := Vector2(a.x, a.z) + d * (float(i) / steps)
		var p1 := Vector2(a.x, a.z) + d * (float(i + 1) / steps)
		_quad(_gp(p0.x - n.x, p0.y - n.y), _gp(p0.x + n.x, p0.y + n.y), _gp(p1.x + n.x, p1.y + n.y), _gp(p1.x - n.x, p1.y - n.y), col)


## The first `frac` (0..1) of a polyline by length.
func _polyline_head(pts: PackedVector3Array, frac: float) -> PackedVector3Array:
	var total := 0.0
	for i in range(1, pts.size()):
		total += pts[i - 1].distance_to(pts[i])
	var want := total * clampf(frac, 0.0, 1.0)
	var out := PackedVector3Array()
	if pts.is_empty() or want <= 0.0:
		return out
	out.append(pts[0])
	for i in range(1, pts.size()):
		var seg := pts[i - 1].distance_to(pts[i])
		if want <= seg:
			out.append(pts[i - 1].lerp(pts[i], want / maxf(seg, 0.001)))
			return out
		want -= seg
		out.append(pts[i])
	return out


## Two lines along the trench edges (TRENCH_HALF_W + 0.5 m out).
func _edges(pts: PackedVector3Array, col: Color, half_w: float) -> void:
	for i in range(1, pts.size()):
		var a := pts[i - 1]
		var b := pts[i]
		var d := Vector2(b.x - a.x, b.z - a.z)
		if d.length() < 0.01:
			continue
		var n := Vector2(-d.y, d.x).normalized() * (TRENCH_HALF_W + 0.5)
		for k in [-1.0, 1.0]:
			var o := Vector3(n.x * k, 0, n.y * k)
			_line(a + o, b + o, half_w, col)


func _trench(pts: PackedVector3Array, col: Color) -> void:
	for i in range(1, pts.size()):
		_line(pts[i - 1], pts[i], TRENCH_HALF_W, col)
