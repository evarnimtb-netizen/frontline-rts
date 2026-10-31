extends Node
## M0 render benchmark (SYSTEM_DESIGN.md §2.2 gate 7, §15 M0).
##
## Instances main.tscn with the requested counts, waits for warm-up, records
## per-frame times for --seconds, writes JSON to --out and quits.
##
##   Godot --path godot --resolution 1920x1080 --disable-vsync res://bench.tscn \
##       [--rendering-method mobile] -- \
##       --soldiers 2000 --vehicles 300 --seconds 20 --out /abs/out.json \
##       --renderer forward_plus
##
## Pass --disable-vsync on the Godot command line (before `--`) so the
## swapchain is created without vsync; the script only checks and records
## the effective mode.
##
## Other user args: --warmup S, --projectile_cap N, --map_size_m N,
## --spawn_area_m N, --sim_threads N, --tick_hz 30|20, --seed N,
## --view rts|close|ground (camera preset; see VIEWS), --height M and
## --pitch DEG (override the view), --max_fps N, --hud 0|1,
## --lod0_distance M / --lod1_distance M / --draw_distance M (override the
## node's LOD switch distances), --preset deck (§2.2 Steam Deck presenter
## profile: 1280x800, 0.75 render scale + FSR1, 30 fps lock, LOD2 to 200 m;
## run it with --resolution 1280x800 and, for the worker-pool cap, through
## tools/render_bench.sh), --screenshot /abs/shot.png (saved when recording
## starts), --allow_headless (record CPU-side numbers without a display).
## The default spawn area (200 m) keeps most of the battle inside the view.
##
## Frame time = wall time between _process calls. GPU and render-CPU times
## come from RenderingServer.viewport_get_measured_render_time_*(); Godot
## 4.7.2 on Metal reports 0 for GPU time, so `gpu_timing_available` is false
## there and GPU-busy % cannot be computed. All numbers on a non-target
## machine are a baseline only.

const MAIN := preload("res://main.tscn")

## Camera presets: height above the focus (m) and pitch (deg).
const VIEWS := {
	"rts": {"height": 85.0, "pitch": -50.0},   # every soldier beyond 80 m: LOD2
	"close": {"height": 18.0, "pitch": -30.0}, # LOD0 / LOD1 / LOD2 mix
	"ground": {"height": 3.0, "pitch": -8.0},  # direct-control style
}

var opts := {
	"soldiers": 2000,
	"vehicles": 300,
	"seconds": 20.0,
	"warmup": 5.0,
	"out": "user://bench.json",
	"renderer": "",
	"projectile_cap": 5000,
	"map_size_m": 1024,
	"spawn_area_m": 200,
	"sim_threads": 3,
	"tick_hz": 30,
	"seed": 1,
	"view": "rts",
	"height": -1.0,
	"pitch": 0.0,
	"max_fps": 0,
	"hud": false,
	"lod0_distance": -1.0,
	"lod1_distance": -1.0,
	"draw_distance": -1.0,
	"preset": "",
	"screenshot": "",
	"allow_headless": false,
}

var _sim: Node
var _vp: RID
var _state := 0  # 0 warm-up, 1 recording, 2 done
var _t_start := 0
var _t_last := 0
var _t_rec := 0
var _counter_names := PackedStringArray()
var _series := {
	"frame_ms": PackedFloat32Array(),
	"gpu_ms": PackedFloat32Array(),
	"gpu_timestamp_span_ms": PackedFloat32Array(),
	"render_cpu_ms": PackedFloat32Array(),
	"process_ms": PackedFloat32Array(),
	"draw_calls": PackedFloat32Array(),
	"primitives": PackedFloat32Array(),
	"objects": PackedFloat32Array(),
}


func _ready() -> void:
	_parse(OS.get_cmdline_user_args())
	if DisplayServer.get_name() == "headless" and not opts.allow_headless:
		push_error("bench: the display server is headless, so no GPU work is measured. Run windowed, or pass --allow_headless for CPU-side numbers only.")
		get_tree().quit(2)
		return
	if not VIEWS.has(opts.view):
		push_error("bench: unknown --view %s" % opts.view)
		get_tree().quit(2)
		return
	var main := MAIN.instantiate()
	_sim = main.get_node("FrontlineSim")
	for k in ["soldiers", "vehicles", "projectile_cap", "map_size_m", "spawn_area_m", "sim_threads", "tick_hz", "seed"]:
		_sim.set(k, opts[k])
	if opts.preset == "deck":
		_apply_deck_preset()
	elif opts.preset != "":
		push_error("bench: unknown --preset %s" % opts.preset)
		get_tree().quit(2)
		return
	Engine.max_fps = int(opts.max_fps)
	for k in ["lod0_distance", "lod1_distance", "draw_distance"]:
		if float(opts[k]) >= 0.0:
			_sim.set(k, float(opts[k]))
	var view: Dictionary = VIEWS[opts.view]
	var cam := main.get_node("Camera")
	cam.input_enabled = false
	cam.height = float(opts.height) if float(opts.height) > 0.0 else float(view.height)
	cam.pitch_deg = float(opts.pitch) if float(opts.pitch) != 0.0 else float(view.pitch)
	if not opts.hud:
		main.get_node("HUD").queue_free()
	add_child(main)
	if not _sim.is_running():
		push_error("bench: FrontlineSim did not start")
		get_tree().quit(1)
		return
	_counter_names = _sim.get_frame_counter_names()
	for n in _counter_names:
		_series[n] = PackedFloat32Array()
	_vp = get_viewport().get_viewport_rid()
	RenderingServer.viewport_set_measure_render_time(_vp, true)
	_t_start = Time.get_ticks_usec()
	_t_last = _t_start
	if DisplayServer.window_get_vsync_mode() != DisplayServer.VSYNC_DISABLED:
		push_warning("bench: vsync is on; pass --disable-vsync to Godot (before `--`)")
	print("bench: %s, warm-up %.1f s, recording %.1f s" % [_label(), opts.warmup, opts.seconds])


## §2.2 Steam Deck presenter profile.
func _apply_deck_preset() -> void:
	var vp := get_viewport()
	vp.scaling_3d_mode = Viewport.SCALING_3D_MODE_FSR
	vp.scaling_3d_scale = 0.75
	if opts.max_fps == 0:
		opts.max_fps = 30
	if float(opts.draw_distance) < 0.0:
		opts.draw_distance = 200.0
	if DisplayServer.get_name() != "headless":
		get_window().size = Vector2i(1280, 800)


func _parse(args: PackedStringArray) -> void:
	var i := 0
	while i < args.size():
		var a := args[i]
		i += 1
		if not a.begins_with("--"):
			continue
		var key := a.substr(2).replace("-", "_")
		var value := ""
		if a.contains("="):
			key = a.substr(2, a.find("=") - 2).replace("-", "_")
			value = a.substr(a.find("=") + 1)
		elif i < args.size() and not args[i].begins_with("--"):
			value = args[i]
			i += 1
		if not opts.has(key):
			push_warning("bench: unknown option --%s" % key)
			continue
		match typeof(opts[key]):
			TYPE_INT:
				opts[key] = int(value)
			TYPE_FLOAT:
				opts[key] = float(value)
			TYPE_BOOL:
				opts[key] = value == "" or value.to_lower() in ["1", "true", "yes", "on"]
			_:
				opts[key] = value


func _label() -> String:
	return "%s soldiers=%d vehicles=%d renderer=%s view=%s preset=%s" % [
		_rendering_method(), opts.soldiers, opts.vehicles, opts.renderer, opts.view, opts.preset]


func _rendering_method() -> String:
	if RenderingServer.has_method("get_current_rendering_method"):
		return RenderingServer.get_current_rendering_method()
	return str(ProjectSettings.get_setting("rendering/renderer/rendering_method"))


func _process(_delta: float) -> void:
	var now := Time.get_ticks_usec()
	var dt_ms := float(now - _t_last) / 1000.0
	_t_last = now
	match _state:
		0:
			if float(now - _t_start) / 1e6 >= opts.warmup:
				_state = 1
				_t_rec = now
				_sim.reset_sim_stats()
				if opts.screenshot != "":
					_save_screenshot.call_deferred()
		1:
			# Per frame: only lock-free counters (get_stats() copies and
			# sorts the sim window; it is called once, at the end).
			var c: PackedFloat32Array = _sim.get_frame_counters()
			for k in c.size():
				_series[_counter_names[k]].append(c[k])
			_series.frame_ms.append(dt_ms)
			_series.gpu_ms.append(RenderingServer.viewport_get_measured_render_time_gpu(_vp))
			_series.gpu_timestamp_span_ms.append(_gpu_span_ms())
			_series.render_cpu_ms.append(RenderingServer.viewport_get_measured_render_time_cpu(_vp))
			_series.process_ms.append(Performance.get_monitor(Performance.TIME_PROCESS) * 1000.0)
			_series.draw_calls.append(Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME))
			_series.primitives.append(Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME))
			_series.objects.append(Performance.get_monitor(Performance.RENDER_TOTAL_OBJECTS_IN_FRAME))
			if float(now - _t_rec) / 1e6 >= opts.seconds:
				_state = 2
				_finish(float(now - _t_rec) / 1e6)


## GPU time between the first and last RenderingDevice timestamps of the
## last captured frame (fallback when the viewport GPU timer reports 0).
## Godot's own frame profile divides these by 1e6 (ns -> ms). This is the
## elapsed span, which includes idle gaps; it is not GPU-busy time.
func _gpu_span_ms() -> float:
	var rd := RenderingServer.get_rendering_device()
	if rd == null:
		return 0.0
	var n := rd.get_captured_timestamps_count()
	if n < 2:
		return 0.0
	return float(rd.get_captured_timestamp_gpu_time(n - 1) - rd.get_captured_timestamp_gpu_time(0)) / 1e6


func _save_screenshot() -> void:
	await RenderingServer.frame_post_draw
	var img := get_viewport().get_texture().get_image()
	if img != null:
		var path: String = opts.screenshot
		DirAccess.make_dir_recursive_absolute(path.get_base_dir())
		img.save_png(path)
		print("bench: screenshot -> %s" % path)


static func _summary(a: PackedFloat32Array) -> Dictionary:
	if a.is_empty():
		return {}
	var s := a.duplicate()
	s.sort()
	var n := s.size()
	var total := 0.0
	for v in s:
		total += v
	var pct := func(p: float) -> float: return s[int(round((n - 1) * p))]
	return {
		"mean": total / n, "min": s[0], "p50": pct.call(0.5), "p95": pct.call(0.95),
		"p99": pct.call(0.99), "max": s[n - 1],
	}


func _out_path() -> String:
	var p: String = opts.out
	if p.begins_with("user://") or p.begins_with("res://") or p.is_absolute_path():
		return p
	var cwd := OS.get_environment("PWD")
	return cwd.path_join(p) if cwd != "" else "user://".path_join(p)


func _vsync_name(m: int) -> String:
	match m:
		DisplayServer.VSYNC_DISABLED: return "disabled"
		DisplayServer.VSYNC_ENABLED: return "enabled"
		DisplayServer.VSYNC_ADAPTIVE: return "adaptive"
		DisplayServer.VSYNC_MAILBOX: return "mailbox"
	return str(m)


func _finish(recorded_s: float) -> void:
	var frames: int = _series.frame_ms.size()
	var vp := get_viewport()
	var win := get_window()
	var refresh := DisplayServer.screen_get_refresh_rate()
	var result := {
		"benchmark": "frontline-m0-render",
		"timestamp_utc": Time.get_datetime_string_from_system(true),
		"renderer_label": opts.renderer,
		"rendering_method": _rendering_method(),
		"rendering_driver": RenderingServer.get_current_rendering_driver_name() if RenderingServer.has_method("get_current_rendering_driver_name") else "",
		"video_adapter": RenderingServer.get_video_adapter_name(),
		"video_vendor": RenderingServer.get_video_adapter_vendor(),
		"display_server": DisplayServer.get_name(),
		"vsync_mode": _vsync_name(DisplayServer.window_get_vsync_mode()),
		"refresh_rate_hz": refresh,
		"max_fps": Engine.max_fps,
		"resolution": [vp.get_visible_rect().size.x, vp.get_visible_rect().size.y],
		"window_size": [win.size.x, win.size.y],
		"screen_scale": DisplayServer.screen_get_scale(),
		"scaling_3d_mode": vp.scaling_3d_mode,
		"scaling_3d_scale": vp.scaling_3d_scale,
		"worker_pool_max_threads": ProjectSettings.get_setting("threading/worker_pool/max_threads"),
		"engine": Engine.get_version_info().string,
		"cpu": OS.get_processor_name(),
		"cpu_count": OS.get_processor_count(),
		"model": OS.get_model_name(),
		"options": opts,
		"recorded_s": recorded_s,
		"frames": frames,
		"fps_mean": frames / recorded_s if recorded_s > 0.0 else 0.0,
		"memory_static_mb": Performance.get_monitor(Performance.MEMORY_STATIC) / 1048576.0,
		"video_mem_mb": Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0,
		"bridge": _sim.get_stats(),
	}
	for k in _series:
		result[k] = _summary(_series[k])

	# Validity: a frame time that sits on a multiple of the refresh interval
	# means the run was paced by the display or compositor, not by the work.
	var fm: Dictionary = result.frame_ms
	var notes := PackedStringArray()
	var refresh_locked := false
	if Engine.max_fps > 0:
		notes.append("frame rate capped at %d fps (max_fps): frame times show the cap, not the cost" % Engine.max_fps)
	elif refresh > 0.0 and not fm.is_empty():
		var interval := 1000.0 / refresh
		for k in [1, 2, 3, 4]:
			if absf(fm.p50 - k * interval) <= 0.03 * k * interval:
				refresh_locked = true
				notes.append("p50 frame time %.2f ms is within 3%% of %d x the %.1f Hz refresh interval: display/compositor-locked" % [fm.p50, k, refresh])
				break
	if result.vsync_mode != "disabled":
		notes.append("vsync is %s" % result.vsync_mode)
	var gm: Dictionary = result.gpu_ms
	var gs: Dictionary = result.gpu_timestamp_span_ms
	var gpu_ok: bool = (not gm.is_empty() and gm.max > 0.0)
	result["gpu_span_available"] = (not gs.is_empty() and gs.max > 0.0)
	if not gpu_ok:
		notes.append("GPU timing unavailable (every viewport GPU sample is 0): GPU ms and GPU-busy % cannot be reported")
	else:
		# A timer that is never refreshed (seen under MoltenVK) repeats one
		# value; one that lags the frames reports more GPU time than wall time.
		var distinct := {}
		for v in _series.gpu_ms:
			distinct[v] = true
		var stale: bool = distinct.size() < maxi(3, frames / 20)
		var over: bool = gm.mean > fm.get("mean", 0.0) * 1.02
		if stale or over:
			gpu_ok = false
			notes.append("GPU timing rejected: %d distinct values in %d frames%s" % [
				distinct.size(), frames, ", GPU ms above frame ms" if over else ""])
	result["gpu_timing_available"] = gpu_ok
	if not gpu_ok:
		push_warning("bench: GPU TIMING UNAVAILABLE on %s/%s; only CPU-side frame times are meaningful" % [result.rendering_driver, result.video_adapter])
	result["gpu_busy_pct"] = (100.0 * gm.mean / fm.mean) if (gpu_ok and fm.get("mean", 0.0) > 0.0) else -1.0
	result["refresh_locked"] = refresh_locked
	result["valid_for_cost_comparison"] = not refresh_locked and Engine.max_fps == 0 and result.vsync_mode == "disabled"
	result["notes"] = notes

	var path := _out_path()
	DirAccess.make_dir_recursive_absolute(path.get_base_dir())
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		push_error("bench: cannot write %s (%s)" % [path, error_string(FileAccess.get_open_error())])
		get_tree().quit(1)
		return
	f.store_string(JSON.stringify(result, "  "))
	f.close()
	print("bench: %d frames, frame ms mean %.2f p95 %.2f p99 %.2f, gpu ms %s, sim step p99 %.2f ms, valid=%s -> %s" % [
		frames, fm.get("mean", 0.0), fm.get("p95", 0.0), fm.get("p99", 0.0),
		("%.2f" % gm.mean) if gpu_ok else "n/a", result.bridge.get("sim_step_ms_p99", 0.0),
		result.valid_for_cost_comparison, ProjectSettings.globalize_path(path)])
	for n in notes:
		print("bench: note: %s" % n)
	_sim.stop()
	get_tree().quit()
