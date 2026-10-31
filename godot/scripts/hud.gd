extends CanvasLayer
## Minimal stats overlay (F1 toggles).

@export var sim_path: NodePath = ^"../FrontlineSim"

var _label: Label
var _acc := 0.0
var _sim: Node


func _ready() -> void:
	_sim = get_node_or_null(sim_path)
	_label = Label.new()
	_label.position = Vector2(12, 10)
	_label.add_theme_color_override("font_color", Color(1, 1, 1))
	_label.add_theme_color_override("font_outline_color", Color(0, 0, 0))
	_label.add_theme_constant_override("outline_size", 4)
	add_child(_label)


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and event.physical_keycode == KEY_F1:
		_label.visible = not _label.visible


func _process(delta: float) -> void:
	_acc += delta
	# get_stats() copies and sorts the sim step window: twice a second.
	if _acc < 0.5 or _sim == null:
		return
	_acc = 0.0
	var s: Dictionary = _sim.get_stats()
	if s.is_empty():
		_label.text = "sim not running"
		return
	if not s.sim_alive:
		_label.text = "SIM THREAD DIED: %s" % s.sim_panic
		return
	_label.text = (
		"FPS %d   process %.2f ms\n" % [Engine.get_frames_per_second(), Performance.get_monitor(Performance.TIME_PROCESS) * 1000.0]
		+ "sim tick %d @ %d Hz, %d threads   step %.2f ms (p99 %.2f)   late %d\n" % [s.sim_tick, s.sim_tick_hz, s.sim_threads, s.sim_step_ms_last, s.sim_step_ms_p99, s.sim_late_ticks]
		+ "snapshot age %.1f ms   presenter %.0f us\n" % [s.snapshot_age_ms, s.presenter_cpu_us]
		+ "soldiers %d  drawn LOD0/1/2 %d / %d / %d  shadow casters %d\n" % [s.soldiers_live, s.soldiers_lod0, s.soldiers_lod1, s.soldiers_lod2, s.soldier_casters]
		+ "placeholder vehicles (not simulated) %d / %d   tracers %d / %d live\n" % [s.vehicles_drawn, s.vehicles_placeholder_total, s.tracers_drawn, s.projectiles_live]
		+ "instances %d   draw calls %d   primitives %d" % [s.instances_drawn, Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME), Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME)]
	)
