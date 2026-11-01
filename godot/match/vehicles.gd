class_name Vehicles
extends RefCounted
## S3 vehicle constants and helpers shared by the presenter pieces (the
## renderer, effects, HUD cards, markers, picking): flag bits, seat and
## component order, texts, the card silhouette and the ground footprint.
## The state schema is in godot/docs/match_state.md "Vehicles (S3)".

## vehicles[].flags bits (snapshot.rs `vehicle_flags`).
const MOVING := 1
const REVERSING := 2
const BOGGED := 4
const IMMOBILISED := 8
const BURNING := 16
const REPAIRING := 32
const CROSSING := 64
const WRECK := 128
const KNOCKED_OUT := 256
## S3c: the crew bailed out (owner -1, `last_owner` set); someone is
## climbing in (re-crew).
const ABANDONED := 512
const BOARDING := 1024

## Crew seats in pip order (D G C L R).
const SEATS := ["driver", "gunner", "commander", "loader", "radio_mg"]
const SEAT_LETTERS := ["D", "G", "C", "L", "R"]
const SEAT_NAMES := {"driver": "Driver", "gunner": "Gunner", "commander": "Commander", "loader": "Loader", "radio_mg": "Radio operator"}
## Components in snapshot order; STRIP is the card's component strip.
const COMPONENTS := ["engine", "track_l", "track_r", "gun", "ring", "ammo_rack", "transmission"]
const STRIP := ["engine", "track_l", "track_r", "gun", "ring"]
const COMPONENT_NAMES := {"engine": "Engine", "track_l": "Left track", "track_r": "Right track", "gun": "Main gun", "ring": "Turret ring", "ammo_rack": "Ammo rack", "transmission": "Transmission"}
const COMPONENT_SHORT := {"engine": "Eng", "track_l": "Trk L", "track_r": "Trk R", "gun": "Gun", "ring": "Ring", "ammo_rack": "Rack", "transmission": "Trans"}
## Below this fraction a component reads as damaged (S3 spec: < 50 HP of
## 100; the gun and ring have 80, so this is a presenter threshold).
const DAMAGED := 0.5
const STATE_TEXT := {
	"wreck": "Wreck", "abandoned": "Abandoned", "burning": "Burning", "bogged": "Bogged", "immobilised": "Immobilised",
	"repairing": "Repairing", "crossing": "Crossing", "reversing": "Reversing", "moving": "Moving",
	"idle": "Holding",
}
## Badge states drawn in the alarm (accent) colour.
const ALARM_STATES := ["burning", "bogged", "immobilised", "abandoned"]
## S3c: why a crew bailed out (vehicles[].abandon_cause), for hover texts.
const ABANDON_TEXT := {
	"ammo_fire": "ammo fire", "engine_fire": "engine fire", "crew": "crew losses",
	"useless": "gun and tracks out", "under_fire": "disabled under fire", "order": "ordered out",
}
## S3b: "apcr" (the special round) only on guns that carry it.
const AMMO_CHOICES := ["auto", "ap", "apcr", "he"]
const AMMO_TEXT := {"auto": "Auto", "ap": "AP", "apcr": "APCR", "he": "HE"}
## Hit marker words by armour event kind (spec "Hit markers").
const HIT_WORDS := {"ricochet": "Ricochet", "no_pen": "No pen", "pen": "Penetrated", "crew_hit": "Crew hit"}
## Placeholder paint by faction, mixed toward the side colour for the team
## tint; glacis slope for the mesh look (not in the state: a Sherman's
## steep glacis, a Panzer IV's near-vertical plate).
const PAINT := {"germany": Color(0.46, 0.43, 0.32), "usa": Color(0.30, 0.33, 0.22)}
## Side colours for the tint (darker than the HUD's, so painted steel reads).
const TEAM_OWN := Color(0.22, 0.36, 0.56)
const TEAM_ENEMY := Color(0.58, 0.24, 0.20)
const GLACIS_DEG := {"germany": 10.0, "usa": 47.0}
const TEAM_MIX := 0.42


## Next ammo choice for the N key: Auto → AP → (APCR, on a gun that has
## it) → HE → Auto.
static func next_ammo(choice: String, has_apcr := false) -> String:
	var order: Array = AMMO_CHOICES if has_apcr else AMMO_CHOICES.filter(func(c): return c != "apcr")
	var i := order.find(choice)
	return order[(i + 1) % order.size()]


## Whether a vehicle's state lists the special round (S3b: "apcr" in its
## ammo counts).
static func has_apcr(v: Dictionary) -> bool:
	return (v.get("ammo", {}) as Dictionary).has("apcr")


static func has(v: Dictionary, bit: int) -> bool:
	return (int(v.get("flags", 0)) & bit) != 0


static func is_wreck(v: Dictionary) -> bool:
	if has(v, WRECK):
		return true
	return int(v.get("owner", -1)) < 0 and not is_abandoned(v)


## S3c: abandoned by its crew (nobody's, can be re-crewed).
static func is_abandoned(v: Dictionary) -> bool:
	return bool(v.get("abandoned", false)) or has(v, ABANDONED)


## S3c hover text: "Abandoned: Panzer IV (gun out, left track out)".
static func abandoned_text(v: Dictionary, vt: Dictionary) -> String:
	var parts := PackedStringArray()
	for c in v.get("parts_out", PackedStringArray()):
		parts.append("%s out" % String(COMPONENT_NAMES.get(String(c), String(c))).to_lower())
	var why := String(ABANDON_TEXT.get(String(v.get("abandon_cause", "")), ""))
	var inside := int(v.get("inside", 0))
	var tail := ", ".join(parts) if not parts.is_empty() else why
	var text := "Abandoned: %s" % short_name(vt, String(v.get("name", "vehicle")))
	if tail != "":
		text += " (%s)" % tail
	if inside > 0:
		text += ", %d still inside" % inside
	if has(v, BOARDING):
		text += ", crew boarding"
	return text


## Badge [text, alarm] for a vehicle's `state`.
static func badge(v: Dictionary) -> Array:
	var st := String(v.get("state", "idle"))
	return [STATE_TEXT.get(st, st.capitalize()), st in ALARM_STATES]


## Component fraction 0..1 (1 when the key is missing: enemies, wrecks).
static func component(v: Dictionary, name: String) -> float:
	return float(v.get("components", {}).get(name, 1.0))


## Short display name of a vehicle type ("Panzer IV").
static func short_name(vt: Dictionary, fallback := "") -> String:
	return String(vt.get("short", vt.get("name", fallback)))


## Ground footprint corners (Godot space, at the hull origin's height),
## front-left, front-right, rear-right, rear-left. Hull forward is −Z
## turned by `yaw`.
static func footprint(pos: Vector3, yaw: float, vt: Dictionary, pad := 0.0) -> PackedVector3Array:
	var hl := float(vt.get("hull_length", 5.8)) * 0.5 + pad
	var hw := float(vt.get("hull_width", 2.8)) * 0.5 + pad
	var fwd := Vector3(-sin(yaw), 0.0, -cos(yaw))
	var right := Vector3(cos(yaw), 0.0, -sin(yaw))
	return PackedVector3Array([
		pos + fwd * hl - right * hw, pos + fwd * hl + right * hw,
		pos - fwd * hl + right * hw, pos - fwd * hl - right * hw,
	])


## Hull yaw that faces the Godot ground direction (dx, dz).
static func yaw_toward(dx: float, dz: float) -> float:
	return atan2(-dx, -dz)


## Side-view silhouette from the type's box sizes (no image files), facing
## right, fitted into `r`: tracks, hull, turret and barrel.
static func silhouette(ci: CanvasItem, r: Rect2, vt: Dictionary, col: Color) -> void:
	var hl := float(vt.get("hull_length", 5.8))
	var hh := float(vt.get("hull_height", 1.36))
	var hg := float(vt.get("hull_ground", 0.4))
	var tx := float(vt.get("turret_x", 0.2))
	var tb := float(vt.get("turret_base", 1.76))
	var tl := float(vt.get("turret_length", 2.3))
	var th := float(vt.get("turret_height", 0.9))
	var to := float(vt.get("turret_offset_x", 0.0))
	var gl := float(vt.get("gun_length", 4.5))
	var gh := float(vt.get("gun_height", 0.45))
	# Extent in metres: from the hull rear to the muzzle, ground to roof.
	var x0 := -hl * 0.5
	var x1 := maxf(hl * 0.5, tx + gl)
	var top := maxf(hg + hh, tb + th)
	var k := minf(r.size.x / (x1 - x0), r.size.y / top)
	var org := Vector2(r.position.x + (r.size.x - (x1 - x0) * k) * 0.5 - x0 * k, r.end.y - (r.size.y - top * k) * 0.5)
	var p := func(x: float, y: float) -> Vector2: return org + Vector2(x * k, -y * k)
	# Tracks: a rounded band along the bottom.
	var track := PackedVector2Array([p.call(-hl * 0.5, hg * 0.9), p.call(-hl * 0.4, 0.0), p.call(hl * 0.4, 0.0), p.call(hl * 0.5, hg * 0.9), p.call(hl * 0.48, hg + hh * 0.45), p.call(-hl * 0.49, hg + hh * 0.45)])
	ci.draw_colored_polygon(track, Color(col, 0.55))
	# Hull with a sloped front plate.
	var hull := PackedVector2Array([p.call(-hl * 0.48, hg + hh * 0.3), p.call(hl * 0.48, hg + hh * 0.3), p.call(hl * 0.5, hg + hh * 0.55), p.call(hl * 0.42, hg + hh), p.call(-hl * 0.47, hg + hh)])
	ci.draw_colored_polygon(hull, col)
	# Turret.
	var c := tx + to
	var tur := PackedVector2Array([p.call(c - tl * 0.5, tb), p.call(c + tl * 0.5, tb), p.call(c + tl * 0.42, tb + th), p.call(c - tl * 0.45, tb + th)])
	ci.draw_colored_polygon(tur, col)
	ci.draw_line(p.call(c + tl * 0.45, tb + gh), p.call(tx + gl, tb + gh), col, maxf(1.5, 0.16 * k))
