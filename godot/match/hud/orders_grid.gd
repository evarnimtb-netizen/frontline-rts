extends Control
## Orders grid (HUD v2 console, right): 5 columns x 3 rows.
##   row 1, warm ability row: Frag G, Smoke T, Satchel F (each with the
##          selection's count, greyed at 0), Barrage B (mortars), S3b
##          Suppress P (area fire: click a point or drag a line);
##   row 2, orders: Move (RMB), Attack (Ctrl+RMB), Stop H, Retreat R,
##          Dig in Y;
##   row 3, a segmented stance strip: Stand X, Crouch C, Prone Z, Auto
##          (no key: pressing the active stance again returns to Auto),
##          then (S3c) Crew K (the re-crew tool: click an abandoned
##          vehicle) split with (S8a) Board V (click an own half-track),
##          and Regroup L (S8a: Unload U for a mounted selection).
##   below (S5a), a thin segmented fire strip: Discipline (the default: open
##          fire within 70 % of the weapon's range), At will (full
##          range), Hold (fire only at enemies within 30 m, Panzerfausts
##          at their reach; the squad springs to Discipline when it fires
##          or comes under fire).
## The armed tool and the selection's shared stance are outlined in the
## accent colour. Move and Attack arm a click-to-place tool (the mouse
## shortcut needs no button). Every button has a tooltip.
##
## S3, vehicles only selected (a mixed selection keeps the soldier grid;
## the vehicle keys still reach the vehicles):
##   row 1: Reverse (Alt+RMB, arms a reverse tool), Face O (tool), Repair J,
##   S3b Suppress P (tool), S8a Unload U (half-tracks: Shift+U or
##   Shift+click over the sides);
##   row 2: Move, Attack, Stop H, Retreat R, S3c Bail out (no key: a
##   second press within 2 s; the label says "Again?" meanwhile);
##   row 3: the ammo toggle, a segmented strip Auto | AP | APCR | HE (N
##   cycles; APCR only lights on a gun that carries it, S3b).

## S3b: area fire without a spotted target (both grids).
const SUPPRESS := [&"suppress", "Suppress", &"order_suppress", "Fire at an area without a spotted target to keep the enemy's heads down: click a point or drag a line (up to 40 m). Rifles and MGs hold and fire; vehicles fire their MGs and now and then HE. Mortars keep Fire mission. Lasts until another order."]
const ROW_ABILITY := [
	[&"frag", "Frag", &"ability_frag", "Throw a frag grenade (5–30 m). The nearest eligible man throws; Shift+click: every eligible man throws one."],
	[&"smoke", "Smoke", &"ability_smoke", "Throw a smoke grenade (5–30 m); the cloud blocks spotting for about 30 s."],
	[&"satchel", "Satchel", &"ability_satchel", "Place or throw a satchel charge (3–8 m); cuts trench lines. Engineers carry one per squad."],
	[&"barrage", "Mission", &"order_barrage", "Fire mission: mortars fire a mission on a position (60–600 m): observed when a leader in the voice link (150 m) sees it, else map fire on ground you know (never on unknown ground). The cursor shows the mode and the expected miss; Shift+wheel sets the rounds. Right-clicking the ground in range with mortars selected also fires."],
	SUPPRESS,
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
## S5a: fire modes (`set_fire`).
const ROW_FIRE := [
	[&"fire_discipline", "Discipline", &"", "Fire discipline (the default): open fire only within 70 % of the weapon's range, to save rounds. A Suppress order still fires to full range."],
	[&"fire_at_will", "At will", &"", "Fire at will: engage any seen enemy within the weapon's full range (rounds go faster)."],
	[&"fire_hold", "Hold", &"", "Hold fire: no shot except at enemies within 30 m; Panzerfausts and launchers fire at their reach. The squad springs to Discipline once one of them fires or comes under fire (ambush)."],
]
const FIRE_IDS := [&"fire_discipline", &"fire_at_will", &"fire_hold"]
const REGROUP := [&"regroup", "Regroup", &"order_regroup", "Detached men of the selected squads rejoin their squad's formation."]
## S4: takes the Satchel slot when the selection carries Panzerfausts and
## no satchel (German line squads).
const PANZERFAUST := [&"panzerfaust", "Pzfaust", &"ability_satchel", "Panzerfaust 60: click a seen enemy vehicle. The selected squads that carry one aim at it first and close in; a man fires within 60 m (40 m at a moving vehicle). The men also fire on their own at vehicles in reach."]
## S3c.
const CREW := [&"crew", "Crew", &"order_crew", "Click an abandoned vehicle: the selected squads walk to it and man it. Tank crews man it fully; other infantry (not AT teams) man it untrained (slower reload and traverse, wider spread). An enemy's abandoned vehicle is captured. Needs a free vehicle slot and pop. Right-click on it with a tank crew selected does the same."]
## S8a.
const BOARD := [&"embark", "Board", &"order_embark", "Click an own half-track: the selected squads walk to its door and climb in, one man a second. A squad that does not fit is refused (squads never split). Right-click on it does the same."]
const UNLOAD := [&"disembark", "Unload", &"order_disembark", "The half-track stops and its passengers get out by the rear door, one man every 0.8 s. Shift+U (or Shift+click): over the sides, faster (0.5 s) but each man stands up over the rim while he climbs out, on the side away from the last hit."]
## S8b: trucks.
const DROP := [&"drop_crates", "Drop", &"order_drop_crates", "Click a point: the truck drives there and sets down up to 4 crates (3 s each), where they feed mortars and men or stock an HQ inside its ring. I arms the tool; pressing I again changes the kind (shells, small arms, explosives). Nothing is paid here: the truck was paid for when it loaded at the base."]
const BAIL := [&"bail", "Bail out", &"", "The crew leave the vehicle by its hatches and become a crew squad; the vehicle stays, abandoned, and can be re-crewed (by anyone, enemies included). Press twice within 2 s."]
const ROW_VEHICLE := [
	[&"reverse", "Reverse", &"order_reverse", "Back up to a point, front kept toward the enemy (Alt+right-click). The button arms a click-to-place tool."],
	[&"face", "Face", &"order_face", "Turn the hull front toward a point and hold (click the point)."],
	[&"repair", "Repair", &"order_repair", "The crew repairs the worst repairable part in place: tracks to full, engine and turret ring to half. No moving or main gun meanwhile; a hit stops it."],
	SUPPRESS,
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
## S6, only HQ squads and ammunition bearers selected (the supply grid):
##   row 1: Build HQ, Build dump (tools: click the spot), Shells, Ammo,
##          Explosives (each selected squad takes a crate of that kind in
##          the own base, paid there: kg x the kind's price);
##   row 2: the orders row; row 3: the stance strip, Place (tool: set the
##          carried crates down), Pick up (tool: click an own crate).
const ROW_SUPPLY := [
	[&"build_hq", "Build HQ", &"", "Dig in the HQ here (the Zugtrupp / Platoon HQ): 100 MP when ordered. It stands when the pit is done, starts empty and fills only from crates set down inside its ring; men, mortars and tanks inside are served from it."],
	[&"build_dump", "Dump", &"", "Dig an ammunition dump here (up to 3): a smaller ring and stock than the HQ."],
	[&"crate_shells", "Shells", &"", "In the base: each selected squad takes a 32 kg shells crate (mortar bombs, tank rounds), paid now. One crate per 10 s from the base; 12 live crates at most (8 of shells)."],
	[&"crate_small_arms", "Ammo", &"", "In the base: each selected squad takes a 25 kg small-arms crate, paid now."],
	[&"crate_explosives", "Expl.", &"", "In the base: each selected squad takes a 20 kg crate of grenades and charges, paid now."],
]
const PLACE := [&"place_crate", "Place", &"", "Click the ground: the squad carries its crates there and sets them down. Inside an own HQ or dump ring they go into its stock; within 5 m of a mortar or tank they feed it."]
const PICK_UP := [&"pick_up", "Pick up", &"", "Click an own crate on the ground: the squad walks to it and a free man takes it."]
const TOOLS := [&"drop_crates", &"move", &"attack_move", &"dig", &"barrage", &"face", &"reverse", &"suppress", &"crew", &"embark", &"panzerfaust", &"build_hq", &"build_dump", &"place_crate", &"pick_up"]
const AMMO_IDS := [&"ammo_auto", &"ammo_ap", &"ammo_apcr", &"ammo_he"]
const ITEMS := [&"frag", &"smoke", &"satchel"]
## S4: item buttons with a count badge (the Panzerfaust is a tool, not a
## throw).
const COUNTED := [&"frag", &"smoke", &"satchel", &"panzerfaust"]
const STANCE_IDS := [&"stand", &"crouch", &"prone"]

var view: MatchView
## Laid-out buttons: {id, rect, label, key, tip, kind: "ability"|"order"|"stance"}.
var buttons: Array = []
## "soldier" (S1b grid) or "vehicle" (vehicles only selected).
var grid_mode := "soldier"
var _hover: StringName = &""
var _size := Vector2.ZERO
## S4: the third ability slot shows the Panzerfaust.
var _pzf_slot := false
## S8a: the selection has passengers (Unload replaces Regroup).
var _mounted := false
## S8a Fix (L4): a selected vehicle has a main gun (ammo buttons shown).
var _gun := true
## S8b: a selected vehicle is a truck (Drop replaces Unload).
var _truck := false


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_STOP
	tooltip_text = " "


func update_state(_s: Dictionary) -> void:
	var sel: Node = view.selection
	var want := "vehicle" if not sel.selected_vehicles().is_empty() and sel.selected_soldiers().is_empty() else "soldier"
	if want == "soldier" and sel.supply_only():
		want = "supply"
	var pzf: bool = sel.item_count("panzerfaust") > 0 and sel.item_count("satchel") == 0
	var mounted: bool = not sel.unload_vehicles().is_empty()
	var truck: bool = not sel.truck_vehicles().is_empty()
	var gun := false
	for id in sel.selected_vehicles():
		if (view.vehicles_by_id.get(id, {}).get("ammo", {}) as Dictionary).has("ap"):
			gun = true
	if size != _size or buttons.is_empty() or want != grid_mode or pzf != _pzf_slot or truck != _truck or mounted != _mounted or gun != _gun:
		_size = size
		grid_mode = want
		_pzf_slot = pzf
		_mounted = mounted
		_gun = gun
		_truck = truck
		_layout()
	queue_redraw()


## S8b: the Drop button names the armed kind.
func _drop_label() -> void:
	for b in buttons:
		if b.id == &"drop_crates":
			var k: String = view.selection.drop_kind
			b.label = "Drop " + {"shells": "shells", "small_arms": "ammo", "explosives": "expl."}.get(k, "") if view.selection.tool == &"drop_crates" else "Drop"


func _layout() -> void:
	buttons.clear()
	var m: Dictionary = view.hud.m
	var bw: float = m.btn_w
	var g: float = m.btn_gap
	var ow := 5.0 * bw + 4.0 * g
	var bh := (size.y - 2.0 * g) / 3.0
	# S3b: five slots in the top row (the fifth is Suppress).
	var abw := (ow - 4.0 * g) / 5.0
	if grid_mode == "vehicle":
		for i in ROW_VEHICLE.size():
			_add(ROW_VEHICLE[i], Rect2(i * (abw + g), 0, abw, bh), "order")
		for i in ROW_VEHICLE_ORDERS.size():
			_add(ROW_VEHICLE_ORDERS[i], Rect2(i * (bw + g), bh + g, bw, bh), "order")
		# S3c: Bail out in the fifth slot of the orders row.
		_add(BAIL, Rect2(4.0 * (bw + g), bh + g, bw, bh), "order")
		# S8a: Unload in the fifth slot of the top row.
		_add(DROP if _truck else UNLOAD, Rect2(4.0 * (abw + g), 0, abw, bh), "order")
		var aw := 4.0 * bw + 3.0 * g
		if not _gun:
			return
		for i in ROW_AMMO.size():
			_add(ROW_AMMO[i], Rect2(i * aw / ROW_AMMO.size(), 2.0 * (bh + g), aw / ROW_AMMO.size(), bh), "stance")
		return
	# S5a: a thin fire strip below the three button rows (the console is
	# `fire_h` + a gap taller, so the buttons keep their height).
	var fh: float = m.get("fire_h", 20.0)
	bh = (size.y - fh - 3.0 * g) / 3.0
	var top: Array = ROW_SUPPLY if grid_mode == "supply" else ROW_ABILITY
	for i in top.size():
		var def: Array = PANZERFAUST if i == 2 and _pzf_slot and grid_mode != "supply" else top[i]
		_add(def, Rect2(i * (abw + g), 0, abw, bh), "ability")
	for i in ROW_ORDERS.size():
		_add(ROW_ORDERS[i], Rect2(i * (bw + g), bh + g, bw, bh), "order")
	# S3c: the stance strip spans three columns; Crew takes the fourth.
	var sw := 3.0 * bw + 2.0 * g
	var seg := sw / 4.0
	for i in ROW_STANCE.size():
		_add(ROW_STANCE[i], Rect2(i * seg, 2.0 * (bh + g), seg, bh), "stance")
	if grid_mode == "supply":
		_add(PLACE, Rect2(3.0 * (bw + g), 2.0 * (bh + g), bw, bh), "order")
		_add(PICK_UP, Rect2(4.0 * (bw + g), 2.0 * (bh + g), bw, bh), "order")
	else:
		# S8a: Crew and Board share the fourth slot; a mounted selection
		# gets Unload in Regroup's.
		var half := (bw - g) * 0.5
		_add(CREW, Rect2(3.0 * (bw + g), 2.0 * (bh + g), half, bh), "order")
		_add(BOARD, Rect2(3.0 * (bw + g) + half + g, 2.0 * (bh + g), half, bh), "order")
		var mounted: bool = not view.selection.unload_vehicles().is_empty()
		_add(UNLOAD if mounted else REGROUP, Rect2(4.0 * (bw + g), 2.0 * (bh + g), bw, bh), "order")
	var fseg := sw / 3.0
	for i in ROW_FIRE.size():
		_add(ROW_FIRE[i], Rect2(i * fseg, 3.0 * (bh + g), fseg, fh), "fire")


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
	if id in STANCE_IDS or id == &"auto" or id in FIRE_IDS:
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
	if id in FIRE_IDS:
		return enabled(id) and "fire_" + sel.active_fire() == String(id)
	if id == &"bail":
		return sel.bail_armed()
	return false


## The label of laid-out button `id` ("" when it is not shown).
func label_of(id: StringName) -> String:
	for b in buttons:
		if b.id == id:
			return String(b.label)
	return ""


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
	elif id in FIRE_IDS:
		sel.set_fire(String(id).trim_prefix("fire_"))
	else:
		sel.activate(id)


func _draw() -> void:
	if view == null:
		return
	var font := get_theme_default_font()
	var bold := HudStyle.bold()
	var sel: Node = view.selection
	# Stance strip background (one segmented box); S5a: the fire strip too.
	for kind in ["stance", "fire"]:
		var strip := Rect2()
		for b in buttons:
			if b.kind == kind:
				strip = b.rect if strip.size == Vector2.ZERO else strip.merge(b.rect)
		if strip.size != Vector2.ZERO:
			HudStyle.box(self, strip, HudStyle.BG_BUTTON, 4)
	for b in buttons:
		var r: Rect2 = b.rect
		var id: StringName = b.id
		var on := enabled(id)
		var lit := active(id)
		if b.kind == "stance" or b.kind == "fire":
			if id != &"stand" and id != &"ammo_auto" and id != &"fire_discipline":
				var inset := 7.0 if b.kind == "stance" else 4.0
				draw_rect(Rect2(r.position.x - 0.5, r.position.y + inset, 1.0, r.size.y - 2.0 * inset), Color(HudStyle.TEXT, 0.2))
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
		if id in COUNTED:
			var n: int = sel.item_count(String(id))
			HudStyle.text(self, bold, Vector2(r.end.x - 6.0, r.position.y + 16.0), "×%d" % n, HudStyle.FONT_SMALL, txt_col, 1)
		# S3c: Bail out waits for its second press.
		var text: String = "Again?" if id == &"bail" and lit else b.label
		if b.kind == "fire":
			# S5a: the thin fire strip, small type, centred.
			var fl := HudStyle.fit(font, text, r.size.x - 6.0, HudStyle.FONT_SMALL)
			HudStyle.text(self, font, Vector2(r.get_center().x, r.get_center().y + 5.0), fl, HudStyle.FONT_SMALL, txt_col, 0)
			continue
		# S5b: a label too long for the button ("Fire mission") steps down
		# to the small type before it is cut.
		var fs := HudStyle.FONT
		if HudStyle.text_w(font, text, fs) > r.size.x - 6.0:
			fs = HudStyle.FONT_SMALL
		var label := HudStyle.fit(font, text, r.size.x - 6.0, fs)
		HudStyle.text(self, font, Vector2(r.get_center().x, r.end.y - 8.0), label, fs, txt_col, 0)


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
