class_name MockMapLinden
extends RefCounted
## Presenter-side stand-in for "Linden Brook" (S16 map): see MockMap for the
## role. Point-symmetric through the centre like the real map: a shallow
## brook valley down the middle, a low ridge each side, bases west and
## east, C (worth 2) in the valley. Sim coordinates (x east, y north, 0..1024) are
## converted to Godot's (x east, z south, centred) by _g().

const POINTS := [
	{"id": "A", "pos": Vector2(0, -348)},
	{"id": "B", "pos": Vector2(-252, 92)},
	{"id": "C", "pos": Vector2(0, 0)},
	{"id": "D", "pos": Vector2(252, -92)},
	{"id": "E", "pos": Vector2(0, 348)},
]
const BASES := [Vector2(-440, 212), Vector2(440, -212)]
const WOODS := [
	{"c": Vector2(-42, 212), "r": 50.0},
	{"c": Vector2(42, -212), "r": 50.0},
	{"c": Vector2(-332, -88), "r": 45.0},
	{"c": Vector2(332, 88), "r": 45.0},
	{"c": Vector2(132, 368), "r": 40.0},
	{"c": Vector2(-132, -368), "r": 40.0},
	{"c": Vector2(28, 92), "r": 30.0},
	{"c": Vector2(-28, -92), "r": 30.0},
]
## Opening squads: [type, position] for player 0, mirrored for player 1.
const OPENING := [["rifle", Vector2(-260, 100)], ["rifle", Vector2(-200, 150)], ["mg", Vector2(-120, 60)],
	["engineer", Vector2(-380, 190)], ["mortar", Vector2(-410, 250)], ["rifle", Vector2(-40, 20)]]
## Points each side owns at the start (indices into POINTS): B and D.
const OWNED := [1, 3]
## Road polylines in sim coordinates and their width (m).
const ROADS_SIM := [
	{"pts": [[72, 300], [260, 420], [512, 512], [764, 604], [952, 724]], "w": 6.0},
	{"pts": [[512, 860], [470, 700], [512, 512], [554, 324], [512, 164]], "w": 6.0},
	{"pts": [[72, 300], [300, 200], [512, 164]], "w": 6.0},
	{"pts": [[952, 724], [724, 824], [512, 860]], "w": 6.0},
]


static func _g(p: Array) -> Vector2:
	return Vector2(float(p[0]) - 512.0, 512.0 - float(p[1]))


static func roads() -> Array:
	var out: Array = []
	for r in ROADS_SIM:
		var pts: Array = []
		for p in r.pts:
			pts.append(_g(p))
		out.append({"pts": pts, "w": r.w})
	return out


## Ground height (m): even under a half-turn, so the field is point-symmetric.
static func height(x: float, z: float) -> float:
	var west := x - (-232.7 - 0.0909 * z)
	var east := x - (232.7 - 0.0909 * z)
	var valley := x + 0.1406 * z
	var ridges := 10.0 * (exp(-pow(west / 130.0, 2.0)) + exp(-pow(east / 130.0, 2.0)))
	var brook := -6.0 * exp(-pow(valley / 70.0, 2.0))
	var roll := 2.5 * cos(x / 61.0) * cos(z / 47.0) + 1.5 * cos((x + 2.0 * z) / 83.0)
	return 12.0 + ridges + brook + roll
