class_name MockMap
extends RefCounted
## Presenter-side stand-in for "Kessel Ridge" (S1 map): an analytic height
## function, capture points, bases and woods in Godot world space (map
## centred on the origin, x east, z south, metres; §5 / godot-bridge
## present.rs). Used by MockMatchSource, the menu preview and the minimap
## until the real map comes from mods/ww2/defs/maps/kessel_ridge.toml
## through the bridge. Floats are fine here: nothing feeds the sim.

const SIZE_M := 1024.0
const HALF := SIZE_M * 0.5

## The map the mock currently plays (S16): use() switches it. Tests and the
## menu preview call use() before reading anything below.
static var map_id := "kessel_ridge"
static var POINTS: Array = KESSEL_POINTS
static var BASES: Array = KESSEL_BASES
static var WOODS: Array = KESSEL_WOODS
## Road polylines [{pts: Array[Vector2], w: metres}] in Godot coordinates.
static var ROADS: Array = []
## Opening squads for player 0 ([type, pos]; player 1 mirrors them through
## the centre), and the points each side owns at the start.
static var OPENING: Array = []
static var OWNED: Array = [0, 4]

const KESSEL_POINTS := [
	{"id": "A", "pos": Vector2(-300, 250)},
	{"id": "B", "pos": Vector2(-260, -210)},
	{"id": "C", "pos": Vector2(0, 0)},
	{"id": "D", "pos": Vector2(250, 220)},
	{"id": "E", "pos": Vector2(300, -250)},
]
## Base zones: player 0 south-west, player 1 north-east.
const KESSEL_BASES := [Vector2(-420, 420), Vector2(420, -420)]
const BASE_RADIUS := 60.0
const KESSEL_WOODS := [
	{"c": Vector2(-120, 60), "r": 70.0},
	{"c": Vector2(140, -90), "r": 60.0},
	{"c": Vector2(-330, -40), "r": 55.0},
	{"c": Vector2(330, 40), "r": 50.0},
	{"c": Vector2(60, 330), "r": 45.0},
	{"c": Vector2(-80, -340), "r": 45.0},
]


## Switches the mock to map `id` (unknown ids fall back to Kessel Ridge).
static func use(id: String) -> void:
	if id == "linden_brook":
		map_id = id
		POINTS = MockMapLinden.POINTS
		BASES = MockMapLinden.BASES
		WOODS = MockMapLinden.WOODS
		ROADS = MockMapLinden.roads()
		OPENING = MockMapLinden.OPENING
		OWNED = MockMapLinden.OWNED
	else:
		map_id = "kessel_ridge"
		POINTS = KESSEL_POINTS
		BASES = KESSEL_BASES
		WOODS = KESSEL_WOODS
		ROADS = []
		OPENING = [["rifle", Vector2(-250, 170)], ["rifle", Vector2(-215, -150)], ["mg", Vector2(-120, 80)],
			["engineer", Vector2(-285, 228)], ["mortar", Vector2(-330, 310)], ["rifle", Vector2(-60, 40)]]
		OWNED = [0, 4]


static func map_name() -> String:
	return "Linden Brook" if map_id == "linden_brook" else "Kessel Ridge"


## True when a road lies within its half width of `p`.
static func on_road(p: Vector2) -> bool:
	for r in ROADS:
		var pts: Array = r.pts
		for i in pts.size() - 1:
			var q: Vector2 = Geometry2D.get_closest_point_to_segment(p, pts[i], pts[i + 1])
			if q.distance_to(p) <= float(r.w) * 0.5:
				return true
	return false


## Ground height (m) at Godot (x, z): a ridge running NW→SE through the
## centre, a shallow valley south-west of it, and rolling noise.
static func height(x: float, z: float) -> float:
	if map_id == "linden_brook":
		return MockMapLinden.height(x, z)
	var d := (x - z) * 0.70710678  # distance across the ridge line x = z
	var along := (x + z) * 0.70710678
	var ridge := 16.0 * exp(-pow(d / 110.0, 2.0)) * (0.8 + 0.2 * cos(along / 140.0))
	var valley := -5.0 * exp(-pow((d + 240.0) / 90.0, 2.0))
	var roll := 2.5 * sin(x / 61.0) * cos(z / 47.0) + 1.5 * sin((x + 2.0 * z) / 83.0)
	return ridge + valley + roll


static func in_woods(p: Vector2) -> bool:
	for w in WOODS:
		if p.distance_to(w.c) < w.r:
			return true
	return false


## Shaded top-down image (north up) for the minimap and the menu preview.
static func make_image(px: int) -> Image:
	var img := Image.create(px, px, false, Image.FORMAT_RGB8)
	var step := SIZE_M / px
	var light := Vector3(-0.5, 0.75, -0.45).normalized()
	for j in px:
		for i in px:
			var x := -HALF + (i + 0.5) * step
			var z := -HALF + (j + 0.5) * step
			var h := height(x, z)
			var hx := height(x + step, z) - h
			var hz := height(x, z + step) - h
			var n := Vector3(-hx, step, -hz).normalized()
			var shade := clampf(n.dot(light) * 1.25, 0.35, 1.15)
			var base := Color(0.36, 0.42, 0.26).lerp(Color(0.55, 0.52, 0.36), clampf((h + 4.0) / 22.0, 0.0, 1.0))
			if in_woods(Vector2(x, z)):
				base = Color(0.20, 0.27, 0.17)
			var c := base * shade
			# 4 m contour lines.
			if fposmod(h, 4.0) < 0.35:
				c = c.darkened(0.18)
			if on_road(Vector2(x, z)) or _near_road(Vector2(x, z), step * 0.7):
				c = RoadStyle.MAP_COLOR
			img.set_pixel(i, j, Color(c.r, c.g, c.b))
	return img


static func _near_road(p: Vector2, r: float) -> bool:
	for rd in ROADS:
		var pts: Array = rd.pts
		for i in pts.size() - 1:
			if Geometry2D.get_closest_point_to_segment(p, pts[i], pts[i + 1]).distance_to(p) <= r:
				return true
	return false
