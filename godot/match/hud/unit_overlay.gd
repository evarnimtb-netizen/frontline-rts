extends Control
## Screen-space unit layer, in viewport coordinates under the HUD: a class
## marker above every squad in the state (own and spotted enemy; §12
## "class marker readable at 800p"), capture point labels, the selection
## box, the hovered soldier's tooltip, and the armed tool's cursor hint
## (the throw tool's reticle turns red out of range). Draw-only: it never
## takes mouse input.

const MARKER := Vector2(26, 18)

var view: MatchView


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_IGNORE


func update_state(_s: Dictionary) -> void:
	queue_redraw()


func _draw() -> void:
	if view == null:
		return
	var font := get_theme_default_font()
	var sel: Node = view.selection
	var local := view.local_player
	# Capture point labels.
	for p in view.state.get("points", []):
		var wp: Vector3 = p.pos + Vector3(0, 7, 0)
		if view.camera == null or view.camera.is_position_behind(wp):
			continue
		var sp := view.camera.unproject_position(wp)
		if not Rect2(Vector2.ZERO, size).grow(60.0).has_point(sp):
			continue
		var col := UiPalette.owner_color(p.get("owner", -1), local)
		var pts := PackedVector2Array([sp + Vector2(0, -15), sp + Vector2(15, 0), sp + Vector2(0, 15), sp + Vector2(-15, 0)])
		draw_colored_polygon(pts, Color(UiPalette.BG, 0.8))
		pts.append(pts[0])
		var border := UiPalette.WARN if p.get("contested", false) else col
		draw_polyline(pts, border, 2.0)
		draw_string(font, sp + Vector2(-6, 6), str(p.id), HORIZONTAL_ALIGNMENT_CENTER, 12, 17, col.lightened(0.3))
	# Squad markers.
	var marked: PackedInt32Array = sel.selected_squads()
	var hover_squad := view.soldier_squad(sel.hover_id) if sel.hover_id != -1 else -1
	for id in view.squad_screen:
		var sq: Dictionary = view.squads_by_id[id]
		var sp: Vector2 = view.squad_screen[id]
		var own := int(sq.owner) == local
		var col := UiPalette.OWN if own else UiPalette.ENEMY
		var selected: bool = marked.has(id)
		var r := Rect2(sp - MARKER * 0.5 - Vector2(0, 14), MARKER)
		if selected:
			draw_rect(r.grow(3), Color(UiPalette.TEXT, 0.9), false, 2.0 if sel.squad_fully_selected(id) else 1.0)
		elif id == hover_squad:
			draw_rect(r.grow(3), Color(col, 0.7), false, 1.5)
		draw_rect(r, Color(UiPalette.BG, 0.75), true)
		UiPalette.draw_glyph(self, sq.type, r, col.lightened(0.15 if selected else 0.0), 2.0)
		# Strength bar under the marker.
		var men_max := maxi(int(sq.get("men_max", 1)), 1)
		var frac := float(sq.get("men", men_max)) / men_max
		var bar := Rect2(r.position + Vector2(0, r.size.y + 2), Vector2(r.size.x, 3))
		draw_rect(bar, Color(0, 0, 0, 0.7))
		draw_rect(Rect2(bar.position, Vector2(bar.size.x * frac, bar.size.y)), UiPalette.OK if frac > 0.5 else (UiPalette.WARN if frac > 0.25 else UiPalette.DANGER))
		var status: String = sq.get("status", "ok")
		if status == "pinned" or status == "suppressed":
			draw_string(font, r.position + Vector2(r.size.x + 3, 11), "!" if status == "suppressed" else "!!", HORIZONTAL_ALIGNMENT_LEFT, -1, 14, UiPalette.WARN if status == "suppressed" else UiPalette.DANGER)
		elif status == "retreating":
			draw_string(font, r.position + Vector2(r.size.x + 3, 11), "«", HORIZONTAL_ALIGNMENT_LEFT, -1, 15, UiPalette.TEXT_DIM)
		if own and sq.get("order", "") == "dig":
			var dp: float = sq.get("dig_progress", 0.0)
			draw_rect(Rect2(bar.position + Vector2(0, 5), Vector2(bar.size.x * dp, 2)), UiPalette.ACCENT)
	# Selection box.
	if sel.dragging:
		var box := Rect2(sel.drag_from, Vector2.ZERO).expand(sel.drag_to)
		draw_rect(box, Color(UiPalette.OWN, 0.12), true)
		draw_rect(box, Color(UiPalette.OWN, 0.9), false, 1.0)
	if sel.tool == &"":
		_draw_tooltip(font, sel)
	else:
		_draw_tool_hint(font, sel)


## Hover tooltip: name and squad, weapon, health, state.
func _draw_tooltip(font: Font, sel: Node) -> void:
	if sel.hover_id == -1 or sel.dragging or view.proxy_soldiers:
		return
	var so: Dictionary = view.soldiers_by_id.get(sel.hover_id, {})
	if so.is_empty():
		return
	var own := int(so.owner) == view.local_player
	var sid := int(so.squad)
	var line1 := "%s · %s" % [view.soldier_name(so), view.squad_label(sid, true)] if own else "Enemy %s" % String(UiPalette.SQUAD_NAMES.get(view.squad_type(sid), "soldier")).to_lower()
	var cond := "pinned" if view.soldier_pinned(so) else String(so.get("stance", "stand"))
	if so.get("detached", false) and own:
		cond += ", detached"
	var line2 := "%s · %d HP · %s" % [view.soldier_weapon(so), roundi(float(so.get("hp", 100.0))), cond]
	var bold := HudStyle.bold()
	var w := maxf(HudStyle.text_w(bold, line1, HudStyle.FONT), HudStyle.text_w(font, line2, HudStyle.FONT)) + 16.0
	var at: Vector2 = sel.cursor() + Vector2(16, 18)
	at.x = minf(at.x, size.x - w - 8.0)
	at.y = minf(at.y, size.y - 52.0)
	HudStyle.box(self, Rect2(at, Vector2(w, 44)), Color(0.055, 0.07, 0.08, 0.85), 4)
	draw_rect(Rect2(at + Vector2(0, 4), Vector2(2, 36)), HudStyle.OWN if own else HudStyle.ENEMY)
	HudStyle.text(self, bold, at + Vector2(8, 18), line1, HudStyle.FONT, HudStyle.TEXT)
	HudStyle.text(self, font, at + Vector2(8, 37), line2, HudStyle.FONT, HudStyle.DIM)


func _draw_tool_hint(font: Font, sel: Node) -> void:
	var m: Vector2 = sel.cursor()
	var text := ""
	var col := HudStyle.ACCENT
	match sel.tool:
		&"move":
			text = "MOVE  ·  click to place  ·  right-click cancels"
		&"attack_move":
			text = "ATTACK-MOVE  ·  click target  ·  right-click cancels"
		&"barrage":
			text = "BARRAGE  ·  click target in range  ·  right-click cancels"
		&"dig":
			text = "DIG  %d / %d m  ·  click adds point  ·  right-click digs  ·  Backspace undo" % [roundi(sel.dig_length() + _pending_dig(sel)), roundi(sel.dig_max())]
		&"throw":
			var item: String = sel.throw_item
			var g: Variant = view.ground_pick(m)
			var ok: bool = g != null and sel.throw_in_range(item, g)
			col = HudStyle.ACCENT if ok else HudStyle.DANGER
			var rr := view.throw_range(item)
			text = "%s ×%d  ·  click throws  ·  Shift: every man  ·  right-click cancels" % [item.to_upper(), sel.item_count(item)]
			if not ok:
				text = "%s  ·  OUT OF RANGE (%d–%d m)" % [item.to_upper(), roundi(rr.x), roundi(rr.y)]
			# Reticle at the cursor.
			draw_arc(m, 11.0, 0.0, TAU, 24, col, 2.0, true)
			for d in [Vector2(1, 0), Vector2(-1, 0), Vector2(0, 1), Vector2(0, -1)]:
				draw_line(m + d * 6.0, m + d * 16.0, col, 2.0)
	var w := font.get_string_size(text, HORIZONTAL_ALIGNMENT_LEFT, -1, HudStyle.FONT).x
	var at := m + Vector2(18, 30)
	at.x = minf(at.x, size.x - w - 16)
	draw_rect(Rect2(at - Vector2(6, 15), Vector2(w + 12, 21)), Color(UiPalette.BG, 0.85))
	draw_string(font, at, text, HORIZONTAL_ALIGNMENT_LEFT, -1, HudStyle.FONT, col)


func _pending_dig(sel: Node) -> float:
	if sel.dig_points.is_empty():
		return 0.0
	var c: Variant = sel.dig_candidate(sel.cursor())
	if c == null:
		return 0.0
	return sel.dig_points[sel.dig_points.size() - 1].distance_to(c)
