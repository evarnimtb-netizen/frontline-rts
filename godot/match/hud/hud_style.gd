class_name HudStyle
extends RefCounted
## HUD v2 theme constants (docs/slices/S1b-hud.md, mocks/gen_hud_v2.py):
## font sizes, colours, translucency, the two layout metric sets, and the
## drawing helpers the HUD pieces share (pips, hatching, dashed boxes,
## small glyphs). Every HUD v2 piece takes its sizes and colours from here.
##
## Font floor: FONT_SMALL (13 px) only for hotkey letters, times and badges;
## FONT (14 px) for everything else. Sizes are HUD pixels, which MatchView's
## HUD root keeps at one physical pixel or more (see hud_layout.gd).

const FONT_SMALL := 13
const FONT := 14

## One accent colour for the whole HUD.
const ACCENT := Color(0.878, 0.678, 0.290)  # #E0AD4A
const TEXT := Color(0.925, 0.933, 0.902)  # #ECEEE6
const DIM := Color(0.765, 0.788, 0.733)  # #C3C9BB
const OFF_TEXT := Color(0.925, 0.933, 0.902, 0.62)
const OFF_ACCENT := Color(0.878, 0.678, 0.290, 0.62)
const OWN := Color(0.498, 0.690, 0.847)  # #7FB0D8
const ENEMY := Color(0.812, 0.478, 0.416)  # #CF7A6A
const NEUTRAL := Color(1, 1, 1, 0.10)
const DANGER := Color(0.90, 0.36, 0.30)
const PIP_BRIGHT := Color(0.965, 0.965, 0.941)
const PIP_MID := Color(0.925, 0.933, 0.902, 0.40)
const PIP_LINE := Color(0.925, 0.933, 0.902, 0.85)

## Backgrounds (S1b-hud.md "Translucency"). There is no full-width slab.
const BG_TOP := Color(12.0 / 255, 16.0 / 255, 18.0 / 255, 0.55)
const BG_PANEL := Color(14.0 / 255, 18.0 / 255, 20.0 / 255, 0.46)
const BG_BUTTON := Color(14.0 / 255, 18.0 / 255, 20.0 / 255, 0.40)
const BG_CALLIN := Color(14.0 / 255, 18.0 / 255, 20.0 / 255, 0.38)
const BG_ABILITY := Color(52.0 / 255, 40.0 / 255, 16.0 / 255, 0.50)
const BG_TAB_ON := Color(14.0 / 255, 18.0 / 255, 20.0 / 255, 0.70)
const BG_CHIP := Color(1, 1, 1, 0.10)
const HOVER := Color(1, 1, 1, 0.08)
const ROW_SELECTED := Color(1, 1, 1, 0.15)
## Bottom shade: a gradient up to this alpha over metrics.fade px.
const SHADE := Color(0.047, 0.063, 0.063, 0.26)
const OFF_MAP := Color(0.165, 0.192, 0.153, 0.85)
const OFF_MAP_LINE := Color(0.333, 0.376, 0.298, 0.9)

## Layout metrics, from the mock generator's P1600 / P1280. "compact" is
## used when the HUD space is under COMPACT_BELOW px wide (1280x800 and the
## Steam Deck).
const COMPACT_BELOW := 1440.0
const NORMAL := {
	"compact": false,
	"gutter": 12.0,  # left/right screen margin
	"tb": 28.0,  # top bar height
	"chip_w": 44.0,
	"ticket_w": 100.0,
	"mm": 250.0,  # minimap side / disc diameter
	"feed_w": 300.0,
	"feed_line": 24.0,
	"base_gap": 10.0,  # console bottom margin
	"hc": 130.0,  # console height (one top, one baseline)
	"btn_w": 68.0,
	"btn_gap": 4.0,
	"detail_w": 300.0,
	"cards_per_row": 5,
	"card_gap": 6.0,
	"tab_h": 22.0,
	"pip": 13.0,
	"pip_gap": 3.0,
	"callin_w": 120.0,
	"tile_h": 40.0,
	"ci_gap": 10.0,
	"ci_head": 20.0,
	"ci_q": 26.0,
	"fade": 170.0,
}
const COMPACT := {
	"compact": true,
	"gutter": 12.0,
	"tb": 28.0,
	"chip_w": 40.0,
	"ticket_w": 60.0,
	"mm": 200.0,
	"feed_w": 260.0,
	"feed_line": 24.0,
	"base_gap": 10.0,
	"hc": 124.0,
	"btn_w": 62.0,
	"btn_gap": 4.0,
	"detail_w": 262.0,
	"cards_per_row": 3,
	"card_gap": 6.0,
	"tab_h": 22.0,
	"pip": 12.0,
	"pip_gap": 3.0,
	"callin_w": 120.0,
	"tile_h": 38.0,
	"ci_gap": 8.0,
	"ci_head": 18.0,
	"ci_q": 24.0,
	"fade": 160.0,
}

const _BOLD_PATH := "res://ui/fonts/Barlow-SemiBold.ttf"

static var _bold: Font
static var _boxes := {}
static var _tabular := {}


## The semibold HUD font (titles, numbers, hotkeys).
static func bold() -> Font:
	if _bold == null:
		_bold = load(_BOLD_PATH)
	return _bold


## `font` with tabular figures (OpenType tnum): every digit as wide as "0",
## so a number that changes in place keeps its width. Barlow's proportional
## "1" is 3 px narrower than the other digits at FONT.
static func tabular(font: Font) -> Font:
	var f: Font = _tabular.get(font)
	if f == null:
		var fv := FontVariation.new()
		fv.base_font = font
		fv.opentype_features = {TextServerManager.get_primary_interface().name_to_tag("tnum"): 1}
		f = fv
		_tabular[font] = f
	return f


static func text_w(font: Font, text: String, font_size: int) -> float:
	return font.get_string_size(text, HORIZONTAL_ALIGNMENT_LEFT, -1, font_size).x


## Draws `text` with its baseline at `pos`; align: -1 left (pos is the
## start), 0 centred on pos.x, 1 right (pos is the end).
static func text(ci: CanvasItem, font: Font, pos: Vector2, s: String, font_size: int, col: Color, align := -1) -> void:
	var x := pos.x
	if align != -1:
		var w := text_w(font, s, font_size)
		x -= w if align == 1 else w * 0.5
	ci.draw_string(font, Vector2(x, pos.y), s, HORIZONTAL_ALIGNMENT_LEFT, -1, font_size, col)


## `s` shortened with an ellipsis to fit `width` px (only after layout has
## grown its box as far as it may).
static func fit(font: Font, s: String, width: float, font_size: int) -> String:
	if text_w(font, s, font_size) <= width:
		return s
	var t := s
	while t.length() > 1 and text_w(font, t + "…", font_size) > width:
		t = t.left(t.length() - 1)
	return t + "…"


## Filled rounded box, optionally with a border. Style boxes are cached.
static func box(ci: CanvasItem, r: Rect2, col: Color, radius := 4, border := Color(0, 0, 0, 0), border_w := 0) -> void:
	var key := "%s|%d|%s|%d" % [col.to_html(), radius, border.to_html(), border_w]
	var sb: StyleBoxFlat = _boxes.get(key)
	if sb == null:
		sb = StyleBoxFlat.new()
		sb.bg_color = col
		sb.set_corner_radius_all(radius)
		sb.anti_aliasing = true
		if border_w > 0:
			sb.border_color = border
			sb.set_border_width_all(border_w)
		_boxes[key] = sb
	ci.draw_style_box(sb, r)


## Diagonal hatching (45°, rising to the right) clipped to `r`.
static func hatch(ci: CanvasItem, r: Rect2, col: Color, spacing := 4.0, width := 1.2) -> void:
	var c := -r.size.y
	while c < r.size.x:
		# Line from (c, h) to (c + h, 0) in rect space, clipped.
		var a := Vector2(c, r.size.y)
		var b := Vector2(c + r.size.y, 0.0)
		var seg: Variant = clip_segment(a, b, Rect2(Vector2.ZERO, r.size))
		if seg != null:
			ci.draw_line(r.position + seg[0], r.position + seg[1], col, width)
		c += spacing


## Liang–Barsky: the part of segment a→b inside `r`, or null.
static func clip_segment(a: Vector2, b: Vector2, r: Rect2) -> Variant:
	var d := b - a
	var t0 := 0.0
	var t1 := 1.0
	var p := [-d.x, d.x, -d.y, d.y]
	var q := [a.x - r.position.x, r.end.x - a.x, a.y - r.position.y, r.end.y - a.y]
	for i in 4:
		if is_zero_approx(p[i]):
			if q[i] < 0.0:
				return null
			continue
		var t: float = q[i] / p[i]
		if p[i] < 0.0:
			t0 = maxf(t0, t)
		else:
			t1 = minf(t1, t)
		if t0 > t1:
			return null
	return [a + d * t0, a + d * t1]


static func dashed_rect(ci: CanvasItem, r: Rect2, col: Color, width := 2.0, dash := 4.0, gap := 2.6) -> void:
	var pts := [r.position, Vector2(r.end.x, r.position.y), r.end, Vector2(r.position.x, r.end.y), r.position]
	for i in 4:
		ci.draw_dashed_line(pts[i], pts[i + 1], col, width, dash, false)


## A man's pip. Shape-coded (S1b-hud.md "Pips"): fill = selected, dashed
## outline = detached, hatch = pinned, lower half filled = wounded, X = dead.
static func pip(ci: CanvasItem, r: Rect2, selected: bool, wounded: bool, pinned: bool, detached: bool, dead: bool) -> void:
	if dead:
		var k := 2.0
		ci.draw_line(r.position + Vector2(k, k), r.end - Vector2(k, k), Color(TEXT, 0.75), 2.2)
		ci.draw_line(Vector2(r.end.x - k, r.position.y + k), Vector2(r.position.x + k, r.end.y - k), Color(TEXT, 0.75), 2.2)
		return
	var fill := PIP_BRIGHT if selected else PIP_MID
	if detached:
		if selected:
			ci.draw_rect(r.grow(-3.0), PIP_BRIGHT)
		dashed_rect(ci, r.grow(-1.0), TEXT, 2.0)
	elif wounded:
		ci.draw_rect(r.grow(-0.5), Color(TEXT, 0.08))
		ci.draw_rect(Rect2(r.position.x, r.position.y + r.size.y * 0.5, r.size.x, r.size.y * 0.5), PIP_BRIGHT if selected else PIP_LINE)
		ci.draw_rect(r.grow(-0.5), PIP_LINE, false, 1.2)
	elif not pinned:
		box(ci, r, fill, 2)
	if pinned:
		if selected:
			box(ci, r, PIP_BRIGHT, 2)
		hatch(ci, r.grow(-1.0), Color(0.1, 0.12, 0.13) if selected else TEXT, 4.0, 1.4)
		ci.draw_rect(r.grow(-0.5), PIP_LINE, false, 1.2)


## A small clock (call-in re-call cooldown).
static func clock(ci: CanvasItem, c: Vector2, radius: float, col: Color) -> void:
	ci.draw_arc(c, radius, 0.0, TAU, 16, col, 1.4, true)
	ci.draw_polyline(PackedVector2Array([c + Vector2(0, -radius + 2), c, c + Vector2(radius - 2, 0)]), col, 1.4, true)


## A mouse with its right button filled (RMB hotkey glyph); returns its width.
static func mouse_rmb(ci: CanvasItem, at: Vector2, col: Color) -> float:
	var r := Rect2(at, Vector2(9, 13))
	box(ci, r, Color(0, 0, 0, 0), 4, col, 1)
	ci.draw_rect(Rect2(at + Vector2(4.5, 1.0), Vector2(3.0, 4.5)), col)
	return r.size.x


## A padlock, closed or open (minimap north-up lock).
static func lock(ci: CanvasItem, c: Vector2, col: Color, closed: bool) -> void:
	var shackle_c := c + Vector2(0 if closed else 2.5, -1.0)
	ci.draw_arc(shackle_c, 3.5, PI, TAU if closed else TAU - 0.6, 10, col, 1.6, true)
	ci.draw_line(shackle_c + Vector2(-3.5, 0), shackle_c + Vector2(-3.5, 2.5), col, 1.6)
	if closed:
		ci.draw_line(shackle_c + Vector2(3.5, 0), shackle_c + Vector2(3.5, 2.5), col, 1.6)
	box(ci, Rect2(c + Vector2(-5.5, 1.0), Vector2(11, 7)), col, 2)


## Colour of a side as seen by `local`; -1 is neutral.
static func side_color(owner: int, local: int) -> Color:
	if owner < 0:
		return DIM
	return OWN if owner == local else ENEMY
