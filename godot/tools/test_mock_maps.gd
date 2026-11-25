extends SceneTree
## Headless smoke test of the mock match on both maps (S16 Presenter P):
## the mock follows the chosen map (name, points owned, roads on Linden
## Brook only), the road strip is built, the minimap image differs between
## maps and shows the road colour, and a few seconds of the match run.
##
##   Godot --headless --path godot -s res://tools/test_mock_maps.gd

var _fails := 0
var _checks := 0


func _initialize() -> void:
	_run.call_deferred()
	create_timer(120.0).timeout.connect(func() -> void:
		printerr("test_mock_maps: timed out")
		quit(2))


func _check(ok: bool, what: String) -> void:
	_checks += 1
	if not ok:
		_fails += 1
		printerr("FAIL: " + what)


func _run() -> void:
	var session: Node = root.get_node("Session")
	var images := {}
	for id in ["kessel_ridge", "linden_brook"]:
		session.setup.map = id
		var m: Node = load("res://match/match_mock.tscn").instantiate()
		root.add_child(m)
		for i in 30:
			await process_frame
		var src: Node = m.source
		var st: Dictionary = src.get_state()
		var expect: String = session.MAPS[id].name
		_check(st.map.name == expect, "%s: state map name is %s" % [id, expect])
		_check(st.map.id == id, "%s: state map id" % id)
		_check(MockMap.map_id == id, "%s: MockMap follows" % id)
		_check(st.points.size() == 5, "%s: five points" % id)
		var owned: Array = []
		for p in st.points:
			owned.append(p.owner)
		_check(owned.count(0) == 1 and owned.count(1) == 1, "%s: one point each" % id)
		_check(st.squads.size() >= 6, "%s: opening squads" % id)
		var roads := m.get_node("MockWorld").get_node_or_null("Roads")
		_check((roads != null) == (id == "linden_brook"), "%s: road mesh only on Linden Brook" % id)
		var img := MockMap.make_image(128)
		images[id] = img
		var found := false
		for y in 128:
			for x in 128:
				if absf(img.get_pixel(x, y).r - RoadStyle.MAP_COLOR.r) < 0.01 and absf(img.get_pixel(x, y).g - RoadStyle.MAP_COLOR.g) < 0.01:
					found = true
		_check(found == (id == "linden_brook"), "%s: road pixels in the map image only on Linden Brook" % id)
		# Run the match for a few game seconds; the mock must not error out.
		src.time_scale = 8.0
		for i in 120:
			await process_frame
		_check(src.get_state().tick > 0, "%s: ticks advance" % id)
		m.queue_free()
		await process_frame
	_check(images.kessel_ridge.get_data() != images.linden_brook.get_data(), "map images differ")
	print("test_mock_maps: %d/%d checks passed" % [_checks - _fails, _checks])
	quit(0 if _fails == 0 else 1)
