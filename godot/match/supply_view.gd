class_name SupplyView
extends Node3D
## S6 renderer: the state's `structures` (HQ and dumps: each def box as a
## plain box, stock crates olive, the radio box dark grey; a structure
## still being dug shows its boxes as faint ghosts) and set-down `crates`
## (small boxes in the side colour's tint). Carried crates ride on the
## carrier's back. One MeshInstance3D per box, pooled; at most a few dozen
## per match (the rules' caps), so no MultiMesh.

const STOCK := Color(0.36, 0.38, 0.24)
const RADIO := Color(0.22, 0.23, 0.22)
const CRATE := Color(0.42, 0.36, 0.24)
const CRATE_SIZE := Vector3(0.7, 0.35, 0.4)

var _pool: Array[MeshInstance3D] = []
var _mats := {}
## Boxes drawn last frame (tests).
var drawn := 0


func update_state(s: Dictionary) -> void:
	var n := 0
	var local := int(s.get("local_player", 0))
	for st in s.get("structures", []):
		var pos: Vector3 = st.pos
		var yaw := float(st.get("yaw", 0.0))
		var built := bool(st.get("built", false))
		for b in st.get("boxes", []):
			if int(b.get("pct", 100)) <= 0:
				continue
			var size: Vector3 = b.size
			var off: Vector2 = b.offset
			# Sim yaw from +x counter-clockwise seen from above; Godot's
			# ground is x right, -z the sim's +y.
			var f := Vector2(cos(yaw), sin(yaw))
			var l := Vector2(-f.y, f.x)
			var at := f * off.x + l * off.y
			var p := pos + Vector3(at.x, size.y * 0.5, -at.y)
			var col := RADIO if b.part == "radio" else STOCK
			var alpha := 1.0 if built else 0.3
			_box(n, p, size, yaw, _tint(col, int(st.owner), local), alpha)
			n += 1
	for c in s.get("crates", []):
		if int(c.get("carrier", -1)) != -1:
			continue
		var cp: Vector3 = c.pos
		_box(n, cp + Vector3(0, CRATE_SIZE.y * 0.5, 0), CRATE_SIZE, 0.0, _tint(CRATE, int(c.owner), local), 1.0)
		n += 1
	for k in range(n, _pool.size()):
		_pool[k].visible = false
	drawn = n


func _tint(base: Color, owner: int, local: int) -> Color:
	return base.lerp(UiPalette.owner_color(owner, local), 0.18)


func _box(k: int, p: Vector3, size: Vector3, yaw: float, col: Color, alpha: float) -> void:
	while _pool.size() <= k:
		var mi := MeshInstance3D.new()
		mi.mesh = BoxMesh.new()
		add_child(mi)
		_pool.append(mi)
	var mi := _pool[k]
	(mi.mesh as BoxMesh).size = size
	mi.position = p
	mi.rotation = Vector3(0, yaw, 0)
	mi.material_override = _mat(col, alpha)
	mi.visible = true


func _mat(col: Color, alpha: float) -> StandardMaterial3D:
	var key := "%s/%.2f" % [col.to_html(), alpha]
	if _mats.has(key):
		return _mats[key]
	var m := StandardMaterial3D.new()
	m.albedo_color = Color(col, alpha)
	m.roughness = 0.9
	if alpha < 1.0:
		m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	_mats[key] = m
	return m
