extends HFlowContainer
## Cards for the selected squads: class glyph, name, men (pips), health,
## suppression state and current order. Click a card to select only that
## squad, Shift-click to drop it from the selection. Up to MAX_CARDS cards,
## then a "+N" tile. Wraps to a second row.

const MAX_CARDS := 10
const CARD := Vector2(128, 84)
const STATUS := {
	"ok": ["READY", UiPalette.OK],
	"suppressed": ["SUPPRESSED", UiPalette.WARN],
	"pinned": ["PINNED", UiPalette.DANGER],
	"retreating": ["RETREATING", UiPalette.TEXT_DIM],
}
const ORDERS := {
	"idle": "Holding",
	"move": "Moving",
	"attack_move": "Attacking",
	"retreat": "Retreating",
	"dig": "Digging",
	"barrage": "Barrage",
	"setup": "Setting up",
}

var view: MatchView
var _cards: Array[Control] = []
var _more: Label


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_theme_constant_override("h_separation", 6)
	add_theme_constant_override("v_separation", 6)
	alignment = FlowContainer.ALIGNMENT_CENTER
	for i in MAX_CARDS:
		var c := Control.new()
		c.custom_minimum_size = CARD
		c.mouse_filter = Control.MOUSE_FILTER_STOP
		c.mouse_default_cursor_shape = Control.CURSOR_POINTING_HAND
		c.set_meta("squad", -1)
		c.draw.connect(_draw_card.bind(c))
		c.gui_input.connect(_card_input.bind(c))
		c.visible = false
		add_child(c)
		_cards.append(c)
	_more = Label.new()
	_more.add_theme_font_size_override("font_size", 20)
	_more.visible = false
	add_child(_more)


func update_state(_s: Dictionary) -> void:
	var ids: PackedInt32Array = view.selection.selected
	for i in MAX_CARDS:
		var c := _cards[i]
		if i < ids.size() and (i < MAX_CARDS - 1 or ids.size() == MAX_CARDS):
			c.set_meta("squad", ids[i])
			c.visible = true
			c.queue_redraw()
		else:
			c.set_meta("squad", -1)
			c.visible = false
	var shown := mini(ids.size(), MAX_CARDS if ids.size() == MAX_CARDS else MAX_CARDS - 1)
	_more.visible = ids.size() > shown
	_more.text = "+%d" % (ids.size() - shown)


func _draw_card(c: Control) -> void:
	var id: int = c.get_meta("squad")
	var sq: Dictionary = view.squads_by_id.get(id, {})
	if sq.is_empty():
		return
	var font := get_theme_default_font()
	var r := Rect2(Vector2.ZERO, c.size)
	c.draw_rect(r, Color(0.129, 0.149, 0.165, 0.94))
	c.draw_rect(r, UiPalette.BORDER, false, 1.0)
	c.draw_rect(Rect2(0, 0, 3, r.size.y), UiPalette.OWN)
	UiPalette.draw_glyph(c, sq.type, Rect2(Vector2(10, 7), Vector2(26, 18)), UiPalette.OWN, 2.0)
	var title: String = sq.get("name", UiPalette.SQUAD_NAMES.get(sq.type, sq.type))
	c.draw_string(font, Vector2(42, 23), _fit(font, title, r.size.x - 46, 14), HORIZONTAL_ALIGNMENT_LEFT, -1, 14, UiPalette.TEXT)
	var men := int(sq.get("men", 0))
	var men_max := maxi(int(sq.get("men_max", 1)), 1)
	c.draw_string(font, Vector2(10, 35), "%d/%d men" % [men, men_max], HORIZONTAL_ALIGNMENT_LEFT, -1, 12, UiPalette.TEXT_DIM)
	# Men pips.
	var pip_w := minf(10.0, (r.size.x - 20.0) / men_max - 2.0)
	for i in men_max:
		var pr := Rect2(Vector2(10 + i * (pip_w + 2.0), 42), Vector2(pip_w, 6))
		c.draw_rect(pr, UiPalette.OWN if i < men else Color(1, 1, 1, 0.12))
	# Health bar.
	var hp: float = clampf(float(sq.get("health", 1.0)), 0.0, 1.0)
	var hb := Rect2(Vector2(10, 52), Vector2(r.size.x - 20, 5))
	c.draw_rect(hb, Color(0, 0, 0, 0.6))
	c.draw_rect(Rect2(hb.position, Vector2(hb.size.x * hp, hb.size.y)), UiPalette.OK if hp > 0.5 else (UiPalette.WARN if hp > 0.25 else UiPalette.DANGER))
	# Suppression state and order.
	var st: Array = STATUS.get(sq.get("status", "ok"), STATUS.ok)
	c.draw_string(font, Vector2(10, 75), st[0], HORIZONTAL_ALIGNMENT_LEFT, -1, 12, st[1])
	var order: String = ORDERS.get(sq.get("order", "idle"), str(sq.get("order", "")))
	c.draw_string(font, Vector2(10, 35), order, HORIZONTAL_ALIGNMENT_RIGHT, r.size.x - 18, 12, UiPalette.ACCENT)
	var sup: float = clampf(float(sq.get("suppression", 0.0)), 0.0, 1.0)
	if sup > 0.0:
		c.draw_rect(Rect2(Vector2(10, 60), Vector2((r.size.x - 20) * sup, 2)), Color(st[1], 0.8))


## `text` shortened with an ellipsis to fit `width` px.
func _fit(font: Font, text: String, width: float, font_size: int) -> String:
	if font.get_string_size(text, HORIZONTAL_ALIGNMENT_LEFT, -1, font_size).x <= width:
		return text
	var s := text
	while s.length() > 1 and font.get_string_size(s + "…", HORIZONTAL_ALIGNMENT_LEFT, -1, font_size).x > width:
		s = s.left(s.length() - 1)
	return s + "…"


func _card_input(event: InputEvent, c: Control) -> void:
	if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		var id: int = c.get_meta("squad")
		if id < 0:
			return
		if event.shift_pressed:
			view.selection.deselect(id)
		elif event.double_click:
			view.jump_camera(view.squads_by_id[id].pos)
		else:
			view.selection.select_only(PackedInt32Array([id]))
		c.accept_event()
