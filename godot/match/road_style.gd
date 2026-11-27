class_name RoadStyle
extends RefCounted
## Presentation tunables for roads (S16), in one place. The minimap colour
## for the real match is MINIMAP_ROAD_COLOR in crates/godot-bridge
## (match_node.rs); the mock minimap and menu preview use MAP_COLOR here.

## Scenery strip colour (packed earth) and the factor on the map's road
## width (1.0 = the width the map file gives; the sim marks that width).
const COLOR := Color(0.45, 0.38, 0.27)
const WIDTH_SCALE := 1.0
## Strip lift above the terrain, so it does not z-fight (metres).
const LIFT_M := 0.12
## Sample spacing along a road so the strip follows the ground (metres).
const STEP_M := 4.0
## Mock minimap and menu preview road colour.
const MAP_COLOR := Color(0.62, 0.55, 0.40)


## One flat strip mesh over the terrain for `roads` ([{pts: Array of
## Vector2 (x, z), w: width m}]); `height_fn(x, z)` gives the ground height.
## Returns null when there are no roads.
static func build(roads: Array, height_fn: Callable) -> MeshInstance3D:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var quads := 0
	for r in roads:
		var pts: Array = r.pts
		var half := float(r.w) * WIDTH_SCALE * 0.5
		var prev_l := Vector3.ZERO
		var prev_r := Vector3.ZERO
		var have := false
		for i in pts.size() - 1:
			var a: Vector2 = pts[i]
			var b: Vector2 = pts[i + 1]
			var dir := (b - a).normalized()
			var side := Vector2(-dir.y, dir.x) * half
			var n := maxi(1, int(ceil(a.distance_to(b) / STEP_M)))
			for k in n + 1:
				if k == 0 and have:
					continue
				var p := a.lerp(b, float(k) / n)
				var l := p + side
				var rr := p - side
				var vl := Vector3(l.x, float(height_fn.call(l.x, l.y)) + LIFT_M, l.y)
				var vr := Vector3(rr.x, float(height_fn.call(rr.x, rr.y)) + LIFT_M, rr.y)
				if have:
					st.add_vertex(prev_l)
					st.add_vertex(prev_r)
					st.add_vertex(vl)
					st.add_vertex(prev_r)
					st.add_vertex(vr)
					st.add_vertex(vl)
					quads += 1
				prev_l = vl
				prev_r = vr
				have = true
	if quads == 0:
		return null
	st.generate_normals()
	var mat := StandardMaterial3D.new()
	mat.albedo_color = COLOR
	mat.roughness = 1.0
	mat.cull_mode = BaseMaterial3D.CULL_DISABLED
	var inst := MeshInstance3D.new()
	inst.name = "Roads"
	inst.mesh = st.commit()
	inst.material_override = mat
	inst.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	return inst
