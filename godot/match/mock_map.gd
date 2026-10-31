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

const POINTS := [
	{"id": "A", "pos": Vector2(-300, 250)},
	{"id": "B", "pos": Vector2(-260, -210)},
	{"id": "C", "pos": Vector2(0, 0)},
	{"id": "D", "pos": Vector2(250, 220)},
	{"id": "E", "pos": Vector2(300, -250)},
]
## Base zones: player 0 south-west, player 1 north-east.
const BASES := [Vector2(-420, 420), Vector2(420, -420)]
const BASE_RADIUS := 60.0
const WOODS := [
	{"c": Vector2(-120, 60), "r": 70.0},
	{"c": Vector2(140, -90), "r": 60.0},
	{"c": Vector2(-330, -40), "r": 55.0},
	{"c": Vector2(330, 40), "r": 50.0},
	{"c": Vector2(60, 330), "r": 45.0},
	{"c": Vector2(-80, -340), "r": 45.0},
]


## Ground height (m) at Godot (x, z): a ridge running NW→SE through the
## centre, a shallow valley south-west of it, and rolling noise.
static func height(x: float, z: float) -> float:
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
			img.set_pixel(i, j, Color(c.r, c.g, c.b))
	return img
