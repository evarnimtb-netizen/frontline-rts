extends Control
## HUD v2 root (docs/slices/S1b-hud.md): sizes the HUD space, places the
## pieces on the spec's anchors and draws the bottom shade (the only
## full-width backdrop: a gradient, no slab).
##
## Scale: the project stretches canvas items from 1600x900, so a 1280x800
## window would draw the HUD at 0.8 and push 14 px text under the font
## floor. This root scales itself back to at least one physical pixel per
## HUD pixel, so the HUD space is the window size in points below 1600 px
## (1280x800 stays 1280x800) and the stretched 1600-wide space above it.
## Under HudStyle.COMPACT_BELOW px of HUD width it uses the compact metrics
## (1280 adaptation: 200 px minimap, 3 cards per row, 62 px buttons, ...).
##
## Anchors (normal / compact values in HudStyle):
##   top bar   full width, tb tall
##   minimap   top-left (gutter, tb + 8); setting: docked bottom-left
##   feed      top-right (-gutter, tb + 8)
##   console   one top (H - base_gap - hc) and one baseline (H - base_gap):
##             roster | detail (detail_w) | orders (5 x btn_w), right-aligned
##   call-in   right-aligned, ci_gap above the orders; two tiles per row
##             when one column would rise above 57.5 % of the height (S3)

var view: MatchView
## Metrics in use (HudStyle.NORMAL or COMPACT).
var m: Dictionary = HudStyle.NORMAL
var compact := false
## HUD pixels per logical (stretched) viewport unit.
var k := 1.0
## Console top and baseline in HUD pixels.
var console_top := 0.0
var console_base := 0.0
## Test hook: lay out for this HUD size instead of the viewport's.
var force_size := Vector2.ZERO

var _last_vp := Vector2.ZERO
var _last_win := Vector2i.ZERO
var _last_dock := ""


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	set_anchors_preset(Control.PRESET_TOP_LEFT)


func update_state(_s: Dictionary) -> void:
	var vp := get_viewport().get_visible_rect().size
	var win := get_window().size
	var dock: String = view.minimap.dock
	if vp != _last_vp or win != _last_win or dock != _last_dock or force_size != Vector2.ZERO:
		_last_vp = vp
		_last_win = win
		_last_dock = dock
		relayout()


## Window pixels per logical viewport unit (the stretch factor). Window
## pixels are framebuffer pixels: what a screenshot holds.
func stretch() -> float:
	var vp := get_viewport().get_visible_rect().size
	var win := Vector2(get_window().size)
	# Headless runs have a 64 px dummy window: no stretch to undo.
	if vp.x <= 0.0 or win.x <= 0.0 or DisplayServer.get_name() == "headless":
		return 1.0
	return win.x / vp.x


func relayout() -> void:
	var vp := get_viewport().get_visible_rect().size
	var s := stretch()
	k = maxf(s, 1.0) / s
	var hud_size := vp / k
	if force_size != Vector2.ZERO:
		hud_size = force_size
		k = 1.0
	position = Vector2.ZERO
	scale = Vector2(k, k)
	size = hud_size
	compact = hud_size.x < HudStyle.COMPACT_BELOW
	m = HudStyle.COMPACT if compact else HudStyle.NORMAL
	var w := hud_size.x
	var h := hud_size.y
	var g: float = m.gutter
	var tb: float = m.tb
	console_base = h - float(m.base_gap)
	console_top = console_base - float(m.hc)

	_place(view.top_bar, Rect2(0, 0, w, tb))
	var mm: float = m.mm
	var docked_low: bool = view.minimap.dock == "bottom_left"
	var mm_rect := Rect2(g, tb + 8.0, mm, mm)
	if docked_low:
		mm_rect.position.y = console_base - mm
	_place(view.minimap, mm_rect)
	_place(view.event_feed, Rect2(w - g - float(m.feed_w), tb + 8.0, float(m.feed_w), float(m.feed_line) * 3.0))

	var bw: float = m.btn_w
	var gap: float = m.btn_gap
	var orders_w := 5.0 * bw + 4.0 * gap
	var ox := w - g - orders_w
	_place(view.orders, Rect2(ox, console_top, orders_w, float(m.hc)))
	var dw: float = m.detail_w
	var dx := ox - 12.0 - dw
	_place(view.detail, Rect2(dx, console_top, dw, float(m.hc)))
	var rx := g + (mm + 12.0 if docked_low else 0.0)
	_place(view.roster, Rect2(rx, console_top, maxf(dx - 12.0 - rx, 120.0), float(m.hc)))
	# S3: two tiles per row when one column would rise too high.
	var bottom := console_top - float(m.ci_gap)
	var col: Vector2 = view.call_in.column_size(m, bottom, h)
	_place(view.call_in, Rect2(w - g - col.x, bottom - col.y, col.x, col.y))
	# Net mode (S2): the readout in the top bar's free span right of the
	# enemy tickets; the net panels over the whole HUD.
	var span: Vector2 = view.top_bar.free_span()
	_place(view.net_readout, Rect2(span.x, 0, span.y - span.x, tb))
	_place(view.net_overlay, Rect2(0, 0, w, h))
	for c in [view.top_bar, view.minimap, view.event_feed, view.orders, view.detail, view.roster, view.call_in, view.net_readout, view.net_overlay]:
		c.queue_redraw()
	queue_redraw()


func _place(c: Control, r: Rect2) -> void:
	c.set_anchors_preset(Control.PRESET_TOP_LEFT)
	c.position = r.position
	c.size = r.size


## Rect of a HUD piece in HUD pixels (tests, screenshots).
func rect_of(c: Control) -> Rect2:
	return Rect2(c.position, c.size)


func _draw() -> void:
	# Bottom shade: transparent at the top, SHADE alpha at the bottom.
	var fade: float = m.fade
	var y0 := size.y - fade
	var top := Color(HudStyle.SHADE, 0.0)
	draw_polygon(
		PackedVector2Array([Vector2(0, y0), Vector2(size.x, y0), Vector2(size.x, size.y), Vector2(0, size.y)]),
		PackedColorArray([top, top, HudStyle.SHADE, HudStyle.SHADE]))
