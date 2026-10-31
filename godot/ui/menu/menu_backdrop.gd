extends Control
## Main menu backdrop: the map drawn like a dim field map on a table, with a
## grid, a few plotted positions, and a dark band on the left behind the menu.

var _tex: Texture2D
var _t := 0.0


func _ready() -> void:
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_tex = ImageTexture.create_from_image(MockMap.make_image(256))
	resized.connect(queue_redraw)


func _process(delta: float) -> void:
	_t += delta
	queue_redraw()


func _draw() -> void:
	var sz := size
	draw_rect(Rect2(Vector2.ZERO, sz), UiPalette.BG)
	# Map: square, anchored right, slowly drifting.
	var side := maxf(sz.x * 0.85, sz.y * 1.25)
	var drift := Vector2(sin(_t * 0.03), cos(_t * 0.025)) * 18.0
	var map_rect := Rect2(Vector2(sz.x - side * 0.92, (sz.y - side) * 0.5) + drift, Vector2(side, side))
	draw_texture_rect(_tex, map_rect, false, Color(0.55, 0.57, 0.52, 0.55))
	# Grid every 128 m with small coordinate labels.
	var font := get_theme_default_font()
	var cells := 8
	for k in cells + 1:
		var f := float(k) / cells
		var x := map_rect.position.x + f * side
		var y := map_rect.position.y + f * side
		var col := Color(UiPalette.ACCENT, 0.10)
		draw_line(Vector2(x, map_rect.position.y), Vector2(x, map_rect.end.y), col, 1.0)
		draw_line(Vector2(map_rect.position.x, y), Vector2(map_rect.end.x, y), col, 1.0)
		if k < cells:
			draw_string(font, Vector2(x + 4, map_rect.position.y + 14 + side * 0.12), "%02d" % (k * 13), HORIZONTAL_ALIGNMENT_LEFT, -1, 11, Color(UiPalette.ACCENT, 0.28))
	# Capture points as plotted circles.
	for p in MockMap.POINTS:
		var uv: Vector2 = (p.pos + Vector2(MockMap.HALF, MockMap.HALF)) / MockMap.SIZE_M
		var c := map_rect.position + uv * side
		draw_arc(c, side * 0.018, 0.0, TAU, 24, Color(UiPalette.TEXT, 0.35), 1.5)
		draw_string(font, c + Vector2(side * 0.022, -side * 0.012), p.id, HORIZONTAL_ALIGNMENT_LEFT, -1, 16, Color(UiPalette.TEXT, 0.4))
	# Left band behind the menu, fading into the map.
	var band := sz.x * 0.46
	var steps := 24
	for s in steps:
		var a := 1.0 - float(s) / steps
		var x0 := band * float(s) / steps
		draw_rect(Rect2(x0 + band * 0.4, 0, band / steps + 1.0, sz.y), Color(UiPalette.BG, a * 0.92))
	draw_rect(Rect2(0, 0, band * 0.4, sz.y), Color(UiPalette.BG, 0.92))
	# Vignette top and bottom.
	for s in 12:
		var a := (1.0 - s / 12.0) * 0.5
		draw_rect(Rect2(0, s * 6, sz.x, 6), Color(0, 0, 0, a * 0.5))
		draw_rect(Rect2(0, sz.y - (s + 1) * 6, sz.x, 6), Color(0, 0, 0, a * 0.5))
