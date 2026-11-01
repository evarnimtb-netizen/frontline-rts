extends Control
## Minimap (HUD v2): the whole map, lightly framed. Shows the map image,
## capture points, own squads and spotted enemies (the state already hides
## unspotted ones), selected squads and the camera footprint.
##
## Setting "minimap_rotate" (default on): the map turns with the camera
## (camera forward up), sized so its diagonal is the box side and the whole
## map fits at any yaw. Only the turned square is drawn, floating over the
## world; the box's empty corners draw nothing and pass clicks through. A
## north marker sits just off the map's north edge and the yaw shows while
## the camera turns. Off: north-up, a rounded square filling the box.
## Setting "minimap_dock": "top_left" (default) or "bottom_left".
##
## Input in both modes goes through the inverse of the drawing transform:
## left click / drag moves the camera there, right-click (Ctrl: attack)
## orders the selection there. The lock button toggles the rotation
## setting: north-up it sits in the box's bottom-right corner; rotating it
## follows the turned map, hugging its bottom-right outline.
##
## The map image comes from state.map.minimap (a Texture2D, sent whenever
## it changes) or set_map_texture().

const CORNER := 8.0
const LOCK_R := 11.0
## Gap between the lock button and the turned map (px).
const LOCK_GAP := 4.0
## Map corners in map units, clockwise from north-west.
const MAP_CORNERS: Array[Vector2] = [Vector2(-1, -1), Vector2(1, -1), Vector2(1, 1), Vector2(-1, 1)]

var view: MatchView
## Rotate with the camera (else north-up).
var rotate_with_camera := true
## "top_left" or "bottom_left" (read by the HUD layout).
var dock := "top_left"
var _tex: Texture2D
var _drag := false
var _last_yaw := 0.0
var _yaw_shown_until := 0


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_STOP
	tooltip_text = " "
	var st: Node = get_node_or_null(^"/root/Settings")
	if st != null:
		rotate_with_camera = bool(st.get_value("minimap_rotate"))
		dock = String(st.get_value("minimap_dock"))
		st.changed.connect(_on_setting)


func _on_setting(key: String, value: Variant) -> void:
	if key == "minimap_rotate":
		rotate_with_camera = bool(value)
		queue_redraw()
	elif key == "minimap_dock":
		dock = String(value)


func set_map_texture(tex: Texture2D) -> void:
	_tex = tex
	queue_redraw()


func update_state(s: Dictionary) -> void:
	var t: Variant = s.get("map", {}).get("minimap")
	if t is Texture2D and t != _tex:
		_tex = t
	var yaw := yaw_rad()
	if not is_equal_approx(yaw, _last_yaw):
		_last_yaw = yaw
		_yaw_shown_until = Time.get_ticks_msec() + 800
	queue_redraw()


## Camera yaw (radians) the map turns by; 0 when north-up.
func yaw_rad() -> float:
	if not rotate_with_camera or view == null or view.camera == null:
		return 0.0
	return deg_to_rad(float(view.camera.get("yaw_deg")))


## Half the drawn map side in px: the full box north-up; when rotating,
## side = box side / sqrt 2, so the turned square stays in the box.
func map_half_px() -> float:
	var half := minf(size.x, size.y) * 0.5
	return half / sqrt(2.0) if rotate_with_camera else half


func centre() -> Vector2:
	return size * 0.5


## World (x, z) → minimap px.
func world_to_map(p: Vector3) -> Vector2:
	var half := view.map_half
	var u := Vector2(p.x / half, p.z / half)
	return centre() + u.rotated(yaw_rad()) * map_half_px()


## Minimap px → world point (ground height), clamped to the map. Inverse
## of world_to_map: un-rotate by the yaw, then scale.
func map_to_world(m: Vector2) -> Vector3:
	var half := view.map_half
	var u := _map_units(m).clamp(Vector2(-1, -1), Vector2(1, 1))
	var x := u.x * half
	var z := u.y * half
	return Vector3(x, view.ground_y(x, z), z)


## Map px → map units (-1..1 on both axes inside the map), un-rotated.
func _map_units(m: Vector2) -> Vector2:
	return ((m - centre()) / map_half_px()).rotated(-yaw_rad())


## Centre of the lock button. North-up: the box's bottom-right corner.
## Rotating: on the box's diagonal towards bottom-right, just past the
## map's reach along it (its furthest corner projected on the diagonal),
## so it sits against the map's bottom-right corner or edge. The reach is
## a max over the corners, so it slides smoothly as the camera turns and
## never jumps between corners. The map reaches at most half the box side,
## so the button fits inside the box at any yaw (box side >= 155 px); the
## clamp is only a guard.
func lock_centre() -> Vector2:
	if not rotate_with_camera:
		return size - Vector2(LOCK_R + 5.0, LOCK_R + 5.0)
	var d := Vector2(1, 1).normalized()
	var reach := 0.0
	for u in MAP_CORNERS:
		reach = maxf(reach, u.rotated(yaw_rad()).dot(d) * map_half_px())
	var at := centre() + d * (reach + LOCK_GAP + LOCK_R)
	var m := Vector2(LOCK_R + 1.0, LOCK_R + 1.0)
	return at.clamp(m, size - m)


func _draw() -> void:
	if view == null:
		return
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var half := minf(size.x, size.y) * 0.5
	var c := centre()
	var yaw := yaw_rad()
	var hs := map_half_px()
	var corners := PackedVector2Array()
	var uvs := PackedVector2Array()
	if rotate_with_camera:
		for u in MAP_CORNERS:
			corners.append(c + u.rotated(yaw) * hs)
			uvs.append((u + Vector2.ONE) * 0.5)
	else:
		_rounded_square(c, hs, corners, uvs)
	var cols := PackedColorArray()
	cols.resize(corners.size())
	cols.fill(Color.WHITE if _tex != null else Color(0.42, 0.48, 0.34))
	draw_polygon(corners, cols, uvs, _tex)
	var edge := corners.duplicate()
	edge.append(corners[0])
	draw_polyline(edge, Color(HudStyle.TEXT, 0.55 if rotate_with_camera else 0.35), 1.5, true)
	var local := view.local_player
	# Squads: own and spotted enemy; selected ones get a white frame.
	var sel: Node = view.selection
	var marked: PackedInt32Array = sel.selected_squads()
	for sq in view.state.get("squads", []):
		if _off_map(sq.pos):
			continue
		var own := int(sq.owner) == local
		var at := world_to_map(sq.pos)
		if own and marked.has(int(sq.id)):
			draw_rect(Rect2(at - Vector2(4.5, 4.5), Vector2(9, 9)), HudStyle.TEXT)
		draw_rect(Rect2(at - Vector2(3, 3), Vector2(6, 6)), HudStyle.OWN if own else HudStyle.ENEMY)
		draw_rect(Rect2(at - Vector2(3, 3), Vector2(6, 6)), Color(0.08, 0.1, 0.12), false, 1.0)
	# Vehicles (S3): rectangles along the hull; wrecks as dim crosses.
	for v in view.state.get("vehicles", []):
		if _off_map(v.pos):
			continue
		var at := world_to_map(v.pos)
		if Vehicles.is_wreck(v):
			var wc := Color(HudStyle.TEXT, 0.35)
			draw_line(at - Vector2(3, 3), at + Vector2(3, 3), wc, 1.5)
			draw_line(at + Vector2(-3, 3), at + Vector2(3, -3), wc, 1.5)
			continue
		var own := int(v.owner) == local
		# Hull forward on the map: world −Z turned by yaw, then the map's
		# own rotation.
		var f := world_to_map(v.pos + Vector3(-sin(v.yaw), 0.0, -cos(v.yaw))) - at
		f = f.normalized() if f.length() > 0.001 else Vector2.UP
		var s := Vector2(-f.y, f.x)
		var pts := PackedVector2Array([at + f * 5.5 + s * 3.5, at + f * 5.5 - s * 3.5, at - f * 5.5 - s * 3.5, at - f * 5.5 + s * 3.5])
		if own and view.selection.is_selected(int(v.id)):
			var frame := PackedVector2Array([at + f * 7.0 + s * 5.0, at + f * 7.0 - s * 5.0, at - f * 7.0 - s * 5.0, at - f * 7.0 + s * 5.0])
			draw_colored_polygon(frame, HudStyle.TEXT)
		draw_colored_polygon(pts, HudStyle.OWN if own else HudStyle.ENEMY)
		pts.append(pts[0])
		draw_polyline(pts, Color(0.08, 0.1, 0.12), 1.0)
	# Capture points.
	for p in view.state.get("points", []):
		if _off_map(p.pos):
			continue
		var at := world_to_map(p.pos)
		var owner_i := int(p.get("owner", -1))
		var ring := HudStyle.ACCENT if p.get("contested", false) else HudStyle.side_color(owner_i, local)
		draw_circle(at, 9.0, Color(0.055, 0.07, 0.08, 0.75))
		draw_arc(at, 9.0, 0.0, TAU, 20, ring, 2.0, true)
		HudStyle.text(self, bold, Vector2(at.x, at.y + 5.0), str(p.id), HudStyle.FONT_SMALL, HudStyle.TEXT, 0)
	# Camera footprint.
	if view.camera != null and view.camera.has_method("ground_corners"):
		var poly := _footprint(view.camera.ground_corners())
		if poly.size() >= 3:
			var fill := PackedColorArray()
			fill.resize(poly.size())
			fill.fill(Color(1, 1, 1, 0.10))
			draw_polygon(poly, fill)
			poly.append(poly[0])
			draw_polyline(poly, Color(0.95, 0.95, 0.92), 1.5, true)
	# North marker and yaw readout.
	if rotate_with_camera:
		var n := c + Vector2(0, -1).rotated(yaw) * (half * 0.9)
		draw_circle(n, 10.0, Color(0.055, 0.07, 0.08, 0.85))
		draw_arc(n, 10.0, 0.0, TAU, 20, HudStyle.ACCENT, 1.5, true)
		HudStyle.text(self, bold, n + Vector2(0, 5), "N", HudStyle.FONT_SMALL, HudStyle.ACCENT, 0)
		if Time.get_ticks_msec() < _yaw_shown_until:
			var deg := posmod(roundi(rad_to_deg(-yaw)), 360)
			HudStyle.text(self, bold, c + Vector2(0, -half + 22.0), "%d°" % deg, HudStyle.FONT_SMALL, HudStyle.TEXT, 0)
	else:
		HudStyle.text(self, bold, Vector2(c.x, 15.0), "N", HudStyle.FONT_SMALL, Color(HudStyle.ACCENT, 0.9), 0)
	# North-up lock / rotate toggle.
	var lc := lock_centre()
	draw_circle(lc, LOCK_R, Color(0.055, 0.07, 0.08, 0.8))
	draw_arc(lc, LOCK_R, 0.0, TAU, 20, Color(HudStyle.TEXT, 0.55), 1.0, true)
	HudStyle.lock(self, lc, HudStyle.TEXT, not rotate_with_camera)


func _rounded_square(c: Vector2, hs: float, pts: PackedVector2Array, uvs: PackedVector2Array) -> void:
	var r := CORNER
	var steps := 4
	var centres := [c + Vector2(hs - r, -hs + r), c + Vector2(hs - r, hs - r), c + Vector2(-hs + r, hs - r), c + Vector2(-hs + r, -hs + r)]
	var start := [-PI * 0.5, 0.0, PI * 0.5, PI]
	for k in 4:
		for i in steps + 1:
			var a: float = start[k] + PI * 0.5 * i / steps
			var p: Vector2 = centres[k] + Vector2(cos(a), sin(a)) * r
			pts.append(p)
			uvs.append((p - (c - Vector2(hs, hs))) / (2.0 * hs))


## True when a world point is off the map while rotating: nothing is drawn
## in the box's empty corners. North-up the box is the map.
func _off_map(p: Vector3) -> bool:
	if not rotate_with_camera:
		return false
	var half := view.map_half
	return absf(p.x) > half + 0.5 or absf(p.z) > half + 0.5


## Camera footprint in map px. North-up its corners are clamped to the box;
## rotating, the polygon is clipped to the map square (in map units, before
## the turn) so it never spills into the empty corners.
func _footprint(ground: Array) -> PackedVector2Array:
	var poly := PackedVector2Array()
	if not rotate_with_camera:
		for w in ground:
			poly.append(world_to_map(w).clamp(Vector2.ZERO, size))
		return poly
	var half := view.map_half
	for w in ground:
		poly.append(Vector2(w.x / half, w.z / half))
	if poly.size() < 3:
		return PackedVector2Array()
	var square := PackedVector2Array([Vector2(-1, -1), Vector2(1, -1), Vector2(1, 1), Vector2(-1, 1)])
	var parts := Geometry2D.intersect_polygons(poly, square)
	poly = PackedVector2Array()
	if parts.is_empty():
		return poly
	for u in parts[0]:
		poly.append(centre() + u.rotated(yaw_rad()) * map_half_px())
	return poly


func _gui_input(event: InputEvent) -> void:
	if view == null or view.input_blocked():
		return
	if event is InputEventMouseButton and event.pressed:
		if event.button_index == MOUSE_BUTTON_LEFT and _on_lock(event.position):
			toggle_rotation()
		elif not _inside(event.position):
			return
		elif event.button_index == MOUSE_BUTTON_LEFT:
			_drag = true
			view.jump_camera(map_to_world(event.position))
		elif event.button_index == MOUSE_BUTTON_RIGHT:
			view.selection.order_to_ground(map_to_world(event.position), event.ctrl_pressed or event.meta_pressed)
		accept_event()
	elif event is InputEventMouseButton and not event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		_drag = false
		accept_event()
	elif event is InputEventMouseMotion and _drag:
		view.jump_camera(map_to_world(event.position))
		accept_event()


## On the map: the box north-up, the turned square when rotating. Clicks
## elsewhere in the box (rotating) pass through to the world.
func _inside(p: Vector2) -> bool:
	if not rotate_with_camera:
		return Rect2(Vector2.ZERO, size).has_point(p)
	var u := _map_units(p)
	return absf(u.x) <= 1.0 and absf(u.y) <= 1.0


func _on_lock(p: Vector2) -> bool:
	return p.distance_to(lock_centre()) <= LOCK_R + 2.0


func _has_point(p: Vector2) -> bool:
	return _inside(p) or _on_lock(p)


## Flips north-up / rotate and saves it as the setting.
func toggle_rotation() -> void:
	rotate_with_camera = not rotate_with_camera
	var st: Node = get_node_or_null(^"/root/Settings")
	if st != null:
		st.set_value("minimap_rotate", rotate_with_camera)
	queue_redraw()


func _get_tooltip(at: Vector2) -> String:
	if _on_lock(at):
		return "Rotate with the camera" if not rotate_with_camera else "Lock north-up"
	return "Click: look here · Right-click: move here (Ctrl: attack)"
