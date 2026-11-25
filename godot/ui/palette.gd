class_name UiPalette
extends RefCounted
## Shared UI colours (mirrors ui/theme/frontline_theme.tres) and the squad
## class glyphs drawn on markers, cards and buttons.
##
## Team colours are relative to the local player (own blue, enemy red,
## neutral grey) so they read the same whichever faction is played; the
## faction colours are for the menu and the end screen only.

const BG := Color(0.078, 0.09, 0.098)
const PANEL := Color(0.098, 0.114, 0.125, 0.96)
const BORDER := Color(0.235, 0.275, 0.302)
const ACCENT := Color(0.851, 0.643, 0.255)
const TEXT := Color(0.91, 0.894, 0.847)
const TEXT_DIM := Color(0.608, 0.639, 0.651)
const OK := Color(0.55, 0.77, 0.43)
const WARN := Color(0.89, 0.70, 0.25)
const DANGER := Color(0.88, 0.38, 0.31)

const OWN := Color(0.37, 0.66, 0.83)
## S17 (S17-13): an ally's units, between own blue and neutral.
const ALLY := Color(0.45, 0.74, 0.45)
const ENEMY := Color(0.85, 0.38, 0.31)
const NEUTRAL := Color(0.65, 0.66, 0.64)

const FACTION_COLORS := {
	"germany": Color(0.50, 0.58, 0.65),
	"usa": Color(0.64, 0.58, 0.34),
}
const FACTION_NAMES := {"germany": "Germany", "usa": "USA"}
const FACTION_SHORT := {"germany": "GER", "usa": "USA"}

const SQUAD_NAMES := {
	"rifle": "Rifle squad",
	"mg": "MG team",
	"engineer": "Engineer squad",
	"mortar": "Mortar team",
	"at": "AT team",
	"crew": "Tank crew",
}


## S17: bit mask of the local player's team (him included), set by the
## match view from each frame's `allies`; 0 = no team (every 1v1).
static var ally_mask := 0


## Is `owner` the local player's ally (never the local player himself)?
static func is_ally(owner: int, local_player: int) -> bool:
	return owner >= 0 and owner != local_player and owner < 16 and (ally_mask >> owner) & 1 == 1


## Colour of a side as seen by `local_player`; -1 is neutral (S17: an
## ally's side is ALLY).
static func owner_color(owner: int, local_player: int) -> Color:
	if owner < 0:
		return NEUTRAL
	if owner == local_player:
		return OWN
	return ALLY if is_ally(owner, local_player) else ENEMY


static func faction_color(faction: String) -> Color:
	return FACTION_COLORS.get(faction, NEUTRAL)


static func mmss(seconds: float) -> String:
	var s := maxi(int(seconds), 0)
	return "%d:%02d" % [s / 60, s % 60]


## Draws a squad class glyph (a framed box with a class sign, in the spirit
## of map symbols) into `rect` on `ci`. Types: rifle, mg, engineer, mortar.
static func draw_glyph(ci: CanvasItem, type: String, rect: Rect2, color: Color, width := 2.0) -> void:
	var r := rect.grow(-width * 0.5)
	ci.draw_rect(r, Color(color, 0.18), true)
	ci.draw_rect(r, color, false, width)
	var c := r.get_center()
	match type:
		"rifle":
			ci.draw_line(r.position, r.end, color, width)
			ci.draw_line(Vector2(r.position.x, r.end.y), Vector2(r.end.x, r.position.y), color, width)
		"mg":
			ci.draw_line(r.position, r.end, color, width)
			ci.draw_line(Vector2(r.position.x, r.end.y), Vector2(r.end.x, r.position.y), color, width)
			ci.draw_line(Vector2(c.x, r.position.y + r.size.y * 0.1), Vector2(c.x, r.end.y - r.size.y * 0.1), color, width)
		"engineer":
			var y0 := r.position.y + r.size.y * 0.35
			var y1 := r.position.y + r.size.y * 0.65
			var x0 := r.position.x + r.size.x * 0.25
			var x1 := r.end.x - r.size.x * 0.25
			ci.draw_polyline(PackedVector2Array([Vector2(x0, y1), Vector2(x0, y0), Vector2(x1, y0), Vector2(x1, y1)]), color, width)
			ci.draw_line(Vector2(c.x, y0), Vector2(c.x, y1), color, width)
		"mortar":
			var rad := minf(r.size.x, r.size.y) * 0.16
			var base := Vector2(c.x, r.end.y - r.size.y * 0.22)
			ci.draw_arc(base, rad, 0.0, TAU, 16, color, width)
			ci.draw_line(base - Vector2(0, rad), Vector2(c.x, r.position.y + r.size.y * 0.15), color, width)
			ci.draw_line(Vector2(c.x, r.position.y + r.size.y * 0.15), Vector2(c.x - rad, r.position.y + r.size.y * 0.15 + rad), color, width)
			ci.draw_line(Vector2(c.x, r.position.y + r.size.y * 0.15), Vector2(c.x + rad, r.position.y + r.size.y * 0.15 + rad), color, width)
		"tank":
			# Armour: a track-shaped oval inside the frame.
			var hh := r.size.y * 0.22
			var hw := r.size.x * 0.3
			var rad := hh
			var pts := PackedVector2Array()
			for i in 9:
				var a := PI * 0.5 + PI * i / 8.0
				pts.append(Vector2(c.x - hw + rad, c.y) + Vector2(cos(a), sin(a)) * rad)
			for i in 9:
				var a := -PI * 0.5 + PI * i / 8.0
				pts.append(Vector2(c.x + hw - rad, c.y) + Vector2(cos(a), sin(a)) * rad)
			pts.append(pts[0])
			ci.draw_polyline(pts, color, width)
		"medic":
			# S12: a stretcher (no red cross: white armband and stretcher
			# only): the bed with a handle at each end and two cross bars.
			var sy0 := r.position.y + r.size.y * 0.36
			var sy1 := r.position.y + r.size.y * 0.64
			var sx0 := r.position.x + r.size.x * 0.3
			var sx1 := r.end.x - r.size.x * 0.3
			ci.draw_rect(Rect2(sx0, sy0, sx1 - sx0, sy1 - sy0), color, false, width)
			ci.draw_line(Vector2(r.position.x + r.size.x * 0.14, c.y), Vector2(sx0, c.y), color, width)
			ci.draw_line(Vector2(sx1, c.y), Vector2(r.end.x - r.size.x * 0.14, c.y), color, width)
			ci.draw_line(Vector2(c.x, sy0), Vector2(c.x, sy1), color, width * 0.6)
		"at":
			# Anti-armour: a chevron from the bottom corners to the top.
			ci.draw_polyline(PackedVector2Array([Vector2(r.position.x, r.end.y), Vector2(c.x, r.position.y + r.size.y * 0.12), Vector2(r.end.x, r.end.y)]), color, width)
		_:
			ci.draw_circle(c, minf(r.size.x, r.size.y) * 0.15, color)
