extends Control
## Capture point strip under the top bar: one chip per point, filled with
## the owner's colour, a progress bar in the capturing side's colour, and a
## pulsing amber frame while contested. Click a chip to look at the point.

const CHIP := Vector2(40, 40)
const GAP := 8.0

var view: MatchView
var _points: Array = []
var _t := 0.0


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_STOP


func update_state(s: Dictionary) -> void:
	_points = s.get("points", [])
	_t += get_process_delta_time()
	var w := _points.size() * CHIP.x + maxf(_points.size() - 1, 0) * GAP
	custom_minimum_size = Vector2(w, CHIP.y + 6)
	size = custom_minimum_size
	position.x = (get_parent_area_size().x - w) * 0.5
	queue_redraw()


func _chip_rect(i: int) -> Rect2:
	return Rect2(Vector2(i * (CHIP.x + GAP), 0), CHIP)


func _draw() -> void:
	var font := get_theme_default_font()
	var local := view.local_player if view else 0
	for i in _points.size():
		var p: Dictionary = _points[i]
		var r := _chip_rect(i)
		var owner_i := int(p.get("owner", -1))
		var col := UiPalette.owner_color(owner_i, local)
		draw_rect(r, Color(UiPalette.BG, 0.88))
		draw_rect(r.grow(-3), Color(col, 0.35 if owner_i >= 0 else 0.12))
		var frame := Color(col, 0.9)
		var width := 1.5
		if p.get("contested", false):
			frame = Color(UiPalette.WARN, 0.6 + 0.4 * sin(_t * 8.0))
			width = 2.5
		draw_rect(r, frame, false, width)
		draw_string(font, r.position + Vector2(0, 27), str(p.id), HORIZONTAL_ALIGNMENT_CENTER, r.size.x, 22, UiPalette.TEXT)
		var cap := int(p.get("capturing", -1))
		var prog: float = p.get("progress", 0.0)
		if cap >= 0 and prog > 0.0:
			var bar := Rect2(r.position + Vector2(0, r.size.y + 2), Vector2(r.size.x, 4))
			draw_rect(bar, Color(0, 0, 0, 0.7))
			draw_rect(Rect2(bar.position, Vector2(bar.size.x * prog, bar.size.y)), UiPalette.owner_color(cap, local))


func _gui_input(event: InputEvent) -> void:
	if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		for i in _points.size():
			if _chip_rect(i).has_point(event.position):
				view.jump_camera(_points[i].pos)
				accept_event()
				return
