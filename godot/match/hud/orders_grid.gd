extends Control
## Orders grid (HUD v2 console, right): 5 columns x 3 rows.
##   row 1, warm ability row: Frag G, Smoke T, Satchel F (each with the
##          selection's count, greyed at 0), Barrage B (mortars);
##   row 2, orders: Move (RMB), Attack (Ctrl+RMB), Stop H, Retreat R,
##          Dig in Y;
##   row 3, a segmented stance strip: Stand X, Crouch C, Prone Z, Auto
##          (no key: pressing the active stance again returns to Auto),
##          then Regroup L.
## The armed tool and the selection's shared stance are outlined in the
## accent colour. Move and Attack arm a click-to-place tool (the mouse
## shortcut needs no button). Every button has a tooltip.
##
## S3, vehicles only selected (a mixed selection keeps the soldier grid;
## the vehicle keys still reach the vehicles):
##   row 1: Reverse (Alt+RMB, arms a reverse tool), Face O (tool), Repair J;
##   row 2: Move, Attack, Stop H, Retreat R;
##   row 3: the ammo toggle, a segmented strip Auto | AP | APCR | HE (N
##   cycles; APCR only lights on a gun that carries it, S3b).

const ROW_ABILITY := [
	[&"frag", "Frag", &"ability_frag", "Throw a frag grenade (5–30 m). The nearest eligible man throws; Shift+click: every eligible man throws one."],
	[&"smoke", "Smoke", &"ability_smoke", "Throw a smoke grenade (5–30 m); the cloud blocks spotting for about 30 s."],
	[&"satchel", "Satchel", &"ability_satchel", "Place or throw a satchel charge (3–8 m); cuts trench lines. Engineers carry one per squad."],
	[&"barrage", "Barrage", &"order_barrage", "Mortars fire on a position (60–600 m). Right-clicking the ground in range with mortars selected also fires."],
]
const ROW_ORDERS := [
	[&"move", "Move", &"order_move", "Move to a point. Right-click on the ground does the same; this button arms a click-to-place tool."],
	[&"attack_move", "Attack", &"order_attack_move", "Advance and engage spotted enemies on the way (Ctrl+right-click)."],
	[&"stop", "Stop", &"order_stop", "Halt and hold the current position."],
	[&"retreat", "Retreat", &"order_retreat", "Fall back to base, faster, ignoring enemies."],
	[&"dig", "Dig in", &"order_dig", "Click points along the ground (max 60 m), right-click to dig. Rifle and engineer men dig; engineers twice as fast."],
]
const ROW_STANCE := [
	[&"stand", "Stand", &"stance_stand", "Keep standing (suppression can still force them lower). Press again for Auto."],
	[&"crouch", "Crouch", &"stance_crouch", "Stay crouched. Press again for Auto."],
	[&"prone", "Prone", &"stance_prone", "Go prone. Press again for Auto."],
	[&"auto", "Auto", &"", "Stance chosen by the situation (the default)."],
]
const REGROUP := [&"regroup", "Regroup", &"order_regroup", "Detached men of the selected squads rejoin their squad's formation."]
const ROW_VEHICLE := [
	[&"reverse", "Reverse", &"order_reverse", "Back up to a point, front kept toward the enemy (Alt+right-click). The button arms a click-to-place tool."],
	[&"face", "Face", &"order_face", "Turn the hull front toward a point and hold (click the point)."],
	[&"repair", "Repair", &"order_repair", "The crew repairs the worst repairable part in place: tracks to full, engine and turret ring to half. No moving or main gun meanwhile; a hit stops it."],
]
const ROW_VEHICLE_ORDERS := [
	[&"move", "Move", &"order_move", "Move to a point. Right-click on the ground does the same; a point behind and within 40 m reverses."],
	[&"attack_move", "Attack", &"order_attack_move", "Advance; halt to fire the main gun at spotted enemies (Ctrl+right-click). Right-click a seen enemy vehicle to focus it."],
	[&"stop", "Stop", &"order_stop", "Halt and hold the current position."],
	[&"retreat", "Retreat", &"order_retreat", "Drive back to base."],
]
const ROW_AMMO := [
	[&"ammo_auto", "Auto", &"ammo_next", "Main gun ammo: AP at vehicles (APCR, where carried, only when AP would likely fail), HE at infantry, swapped as targets change (one reload). N cycles Auto, AP, APCR, HE."],
	[&"ammo_ap", "AP", &"", "Fire AP only. N cycles Auto, AP, APCR, HE."],
	[&"ammo_apcr", "APCR", &"", "Fire the special APCR rounds (a handful, Panzer IV only). N cycles Auto, AP, APCR, HE."],
	[&"ammo_he", "HE", &"", "Fire HE only. N cycles Auto, AP, APCR, HE."],
]
const TOOLS := [&"move", &"attack_move", &"dig", &"barrage", &"face", &"reverse"]
const AMMO_IDS := [&"ammo_auto", &"ammo_ap", &"ammo_apcr", &"ammo_he"]
const ITEMS := [&"frag", &"smoke", &"satchel"]
const STANCE_IDS := [&"stand", &"crouch", &"prone"]

var view: MatchView
## Laid-out buttons: {id, rect, label, key, tip, kind: "ability"|"order"|"stance"}.
var buttons: Array = []
## "soldier" (S1b grid) or "vehicle" (vehicles only selected).
var grid_mode := "soldier"
var _hover: StringName = &""
var _size := Vector2.ZERO


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_STOP
	tooltip_text = " "


func update_state(_s: Dictionary) -> void:
	var sel: Node = view.selection
	var want := "vehicle" if not sel.selected_vehicles().is_empty() and sel.selected_soldiers().is_empty() else "soldier"
	if size != _size or buttons.is_empty() or want != grid_mode:
		_size = size
		grid_mode = want
		_layout()
	queue_redraw()


func _layout() -> void:
	buttons.clear()
	var m: Dictionary = view.hud.m
	var bw: float = m.btn_w
	var g: float = m.btn_gap
	var ow := 5.0 * bw + 4.0 * g
	var bh := (size.y - 2.0 * g) / 3.0
	var abw := (ow - 3.0 * g) / 4.0
	if grid_mode == "vehicle":
		for i in ROW_VEHICLE.size():
			_add(ROW_VEHICLE[i], Rect2(i * (abw + g), 0, abw, bh), "order")
		for i in ROW_VEHICLE_ORDERS.size():
			_add(ROW_VEHICLE_ORDERS[i], Rect2(i * (bw + g), bh + g, bw, bh), "order")
		var aw := 4.0 * bw + 3.0 * g
		for i in ROW_AMMO.size():
			_add(ROW_AMMO[i], Rect2(i * aw / ROW_AMMO.size(), 2.0 * (bh + g), aw / ROW_AMMO.size(), bh), "stance")
		return
	for i in ROW_ABILITY.size():
		_add(ROW_ABILITY[i], Rect2(i * (abw + g), 0, abw, bh), "ability")
	for i in ROW_ORDERS.size():
		_add(ROW_ORDERS[i], Rect2(i * (bw + g), bh + g, bw, bh), "order")
	var sw := 4.0 * bw + 3.0 * g
	var seg := sw / 4.0
	for i in ROW_STANCE.size():
		_add(ROW_STANCE[i], Rect2(i * seg, 2.0 * (bh + g), seg, bh), "stance")
	_add(REGROUP, Rect2(4.0 * (bw + g), 2.0 * (bh + g), bw, bh), "order")


func _add(def: Array, r: Rect2, kind: String) -> void:
	var action: StringName = def[2]
	buttons.append({"id": def[0], "label": def[1], "key": MatchInput.LABELS.get(action, ""), "tip": def[3], "rect": r, "kind": kind})


func button_rect(id: StringName) -> Rect2:
	for b in buttons:
		if b.id == id:
			return b.rect
	return Rect2()


## Whether a button can act on the current selection.
func enabled(id: StringName) -> bool:
	if view.input_blocked():
		return false
	var sel: Node = view.selection
	if id in STANCE_IDS or id == &"auto":
		return not sel.selected_soldiers().is_empty() and not view.proxy_soldiers
	return sel.can(id)


## Whether a button shows as active (armed tool, shared stance).
func active(id: StringName) -> bool:
	var sel: Node = view.selection
	if id in TOOLS:
		return sel.tool == id
	if id in ITEMS:
		return sel.tool == &"throw" and StringName(sel.throw_item) == id
	if id in STANCE_IDS or id == &"auto":
		return enabled(id) and StringName(sel.active_stance()) == id
	if id in AMMO_IDS:
		return enabled(id) and "ammo_" + sel.ammo_choice() == String(id)
	return false


## Acts as if the button were clicked (also used by tests).
func press(id: StringName) -> void:
	var sel: Node = view.selection
	if not enabled(id):
		if not sel.selected.is_empty():
			AudioManager.play_ui(&"error")
		return
	AudioManager.play_ui(&"click")
	if id in TOOLS or id in ITEMS:
		if active(id):
			sel.cancel_tool()
		else:
			sel.activate(id)
	elif id in STANCE_IDS:
		sel.press_stance(String(id))
	elif id == &"auto":
		sel.set_stance(&"auto")
	else:
		sel.activate(id)


func _draw() -> void:
	if view == null:
		return
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var sel: Node = view.selection
	# Stance strip background (one segmented box).
	var strip := Rect2()
	for b in buttons:
		if b.kind == "stance":
			strip = b.rect if strip.size == Vector2.ZERO else strip.merge(b.rect)
	HudStyle.box(self, strip, HudStyle.BG_BUTTON, 4)
	for b in buttons:
		var r: Rect2 = b.rect
		var id: StringName = b.id
		var on := enabled(id)
		var lit := active(id)
		if b.kind == "stance":
			if id != &"stand" and id != &"ammo_auto":
				draw_rect(Rect2(r.position.x - 0.5, r.position.y + 7.0, 1.0, r.size.y - 14.0), Color(HudStyle.TEXT, 0.2))
		else:
			HudStyle.box(self, r, HudStyle.BG_ABILITY if b.kind == "ability" else HudStyle.BG_BUTTON, 4)
		if _hover == id and on:
			HudStyle.box(self, r.grow(-1.0), HudStyle.HOVER, 4)
		if lit:
			HudStyle.box(self, r.grow(-1.0), Color(HudStyle.ACCENT, 0.16), 4, HudStyle.ACCENT, 2)
		var key_col := HudStyle.ACCENT if on else HudStyle.OFF_ACCENT
		var txt_col := HudStyle.TEXT if on else HudStyle.OFF_TEXT
		_draw_key(b, bold, key_col)
		# Count badge on the item buttons.
		if id in ITEMS:
			var n: int = sel.item_count(String(id))
			HudStyle.text(self, bold, Vector2(r.end.x - 6.0, r.position.y + 16.0), "×%d" % n, HudStyle.FONT_SMALL, txt_col, 1)
		var label := HudStyle.fit(font, b.label, r.size.x - 6.0, HudStyle.FONT)
		HudStyle.text(self, font, Vector2(r.get_center().x, r.end.y - 8.0), label, HudStyle.FONT, txt_col, 0)


func _draw_key(b: Dictionary, bold: Font, col: Color) -> void:
	var r: Rect2 = b.rect
	var at := r.position + Vector2(6.0, 16.0)
	match b.id:
		&"move":
			HudStyle.mouse_rmb(self, r.position + Vector2(6.0, 5.0), col)
		&"attack_move":
			HudStyle.text(self, bold, at, "Ctrl", HudStyle.FONT_SMALL, col)
			HudStyle.mouse_rmb(self, r.position + Vector2(8.0 + HudStyle.text_w(bold, "Ctrl", HudStyle.FONT_SMALL), 5.0), col)
		&"reverse":
			HudStyle.text(self, bold, at, "Alt", HudStyle.FONT_SMALL, col)
			HudStyle.mouse_rmb(self, r.position + Vector2(8.0 + HudStyle.text_w(bold, "Alt", HudStyle.FONT_SMALL), 5.0), col)
		_:
			if b.key != "":
				HudStyle.text(self, bold, at, b.key, HudStyle.FONT_SMALL, col)


func button_at(p: Vector2) -> StringName:
	for b in buttons:
		if b.rect.has_point(p):
			return b.id
	return &""


func _has_point(p: Vector2) -> bool:
	return button_at(p) != &""


func _gui_input(event: InputEvent) -> void:
	if view == null:
		return
	if event is InputEventMouseMotion:
		var h := button_at(event.position)
		if h != _hover:
			_hover = h
			queue_redraw()
	elif event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		var id := button_at(event.position)
		if id != &"":
			press(id)
		accept_event()


func _notification(what: int) -> void:
	if what == NOTIFICATION_MOUSE_EXIT and _hover != &"":
		_hover = &""
		queue_redraw()


func _get_tooltip(at: Vector2) -> String:
	var id := button_at(at)
	for b in buttons:
		if b.id == id:
			var key: String = b.key
			var head: String = b.label if key == "" else "%s  [%s]" % [b.label, key]
			return "%s\n%s" % [head, b.tip]
	return ""
