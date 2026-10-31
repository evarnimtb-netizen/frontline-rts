extends Camera3D
## RTS camera. Arrows (and WASD when `wasd_pan`) pan, Q/E rotate, mouse
## wheel zooms (height), right- or middle-drag rotates and tilts, and with
## `edge_scroll` the screen edges pan. Starts ~85 m above the map centre
## looking down at the battle.
##
## The match scene sets: wasd_pan off (A/S/D are order hotkeys), right-drag
## off (right-click orders), edge scroll on, height 10–150 m, and
## `bounds_half_m` to clamp the focus to the map.

@export var sim_path: NodePath = ^"../FrontlineSim"
@export var height := 85.0
@export var pitch_deg := -50.0
@export var yaw_deg := 0.0
@export var pan_speed := 70.0
@export var input_enabled := true
@export var wasd_pan := true
@export var right_drag_rotate := true
@export var middle_drag_rotate := true
@export var edge_scroll := false
@export var edge_margin_px := 10.0
@export var min_height := 8.0
@export var max_height := 800.0
## Clamp the focus to [-b, b] on x and z; 0 disables the clamp.
@export var bounds_half_m := 0.0
@export var rotate_speed_deg := 60.0

## Optional ground height override, (x: float, z: float) -> float. Used
## before the sim node's get_height_at.
var height_fn: Callable
var focus := Vector3.ZERO
var _sim: Node
var _mouse_seen := false


func _ready() -> void:
	near = 0.3
	far = 4000.0
	fov = 55.0
	_sim = get_node_or_null(sim_path)
	height = clampf(height, min_height, max_height)
	_apply()


func _ground(x: float, z: float) -> float:
	if height_fn.is_valid():
		return height_fn.call(x, z)
	if _sim != null and _sim.has_method("get_height_at"):
		return _sim.get_height_at(x, z)
	return 0.0


func _apply() -> void:
	if bounds_half_m > 0.0:
		focus.x = clampf(focus.x, -bounds_half_m, bounds_half_m)
		focus.z = clampf(focus.z, -bounds_half_m, bounds_half_m)
	focus.y = _ground(focus.x, focus.z)
	var pitch := deg_to_rad(clampf(pitch_deg, -89.0, -10.0))
	var b := Basis.from_euler(Vector3(pitch, deg_to_rad(yaw_deg), 0.0))
	var forward := -b.z
	var dist := height / sin(-pitch)
	global_transform = Transform3D(b, focus - forward * dist)


## Centres the view on a world position (minimap clicks, group recall).
func jump_to(pos: Vector3) -> void:
	focus.x = pos.x
	focus.z = pos.z
	_apply()


## Where the four screen corners hit the ground plane at the focus height,
## in order top-left, top-right, bottom-right, bottom-left (for the
## minimap frustum). Corners above the horizon are cut off at `far`.
func ground_corners() -> PackedVector3Array:
	var out := PackedVector3Array()
	var vp := get_viewport().get_visible_rect().size
	var plane := Plane(Vector3.UP, focus.y)
	for c in [Vector2.ZERO, Vector2(vp.x, 0), vp, Vector2(0, vp.y)]:
		var o := project_ray_origin(c)
		var d := project_ray_normal(c)
		var hit: Variant = plane.intersects_ray(o, d)
		if hit == null:
			var flat := Vector3(d.x, 0.0, d.z).normalized()
			hit = Vector3(o.x, focus.y, o.z) + flat * minf(far, 1500.0)
		out.append(hit)
	return out


func _process(delta: float) -> void:
	if not input_enabled:
		return
	var move := Vector2.ZERO
	if Input.is_physical_key_pressed(KEY_UP) or (wasd_pan and Input.is_physical_key_pressed(KEY_W)):
		move.y -= 1.0
	if Input.is_physical_key_pressed(KEY_DOWN) or (wasd_pan and Input.is_physical_key_pressed(KEY_S)):
		move.y += 1.0
	if Input.is_physical_key_pressed(KEY_LEFT) or (wasd_pan and Input.is_physical_key_pressed(KEY_A)):
		move.x -= 1.0
	if Input.is_physical_key_pressed(KEY_RIGHT) or (wasd_pan and Input.is_physical_key_pressed(KEY_D)):
		move.x += 1.0
	if Input.is_physical_key_pressed(KEY_Q):
		yaw_deg += rotate_speed_deg * delta
	if Input.is_physical_key_pressed(KEY_E):
		yaw_deg -= rotate_speed_deg * delta
	if edge_scroll and _mouse_seen and get_window().has_focus():
		var vp := get_viewport().get_visible_rect()
		var m := get_viewport().get_mouse_position()
		if vp.has_point(m):
			if m.x < edge_margin_px:
				move.x -= 1.0
			elif m.x > vp.size.x - edge_margin_px:
				move.x += 1.0
			if m.y < edge_margin_px:
				move.y -= 1.0
			elif m.y > vp.size.y - edge_margin_px:
				move.y += 1.0
	if move != Vector2.ZERO:
		move = move.limit_length(1.0)
		var yaw := deg_to_rad(yaw_deg)
		var right := Vector3(cos(yaw), 0.0, -sin(yaw))
		var fwd := Vector3(-sin(yaw), 0.0, -cos(yaw))
		var speed := pan_speed * maxf(height / 85.0, 0.3)
		focus += (right * move.x - fwd * move.y) * speed * delta
	_apply()


func _unhandled_input(event: InputEvent) -> void:
	if not input_enabled:
		return
	if event is InputEventMouseButton and event.pressed:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP:
			height = maxf(height * 0.9, min_height)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN:
			height = minf(height * 1.1, max_height)
	elif event is InputEventMouseMotion:
		_mouse_seen = true
		var rotating: bool = (right_drag_rotate and (event.button_mask & MOUSE_BUTTON_MASK_RIGHT)) \
			or (middle_drag_rotate and (event.button_mask & MOUSE_BUTTON_MASK_MIDDLE))
		if rotating:
			yaw_deg -= event.relative.x * 0.25
			pitch_deg = clampf(pitch_deg - event.relative.y * 0.2, -89.0, -10.0)
