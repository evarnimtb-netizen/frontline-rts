extends Node3D
## S12 renderer, over the sim's own bodies: a pip above every wounded man
## (red: bleeding, amber: being bandaged, white: bandaged, green: treated;
## a tan second pip low on him for a leg wound), a pale groundsheet under
## every downed man with a pip above him (red: bleeding out, white:
## stabilised) and a stretcher (two poles) under one being carried, a white
## armband on each medic, a cot for each bed of an aid post (empty ones
## paler) and over a built post one mark per medic it needs (white: there).
## White armband and stretcher only: no red cross (owner decision S12-2).
## One pooled MeshInstance3D per box (dozens at most), unshaded so the
## signs read in shade; drawn only while the state shows wounds
## (`MatchView.s12_on()` is not asked here: no wound, no downed man, no
## medic and no bed means nothing is drawn).

const BLEED := Color(0.92, 0.25, 0.20)
const DRESSING := Color(0.95, 0.94, 0.88)
const BANDAGING := Color(0.93, 0.70, 0.22)
const TREATED := Color(0.45, 0.80, 0.42)
const LEGS := Color(0.74, 0.62, 0.40)
const SHEET := Color(0.70, 0.68, 0.58)
const POLE := Color(0.42, 0.33, 0.22)
const ARMBAND := Color(1.0, 1.0, 1.0)
const COT := Color(0.62, 0.60, 0.50)
const COT_EMPTY := Color(0.48, 0.47, 0.42)
const STAFF_MISSING := Color(0.40, 0.40, 0.40)
## Pip height over the feet by stance (m).
const PIP_LIFT := {"stand": 2.15, "crouch": 1.6, "prone": 0.95}

var _pool: Array[MeshInstance3D] = []
var _mats := {}
var _n := 0
## Boxes drawn last frame, by kind (tests).
var pips := 0
var sheets := 0
var poles := 0
var armbands := 0
var cots := 0
## Empty cots, and the post's staff marks (all, and those lit).
var empty_cots := 0
var staff_marks := 0
var staff_lit := 0
## Last frame's pip colours of the wounded men by id (tests).
var man_pip: Dictionary = {}
## Last frame's pip colours of the downed men by id (tests).
var down_pip: Dictionary = {}


func update_state(s: Dictionary) -> void:
	_n = 0
	pips = 0
	sheets = 0
	poles = 0
	armbands = 0
	cots = 0
	empty_cots = 0
	staff_marks = 0
	staff_lit = 0
	man_pip.clear()
	down_pip.clear()
	var medic_squads := {}
	for sq in s.get("squads", []):
		if (sq.get("tags", PackedStringArray()) as PackedStringArray).has("medic"):
			medic_squads[int(sq.id)] = true
	for so in s.get("soldiers", []):
		var p: Vector3 = so.pos
		var w: Variant = so.get("wound")
		if w is Dictionary:
			var col := _wound_colour(w)
			man_pip[int(so.id)] = col
			_box(p + Vector3(0, PIP_LIFT.get(so.get("stance", "stand"), 2.15), 0), Vector3(0.22, 0.22, 0.22), 0.0, col)
			pips += 1
			if bool(w.get("legs", false)):
				_box(p + Vector3(0.18, 0.32, 0), Vector3(0.14, 0.14, 0.14), 0.0, LEGS)
				pips += 1
		if medic_squads.has(int(so.get("squad", -1))):
			_box(p + Vector3(0.23, 1.28 if so.get("stance", "stand") == "stand" else 0.8, 0), Vector3(0.1, 0.16, 0.3), 0.0, ARMBAND)
			armbands += 1
	for c in s.get("casualties", []):
		var cp: Vector3 = c.pos
		var yaw := float(int(c.id) % 628) * 0.01
		var carried: bool = not (c.get("carriers", PackedInt32Array()) as PackedInt32Array).is_empty()
		_box(cp + Vector3(0, 0.03, 0), Vector3(0.95, 0.05, 2.05), yaw, SHEET)
		sheets += 1
		if carried:
			var side := Vector3(cos(yaw), 0, -sin(yaw)) * 0.5
			for k in [-1.0, 1.0]:
				_box(cp + Vector3(0, 0.12, 0) + side * k, Vector3(0.06, 0.06, 2.4), yaw, POLE)
				poles += 1
		var own := int(c.owner) == int(s.get("local_player", 0))
		var col := DRESSING if bool(c.get("stabilised", false)) else BLEED
		if not own:
			col = Color(0.6, 0.6, 0.6)
		down_pip[int(c.id)] = col
		_box(cp + Vector3(0, 1.0, 0), Vector3(0.26, 0.26, 0.26), 0.0, col)
		pips += 1
	for st in s.get("structures", []):
		var beds: Array = st.get("beds", [])
		# S12 F2 (bridge gap B3): every bed of the def, empty ones paler.
		var n := maxi(beds.size(), int(st.get("beds_max", 0)))
		var yaw2 := float(st.get("yaw", 0.0))
		var f := Vector2(cos(yaw2), sin(yaw2))
		var l := Vector2(-f.y, f.x)
		for i in n:
			var at := l * (1.2 * (i - (n - 1) * 0.5)) + f * 2.2
			var pos: Vector3 = st.pos + Vector3(at.x, 0.12, -at.y)
			if i >= beds.size():
				_box(pos, Vector3(0.7, 0.12, 1.9), yaw2, COT_EMPTY)
				empty_cots += 1
				continue
			_box(pos, Vector3(0.7, 0.18, 1.9), yaw2, COT)
			cots += 1
			if bool(beds[i].get("evac", false)):
				_box(pos + Vector3(0, 0.8, 0), Vector3(0.2, 0.2, 0.2), 0.0, DRESSING)
				pips += 1
		# B3: staff over staff_min, one armband mark per medic the post
		# needs (white: there, grey: missing), over a built post.
		if st.get("kind", "") == "aid" and bool(st.get("built", false)):
			var need := int(st.get("staff_min", 0))
			var have := int(st.get("staff", 0))
			for k in need:
				var mp: Vector3 = st.pos + Vector3(0.3 * (k - (need - 1) * 0.5), 2.4, 0)
				_box(mp, Vector3(0.18, 0.18, 0.18), 0.0, ARMBAND if k < have else STAFF_MISSING)
				staff_marks += 1
				if k < have:
					staff_lit += 1
	for k in range(_n, _pool.size()):
		_pool[k].visible = false


func _wound_colour(w: Dictionary) -> Color:
	if bool(w.get("treated", false)):
		return TREATED
	if bool(w.get("bandaging", false)):
		return BANDAGING
	if bool(w.get("bandaged", false)):
		return DRESSING
	return BLEED


func _box(p: Vector3, size: Vector3, yaw: float, col: Color) -> void:
	while _pool.size() <= _n:
		var mi := MeshInstance3D.new()
		mi.mesh = BoxMesh.new()
		mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		add_child(mi)
		_pool.append(mi)
	var mi := _pool[_n]
	_n += 1
	(mi.mesh as BoxMesh).size = size
	mi.position = p
	mi.rotation = Vector3(0, yaw, 0)
	mi.material_override = _mat(col)
	mi.visible = true


func _mat(col: Color) -> StandardMaterial3D:
	var key := col.to_html()
	if _mats.has(key):
		return _mats[key]
	var m := StandardMaterial3D.new()
	m.albedo_color = col
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	_mats[key] = m
	return m


## Boxes drawn last frame.
func drawn() -> int:
	return _n
