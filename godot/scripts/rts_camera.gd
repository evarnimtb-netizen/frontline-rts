extends Camera3D
## RTS camera. WASD / arrows pan, Q/E rotate, mouse wheel zooms (height),
## right-drag rotates and tilts. Starts ~85 m above the map centre looking
## down at the battle.

@export var sim_path: NodePath = ^"../FrontlineSim"
@export var height := 85.0
@export var pitch_deg := -50.0
@export var yaw_deg := 0.0
@export var pan_speed := 70.0
@export var input_enabled := true

var focus := Vector3.ZERO
var _sim: Node


func _ready() -> void:
	near = 0.3
	far = 4000.0
	fov = 55.0
	_sim = get_node_or_null(sim_path)
	_apply()


func _ground(x: float, z: float) -> float:
	if _sim != null and _sim.has_method("get_height_at"):
		return _sim.get_height_at(x, z)
	return 0.0


func _apply() -> void:
	focus.y = _ground(focus.x, focus.z)
	var pitch := deg_to_rad(clampf(pitch_deg, -89.0, -10.0))
	var b := Basis.from_euler(Vector3(pitch, deg_to_rad(yaw_deg), 0.0))
	var forward := -b.z
	var dist := height / sin(-pitch)
	global_transform = Transform3D(b, focus - forward * dist)


func _process(delta: float) -> void:
	if not input_enabled:
		return
	var move := Vector2.ZERO
	if Input.is_physical_key_pressed(KEY_W) or Input.is_physical_key_pressed(KEY_UP):
		move.y -= 1.0
	if Input.is_physical_key_pressed(KEY_S) or Input.is_physical_key_pressed(KEY_DOWN):
		move.y += 1.0
	if Input.is_physical_key_pressed(KEY_A) or Input.is_physical_key_pressed(KEY_LEFT):
		move.x -= 1.0
	if Input.is_physical_key_pressed(KEY_D) or Input.is_physical_key_pressed(KEY_RIGHT):
		move.x += 1.0
	if Input.is_physical_key_pressed(KEY_Q):
		yaw_deg += 60.0 * delta
	if Input.is_physical_key_pressed(KEY_E):
		yaw_deg -= 60.0 * delta
	if move != Vector2.ZERO:
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
			height = maxf(height * 0.9, 8.0)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN:
			height = minf(height * 1.1, 800.0)
	elif event is InputEventMouseMotion and (event.button_mask & MOUSE_BUTTON_MASK_RIGHT):
		yaw_deg -= event.relative.x * 0.25
		pitch_deg = clampf(pitch_deg - event.relative.y * 0.2, -89.0, -10.0)
