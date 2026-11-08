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
##
## S3: rings under selected and hovered vehicles, stretched to the hull
## (a second SelectionRings batch); escort links (a dashed line from each
## own escorting squad or man to its vehicle); the face tool's arrows.
##
## S5b: while the fire-mission tool is armed, the expected-miss ring at the
## cursor (state.fire_preview: solid for observed and direct fire, dashed
## for map fire, red when refused), the would-be observer's link line and
## the own knowledge map (state.intel) as faint squares; for a selected own
## mortar on a mission (or with the tool armed) the observer link line from
## the gunner to the observer of the last round and on to the target (none
## for direct fire; dotted when the observer is gone).

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
var _veh_rings: SelectionRings


func _ready() -> void:
	_rings = SelectionRings.new()
	_rings.name = "SelectionRings"
	add_child(_rings)
	_veh_rings = SelectionRings.new()
	_veh_rings.name = "VehicleRings"
	_veh_rings.inner = 0.9
	_veh_rings.outer = 1.0
	add_child(_veh_rings)
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
	_update_vehicle_rings(view, sel)
	_draw_detached(view, sel)
	_draw_escorts(view, state)
	_draw_suppress_lines(view, sel, state)
	if sel.tool == &"face":
		_draw_face_tool(view, sel)
	elif sel.tool == &"suppress":
		_draw_suppress_tool(view, sel)
	if sel.tool == &"barrage":
		var hi := float(view.rules().get("mortar_max_m", 600.0))
		var lo := float(view.rules().get("mortar_min_m", 60.0))
		for sid in sel.selected_squads():
			var sq: Dictionary = view.squads_by_id.get(sid, {})
			if not sq.is_empty() and sq.type in MatchView.MORTARS:
				_ring(sq.pos, hi, 1.5, Color(UiPalette.WARN, 0.45), 128)
				_ring(sq.pos, lo, 0.8, Color(UiPalette.DANGER, 0.5), 48)
		_draw_intel(state)
		_draw_fire_preview(state)
	_draw_mission_links(view, sel)
	_draw_supply(view, sel, state)
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
	for p in view.pending_draw:
		_draw_pending(p)
	_im.surface_end()


## S6: supply radius rings of own HQs and dumps while the HQ squad, the
## bearers or a mortar is selected or a supply tool is armed (a dug-in
## structure's ring solid, one still digging dashed), crate feed rings
## (5 m) round set-down own crates while a mortar is selected, and the
## placement ghost (the def's radius) under the cursor of a build tool.
func _draw_supply(view: MatchView, sel: Node, state: Dictionary) -> void:
	var show: bool = sel.tool in [&"build_hq", &"build_dump", &"place_crate", &"pick_up"]
	# S8b: the selected trucks' reach: vehicles (solid), men (dashed).
	for id in sel.truck_vehicles():
		var tv: Dictionary = view.vehicles_by_id.get(id, {})
		var vt: Dictionary = view.rules().get("vehicle_types", {}).get(String(tv.get("type", "")), {})
		if vt.has("supply_vehicle_radius_m"):
			_ring(tv.pos, float(vt.supply_vehicle_radius_m), 0.3, Color(HudStyle.ACCENT, 0.7), 64)
			var men_r := float(vt.get("supply_radius_m", 8.0))
			for k in 24:
				if k % 2 == 0:
					_arc(tv.pos, men_r, 0.25, TAU * k / 24.0, TAU * (k + 1) / 24.0, Color(HudStyle.ACCENT, 0.5), 3)
	# S9: a selected gun's arc: two rays to its optic range.
	for id in sel.gun_vehicles():
		var gv: Dictionary = view.vehicles_by_id.get(id, {})
		var gt: Dictionary = view.rules().get("vehicle_types", {}).get(String(gv.get("type", "")), {})
		if gt.has("arc_deg") and String(gv.get("gun_state", "")) == "emplaced":
			var reach := float(gt.get("optic_m", 400.0))
			for deg in [int(gt.arc_deg.lo), int(gt.arc_deg.hi)]:
				var a: float = float(gv.yaw) + deg_to_rad(float(deg))
				var ray := Vector3(-sin(a), 0.0, -cos(a))
				_line(gv.pos + ray * 2.0, gv.pos + ray * reach, 0.18, Color(HudStyle.ACCENT, 0.55))
	var mortar := false
	for sid in sel.selected_squads():
		var sq: Dictionary = view.squads_by_id.get(sid, {})
		var tags: PackedStringArray = sq.get("tags", PackedStringArray())
		if tags.has("hq") or tags.has("bearers"):
			show = true
		if sq.get("type", "") in MatchView.MORTARS:
			show = true
			mortar = true
	if not show:
		return
	var local := view.local_player
	for st in state.get("structures", []):
		if int(st.owner) != local:
			continue
		var r := float(st.get("radius", 20.0))
		var col := Color(HudStyle.ACCENT, 0.7)
		if bool(st.get("built", false)):
			_ring(st.pos, r, 0.35, col, 96)
		else:
			var segs := 32
			for k in segs:
				if k % 2 == 0:
					_arc(st.pos, r, 0.3, TAU * k / segs, TAU * (k + 1) / segs, Color(col, 0.5), 3)
	if mortar:
		var feed := float(view.rules().get("supply", {}).get("crate_feed_m", 5.0))
		for c in state.get("crates", []):
			if int(c.owner) == local and int(c.get("carrier", -1)) == -1 and c.kind == "shells":
				_ring(c.pos, feed, 0.15, Color(UiPalette.OK, 0.75), 32)
	if sel.tool == &"build_hq" or sel.tool == &"build_dump":
		var g: Variant = view.ground_pick(sel.cursor())
		if g != null:
			var r := 25.0 if sel.tool == &"build_hq" else 15.0
			_ring(g, r, 0.4, Color(UiPalette.TEXT, 0.5), 96)
			_ring(g, 1.5, 0.3, Color(UiPalette.TEXT, 0.8), 24)


## S5b: the own knowledge map while the fire-mission tool is armed: a
## faint square per cell (trench cells warmer), fading with age.
func _draw_intel(state: Dictionary) -> void:
	for c in state.get("intel", []):
		var age := float(c.get("age_s", 0.0))
		var a := clampf(0.55 - age / 600.0, 0.2, 0.55)
		var col := Color(UiPalette.WARN, a) if c.get("trench", false) else Color(UiPalette.DANGER, a * 0.8)
		var p: Vector3 = c.pos
		var h := 11.0
		var w := 0.3
		_line(Vector3(p.x - h, p.y, p.z - h), Vector3(p.x + h, p.y, p.z - h), w, col)
		_line(Vector3(p.x + h, p.y, p.z - h), Vector3(p.x + h, p.y, p.z + h), w, col)
		_line(Vector3(p.x + h, p.y, p.z + h), Vector3(p.x - h, p.y, p.z + h), w, col)
		_line(Vector3(p.x - h, p.y, p.z + h), Vector3(p.x - h, p.y, p.z - h), w, col)


## S5b: the fire-mission tool's expected-miss ring at the cursor point and
## the would-be observer's link (gunner → observer → point).
func _draw_fire_preview(state: Dictionary) -> void:
	var fp: Dictionary = state.get("fire_preview", {})
	if fp.is_empty():
		return
	var at: Vector3 = fp.point
	var r := maxf(float(fp.get("ring_m", 10.0)), 2.0)
	if not fp.get("ok", false):
		_ring(at, 3.0, 0.5, Color(UiPalette.DANGER, 0.9), 24)
		_line(at + Vector3(-2.2, 0, -2.2), at + Vector3(2.2, 0, 2.2), 0.25, Color(UiPalette.DANGER, 0.9))
		_line(at + Vector3(-2.2, 0, 2.2), at + Vector3(2.2, 0, -2.2), 0.25, Color(UiPalette.DANGER, 0.9))
		return
	var mode := String(fp.get("mode", "map"))
	if mode == "map":
		var n := 24
		for i in n:
			var a0 := TAU * i / n
			_arc(at, r, 0.9, a0, a0 + TAU / n * 0.55, Color(UiPalette.WARN, 0.95), 4)
	else:
		_ring(at, r, 0.9, Color(UiPalette.OK, 0.95), 64)
	_ring(at, 0.6, 0.4, Color(UiPalette.TEXT, 0.9), 10)
	if mode == "observed" and fp.has("observer_pos"):
		var o: Vector3 = fp.observer_pos
		var px := _m_per_px(o)
		_dashed(fp.gunner_pos, o, maxf(0.12, 1.5 * px), Color(HudStyle.ACCENT, 0.9), maxf(1.5, 10.0 * px), maxf(1.0, 6.0 * px))
		_dashed(o, at, maxf(0.1, 1.0 * px), Color(HudStyle.ACCENT, 0.7), maxf(1.0, 8.0 * px), maxf(1.4, 7.0 * px))


## S5b: observer link lines of selected own mortars on a mission:
## gunner → observer (solid; dotted once the observer is gone) and
## observer → target (thin). Direct fire draws none.
func _draw_mission_links(view: MatchView, sel: Node) -> void:
	for sid in sel.selected_squads():
		var sq: Dictionary = view.squads_by_id.get(sid, {})
		var m: Dictionary = sq.get("mission", {})
		if m.is_empty() or String(m.get("mode", "")) == "direct" or not m.has("gunner_pos"):
			continue
		var g: Vector3 = m.gunner_pos
		var col := Color(HudStyle.ACCENT, 0.95)
		# At least 3 px wide (2 px for the thin legs) with a dark outline:
		# 0.1 m lines were a broken 1 px trace from the 330 m camera
		# (review L4).
		var px := _m_per_px(g)
		if m.has("observer_pos"):
			var o: Vector3 = m.observer_pos
			var w := maxf(0.1, 1.5 * px)
			_line(g, o, w + px, Color(0.0, 0.0, 0.0, 0.4))
			_line(g, o, w, col)
			if m.has("target"):
				_dashed(o, m.target, maxf(0.08, 1.0 * px), Color(col, 0.75), maxf(1.0, 8.0 * px), maxf(1.4, 7.0 * px))
		elif m.has("target"):
			# The link is broken (map fire, or the observer fell).
			_dashed(g, m.target, maxf(0.08, 1.0 * px), Color(UiPalette.DANGER, 0.85), maxf(0.4, 3.0 * px), maxf(1.6, 8.0 * px))


## S5b (review L4): metres per screen pixel at `at`, for lines that must
## keep a minimum on-screen width. 0 without a camera (headless tests).
func _m_per_px(at: Vector3) -> float:
	if not is_inside_tree():
		return 0.0
	var cam := get_viewport().get_camera_3d()
	var h := get_viewport().get_visible_rect().size.y
	if cam == null or h <= 0.0:
		return 0.0
	if cam.projection == Camera3D.PROJECTION_ORTHOGONAL:
		return cam.size / h
	var dist := cam.global_position.distance_to(at)
	return 2.0 * dist * tan(deg_to_rad(cam.fov) * 0.5) / h


## Net mode (S2): an order sent but not executed yet. A ring of dashes
## turning around the target, in the order's colour, until the sim runs
## the order (MatchView.pending_draw); it pops in a little at click time.
func _draw_pending(p: Dictionary) -> void:
	var col: Color
	var r := 3.2
	match p.kind:
		"attack_move":
			col = UiPalette.DANGER
		"barrage":
			col = UiPalette.WARN
			r = 8.0
		"throw":
			col = HudStyle.ACCENT
			r = 2.0
		_:
			col = UiPalette.TEXT
	var grow := clampf(float(p.get("age", 1.0)) / 0.12, 0.0, 1.0)
	r *= 0.6 + 0.4 * grow
	var n := 8
	var spin := _t * 2.4
	for i in n:
		var a0 := spin + TAU * i / n
		_arc(p.pos, r, 0.45, a0, a0 + TAU / n * 0.55, Color(col, 0.95), 6)
	_ring(p.pos, 0.45, 0.5, Color(col, 0.9), 10)


## One ring per selected soldier (white), the hovered one (side colour),
## and a faint accent ring under own detached men who are not selected, so
## the men following their own orders stand out from the formations.
func _update_rings(view: MatchView, sel: Node) -> void:
	var pts := PackedVector3Array()
	var cols := PackedColorArray()
	for id in sel.selected:
		var so: Dictionary = view.soldiers_by_id.get(id, {})
		if so.is_empty():
			continue
		pts.append(so.pos)
		cols.append(Color(UiPalette.TEXT, 0.9))
	for id in view.soldiers_by_id:
		var so: Dictionary = view.soldiers_by_id[id]
		if so.get("detached", false) and int(so.owner) == view.local_player and not sel.is_selected(id):
			pts.append(so.pos)
			cols.append(Color(HudStyle.ACCENT, 0.55))
	var h: int = sel.hover_id
	if h != -1 and not sel.is_selected(h) and view.soldiers_by_id.has(h):
		var hs: Dictionary = view.soldiers_by_id[h]
		pts.append(hs.pos)
		cols.append(Color(UiPalette.owner_color(int(hs.owner), view.local_player), 0.8))
	_rings.set_rings(pts, cols)


## Rings under selected own vehicles (white) and the hovered vehicle (side
## colour), stretched to the hull plus a margin and turned with it.
func _update_vehicle_rings(view: MatchView, sel: Node) -> void:
	var pts := PackedVector3Array()
	var yaws := PackedFloat32Array()
	var half := PackedVector2Array()
	var cols := PackedColorArray()
	var ids: PackedInt32Array = sel.selected_vehicles()
	var h: int = sel.hover_id
	if h != -1 and view.is_vehicle(h) and not sel.is_selected(h) and not Vehicles.is_wreck(view.vehicles_by_id[h]):
		ids.append(h)
	for id in ids:
		var v: Dictionary = view.vehicles_by_id[id]
		var vt := view.vehicle_type(id)
		pts.append(v.pos)
		yaws.append(float(v.yaw))
		half.append(Vector2(float(vt.get("hull_width", 2.8)) * 0.5 + 0.9, float(vt.get("hull_length", 5.8)) * 0.5 + 0.9))
		var own := int(v.owner) == view.local_player
		cols.append(Color(UiPalette.TEXT, 0.9) if sel.is_selected(id) else Color(UiPalette.owner_color(int(v.owner), view.local_player), 0.8 if own else 0.9))
	_veh_rings.set_oriented(pts, yaws, half, cols)


## Escort links: a dashed line from each own escorting squad (its anchor)
## or detached man to the vehicle he follows (`follow`, opt), and the
## vehicle's escort count is on its world marker.
func _draw_escorts(view: MatchView, state: Dictionary) -> void:
	var col := Color(HudStyle.OWN, 0.75)
	for sq in state.get("squads", []):
		var vid := int(sq.get("follow", -1))
		if vid != -1 and int(sq.owner) == view.local_player and view.vehicles_by_id.has(vid):
			_dashed(sq.pos, view.vehicles_by_id[vid].pos, 0.09, col, 1.6, 1.0)
	for id in view.soldiers_by_id:
		var so: Dictionary = view.soldiers_by_id[id]
		var vid := int(so.get("follow", -1))
		if vid != -1 and int(so.owner) == view.local_player and view.vehicles_by_id.has(vid):
			_dashed(so.pos, view.vehicles_by_id[vid].pos, 0.05, Color(col, 0.5), 1.0, 1.0)


## S3b: the suppress lines of selected own squads and vehicles (dashed
## accent, with the line's ends ringed).
func _draw_suppress_lines(view: MatchView, sel: Node, state: Dictionary) -> void:
	var col := Color(HudStyle.ACCENT, 0.7)
	var lines: Array = []
	for sq in state.get("squads", []):
		if sq.has("suppress_line") and int(sq.owner) == view.local_player and not sel.selected_in(int(sq.id)).is_empty():
			lines.append(sq.suppress_line)
	for id in sel.selected_vehicles():
		var v: Dictionary = view.vehicles_by_id.get(id, {})
		if v.has("suppress_line"):
			lines.append(v.suppress_line)
	for l in lines:
		var line: PackedVector3Array = l
		if line.size() >= 2 and line[0].distance_to(line[1]) > 0.5:
			_dashed(line[0], line[1], 0.12, col, 1.4, 0.9)
		for p in line:
			_ring(p, 1.0, 0.18, col, 16)


## S3b suppress tool (P): a ring at the cursor; while the button is held,
## the pending line from where it went down, capped at the longest line.
func _draw_suppress_tool(view: MatchView, sel: Node) -> void:
	var col := Color(HudStyle.ACCENT, 0.9)
	var g: Variant = view.ground_pick(sel.cursor())
	if sel.suppress_from == null:
		if g != null:
			_ring(g, 1.2, 0.22, col, 20)
		return
	var line: PackedVector3Array = sel.suppress_line(sel.suppress_from, g)
	if line.size() >= 2:
		_dashed(line[0], line[1], 0.16, col, 1.4, 0.9)
	for p in line:
		_ring(p, 1.2, 0.22, col, 20)


## Face tool (O): from each selected vehicle an arrow toward the cursor.
func _draw_face_tool(view: MatchView, sel: Node) -> void:
	var g: Variant = view.ground_pick(sel.cursor())
	if g == null:
		return
	var col := Color(HudStyle.ACCENT, 0.85)
	for id in sel.selected_vehicles():
		var p: Vector3 = view.vehicles_by_id[id].pos
		var d := Vector3(g.x - p.x, 0.0, g.z - p.z)
		if d.length() < 1.0:
			continue
		d = d.normalized()
		var tip := p + d * 9.0
		_line(p + d * 3.5, tip, 0.18, col)
		var side := Vector3(-d.z, 0.0, d.x)
		_line(tip, tip - d * 2.0 + side * 1.4, 0.18, col)
		_line(tip, tip - d * 2.0 - side * 1.4, 0.18, col)
	_ring(g, 1.0, 0.25, col, 16)


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
		&"follow":
			_ring(m.pos, 5.0 + 2.0 * k, 0.4, Color(HudStyle.OWN, a), 40)
		&"reverse":
			_ring(m.pos, 1.5 + 3.0 * k, 0.4, Color(UiPalette.TEXT, a))
		&"suppress":
			_ring(m.pos, 2.0 + 3.0 * k, 0.4, Color(HudStyle.ACCENT, a), 24)
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
