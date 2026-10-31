extends MeshInstance3D
## World-space overlay lines drawn on the ground from the match state:
## capture point rings, order markers, detached men's own order targets
## (a ring at the target, and a dashed line from the man while selected),
## the dig tool's ghost trench, the mortar range ring while the barrage
## tool is armed, and the throw tool's range rings and target.
##
## One ImmediateMesh rebuilt per frame (a few hundred vertices; §4.1 keeps
## units out of Nodes, and this is a single node). Lines follow the ground
## through MatchView.ground_y and sit slightly above it. The rings under
## selected and hovered soldiers are a separate MultiMesh batch
## (SelectionRings, a child), one buffer upload per frame.

const LIFT := 0.35
const TRENCH_HALF_W := 1.2
const MARKER_TIME := 0.9

var _im := ImmediateMesh.new()
var _view: MatchView
var _markers: Array[Dictionary] = []
var _flash: PackedVector3Array = PackedVector3Array()
var _flash_t := 0.0
var _t := 0.0
var _rings: SelectionRings


func _ready() -> void:
	_rings = SelectionRings.new()
	_rings.name = "SelectionRings"
	add_child(_rings)
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


## Short-lived marker at an order target. kind: move, attack_move, barrage,
## dig, throw.
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
	_update_rings(view, sel)
	_draw_detached(view, sel)
	if sel.tool == &"barrage":
		var hi := float(view.rules().get("mortar_max_m", 600.0))
		var lo := float(view.rules().get("mortar_min_m", 60.0))
		for sid in sel.selected_squads():
			var sq: Dictionary = view.squads_by_id.get(sid, {})
			if not sq.is_empty() and sq.type in MatchView.MORTARS:
				_ring(sq.pos, hi, 1.5, Color(UiPalette.WARN, 0.45), 128)
				_ring(sq.pos, lo, 0.8, Color(UiPalette.DANGER, 0.5), 48)
	if sel.tool == &"dig":
		_draw_dig_ghost(sel)
	elif sel.tool == &"throw":
		_draw_throw_tool(view, sel)
	if _flash_t > 0.0:
		_flash_t -= dt
		_trench(_flash, Color(UiPalette.OK, clampf(_flash_t, 0.0, 0.8)))
	for m in _markers:
		m.t += dt
		_draw_marker(m)
	_markers = _markers.filter(func(m: Dictionary) -> bool: return m.t < MARKER_TIME)
	_im.surface_end()


## One ring per selected soldier (white) and the hovered one (side colour).
func _update_rings(view: MatchView, sel: Node) -> void:
	var pts := PackedVector3Array()
	var cols := PackedColorArray()
	for id in sel.selected:
		var so: Dictionary = view.soldiers_by_id.get(id, {})
		if so.is_empty():
			continue
		pts.append(so.pos)
		cols.append(Color(UiPalette.TEXT, 0.9))
	var h: int = sel.hover_id
	if h != -1 and not sel.is_selected(h) and view.soldiers_by_id.has(h):
		var hs: Dictionary = view.soldiers_by_id[h]
		pts.append(hs.pos)
		cols.append(Color(UiPalette.owner_color(int(hs.owner), view.local_player), 0.8))
	_rings.set_rings(pts, cols)


## Own detached men with an order target: a small ring at the target; a
## dashed line from a selected man to his target.
func _draw_detached(view: MatchView, sel: Node) -> void:
	for id in view.soldiers_by_id:
		var so: Dictionary = view.soldiers_by_id[id]
		if not so.get("detached", false) or not so.has("target") or int(so.owner) != view.local_player:
			continue
		var t: Vector3 = so.target
		var picked: bool = sel.is_selected(id)
		var col := Color(HudStyle.ACCENT, 0.95 if picked else 0.5)
		_ring(t, 0.9, 0.18, col, 16)
		if picked:
			_dashed(so.pos, t, 0.07, col)


## Range ring of the man who would throw (the eligible one nearest the
## cursor) with his minimum range; the other eligible men get faint rings
## (they all throw with Shift). Then the blast or cloud radius at the
## target, accent in range and red out of it.
func _draw_throw_tool(view: MatchView, sel: Node) -> void:
	var item: String = sel.throw_item
	var rr := view.throw_range(item)
	var g: Variant = view.ground_pick(sel.cursor())
	var men: PackedInt32Array = sel.throwers(item)
	var lead := -1
	var best := INF
	if g != null:
		for id in men:
			var p: Vector3 = view.soldiers_by_id[id].pos
			var d := Vector2(p.x, p.z).distance_to(Vector2(g.x, g.z))
			if d < best:
				best = d
				lead = id
	for id in men:
		var p: Vector3 = view.soldiers_by_id[id].pos
		if id == lead:
			_ring(p, rr.y, 0.35, Color(HudStyle.ACCENT, 0.6), 64)
			if rr.x > 0.5:
				_ring(p, rr.x, 0.2, Color(HudStyle.DANGER, 0.5), 24)
		else:
			_ring(p, rr.y, 0.2, Color(HudStyle.ACCENT, 0.14), 48)
	if g == null:
		return
	var ok: bool = sel.throw_in_range(item, g)
	var col := Color(HudStyle.ACCENT if ok else HudStyle.DANGER, 0.85)
	var fx: float = MatchView.THROW_EFFECT_M.get(item, 6.0)
	_ring(g, fx, 0.25, col, 40)
	_ring(g, 0.8, 0.25, col, 12)


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
		&"throw":
			_ring(m.pos, 1.0 + 2.0 * k, 0.4, Color(HudStyle.ACCENT, a), 24)
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


func _dashed(a: Vector3, b: Vector3, half_w: float, col: Color, dash := 1.2, gap := 0.9) -> void:
	var total := Vector2(b.x - a.x, b.z - a.z).length()
	if total < 0.01:
		return
	var t := 0.0
	while t < total:
		var t1 := minf(t + dash, total)
		_line(a.lerp(b, t / total), a.lerp(b, t1 / total), half_w, col)
		t += dash + gap


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
