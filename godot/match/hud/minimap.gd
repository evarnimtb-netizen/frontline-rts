extends Control
## Minimap: map image (north up), capture points, own squads and spotted
## enemies from the state (the state already hides unspotted enemies),
## and the camera's ground footprint. Left click / drag moves the camera,
## right-click orders the selection there (same rules as in the world).
##
## The map image comes from state.map.minimap (a Texture2D, sent whenever
## it changes) or set_map_texture().

var view: MatchView
var _tex: Texture2D
var _drag := false


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_STOP
	clip_contents = true


func set_map_texture(tex: Texture2D) -> void:
	_tex = tex
	queue_redraw()


func update_state(s: Dictionary) -> void:
	var t: Variant = s.get("map", {}).get("minimap")
	if t is Texture2D and t != _tex:
		_tex = t
	queue_redraw()


func _to_map(p: Vector3) -> Vector2:
	var half := view.map_half
	return Vector2((p.x + half) / (2.0 * half), (p.z + half) / (2.0 * half)) * size


func _to_world(m: Vector2) -> Vector3:
	var half := view.map_half
	var uv := (m / size).clamp(Vector2.ZERO, Vector2.ONE)
	var x := uv.x * 2.0 * half - half
	var z := uv.y * 2.0 * half - half
	return Vector3(x, view.ground_y(x, z), z)


func _draw() -> void:
	if view == null:
		return
	var r := Rect2(Vector2.ZERO, size)
	if _tex != null:
		draw_texture_rect(_tex, r, false)
	else:
		draw_rect(r, Color(0.2, 0.24, 0.17))
	var local := view.local_player
	var font := get_theme_default_font()
	for p in view.state.get("points", []):
		var c := _to_map(p.pos)
		var col := UiPalette.owner_color(p.get("owner", -1), local)
		draw_circle(c, 8.0, Color(UiPalette.BG, 0.85))
		draw_arc(c, 8.0, 0.0, TAU, 20, col, 2.0)
		draw_string(font, c + Vector2(-8, 5), str(p.id), HORIZONTAL_ALIGNMENT_CENTER, 16, 12, UiPalette.TEXT)
	var sel: Node = view.selection
	for sq in view.state.get("squads", []):
		var own := int(sq.owner) == local
		var c := _to_map(sq.pos)
		var col := UiPalette.OWN if own else UiPalette.ENEMY
		if own and sel.is_selected(sq.id):
			draw_rect(Rect2(c - Vector2(4, 4), Vector2(8, 8)), UiPalette.TEXT, true)
		draw_rect(Rect2(c - Vector2(3, 3), Vector2(6, 6)), col.lightened(0.1), true)
	# Camera footprint.
	if view.camera != null and view.camera.has_method("ground_corners"):
		var corners: PackedVector3Array = view.camera.ground_corners()
		var poly := PackedVector2Array()
		for w in corners:
			poly.append(_to_map(w).clamp(Vector2.ZERO, size))
		poly.append(poly[0])
		draw_polyline(poly, Color(1, 1, 1, 0.85), 1.5)
	draw_rect(r, UiPalette.BORDER, false, 1.0)


func _gui_input(event: InputEvent) -> void:
	if view == null or view.input_blocked():
		return
	if event is InputEventMouseButton and event.pressed:
		if event.button_index == MOUSE_BUTTON_LEFT:
			_drag = true
			view.jump_camera(_to_world(event.position))
		elif event.button_index == MOUSE_BUTTON_RIGHT:
			view.selection.order_to_ground(_to_world(event.position))
		accept_event()
	elif event is InputEventMouseButton and not event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		_drag = false
		accept_event()
	elif event is InputEventMouseMotion and _drag:
		view.jump_camera(_to_world(event.position))
		accept_event()
