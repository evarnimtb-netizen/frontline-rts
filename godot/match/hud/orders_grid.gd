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

## S10a, the build menu (engineers, assault and construction squads: a
## squad with build_structure, place_wire, clear_wire or breach). The
## Build button takes the Mission slot of the ability row when no mortar is
## selected; pressing it swaps the top two rows for:
##   row 1: Sandbags, Wire (lines: click points, right-click builds, as Dig
##          in), MG nest, Dugout, Roof HQ (click the spot; the camera's
##          forward is the facing);
##   row 2: Materials (load construction kg in the base, paid there),
##          Crate (a construction crate), Cut wire (click the wire),
##          Bangalore (click the wire: the nearest carrier lays it), Back.
## Row 3 stays the stance strip, unless the selection can use an S10b entry:
##   row 3 (S10b): AP mines, AT mines, Hedgehog, Log barrier, Bunker, Clear
##          mines (drag a lane: click points, right-click sweeps first to
##          last), Demolish (click a piece); only the ones it can use.
## Tooltips give kg, time and the capability from the rules
## (`rules.fortify`); buttons a squad cannot use are grey.
const BUILD := [&"build_menu", "Build", &"order_build_menu", "Fortifications: sandbags, wire, MG nest, dugout, HQ roof; materials, wire cutting, Bangalore. Engineer, assault and construction squads."]
const ROW_FORT := [&"build_sandbags", &"build_wire", &"build_mg_nest", &"build_dugout", &"roof_hq"]
const ROW_FORT_ORDERS := [
	[&"load_materials", "Materials", &"", "In the own base: the selected building squads load construction material (a full load of 20 kg a man unless the mod says otherwise), paid now. Builders draw it first, then own construction crates and trucks within 15 m."],
	[&"crate_construction", "Crate", &"", "In the own base: each selected building squad takes a 40 kg construction crate, paid now. Set it down near a site: builders draw from it."],
	[&"cut_wire", "Cut wire", &"", "Click wire: the nearest man cuts the nearest section (field engineers 10 s, assault engineers 5 s, construction engineers 20 s). Not while pinned."],
	[&"bangalore", "Bangalore", &"", "Click wire: the nearest man carrying a Bangalore torpedo walks up, lays it (5 s) and it bursts 8 s later, blowing a gap. Assault engineers only."],
	[&"build_menu", "Back", &"order_build_menu", "Back to the orders."],
]
## S10b: the third build-menu row (shown entry by entry to able squads).
const ROW_FORT_S10B := [&"lay_ap_mines", &"lay_at_mines", &"build_hedgehog", &"build_log_barrier", &"build_bunker", &"clear_mines", &"demolish"]
## S10a: the build-menu buttons' labels.
const FORT_LABELS := {&"build_sandbags": "Sandbags", &"build_wire": "Wire", &"build_mg_nest": "MG nest", &"build_dugout": "Dugout", &"roof_hq": "Roof HQ", &"lay_ap_mines": "AP mines", &"lay_at_mines": "AT mines", &"build_hedgehog": "Hedgehog", &"build_log_barrier": "Logs", &"build_bunker": "Bunker", &"clear_mines": "Clear mines", &"demolish": "Demolish"}

## S12: wounds and medics. The Aid button (the Mission slot of squads that
## cannot fire a mission or build; the build menu's third row otherwise)
## opens the aid menu: Tend, Carry, Evacuate, Reinforce and (medic squads)
## the aid post. No red cross anywhere: medics wear a white armband and
## carry a stretcher.
const AID := [&"aid_menu", "Aid", &"", "Wounds: tend and carry the wounded and downed, evacuate them, call replacements, build the aid post. Opens the aid menu."]
const ROW_AID := [
	[&"tend", "Tend", &"", "Click a wounded man or a downed one: the nearest selected man goes to him. A medic stabilises a downed man (15 s) so he stops bleeding out; anyone bandages a wounded man (10 s), who then loses no more HP."],
	[&"carry", "Carry", &"", "Click a downed man, then the ground: one or two selected men (medics first, on a stretcher) carry him there, slowly. Toward an aid post or the base, where Evacuate takes him off the map."],
	[&"evacuate", "Evacuate", &"", "Selected wounded men: at a staffed aid post with a free bed they are treated and return; in the base they leave the map (a paid man refunds part of his price). With none selected, click a downed man."],
	[&"reinforce", "Reinforce", &"", "Selected infantry squads fill their gaps from the base: the men are paid per head (at the card's price), walk to the squad and join. One order per card cooldown; not at the pop cap, not without manpower."],
	[&"build_aid", "Aid post", &"", "Click the spot (medic squads): a tent on a scrape with two beds. It works while two own medics stand within its ring: a stabilised downed man in the ring is evacuated, a wounded man is treated and returns to the fight. Shell fire that breaks the tent destroys it."],
]
const ROW_AID_ORDERS := [&"move", &"attack_move", &"stop", &"retreat"]
const AID_BACK := [&"aid_menu", "Back", &"", "Back to the orders."]
## S12: the build menu's last entries (Aid post for medic squads, then the Aid button).
const ROW_FORT_S12 := [&"build_aid", &"aid_menu"]

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
## S9: a gun selected (the gun grid; the keys are the ones its men do not
## use for anything else: Y as dig, L as regroup, V as board, U as unload,
## B as barrage).
const ROW_GUN := [
	[&"emplace", "Emplace", &"order_dig", "Set the gun up facing a point (click it): the crew spread the trail and dig the spades in. 25-30 s; it cannot fire until it is done. Limbered guns only."],
	[&"pack_gun", "Pack", &"order_regroup", "Limber the gun to move it (about 17 s). A gun being set up limbers at once."],
	[&"hitch", "Hitch", &"order_embark", "Click an own tractor or half-track near the limbered gun: the crew hitch it (20 s) and ride. The tow must stand still."],
	[&"unhitch", "Unhitch", &"order_disembark", "The tow stops and the crew take the gun off the hitch (20 s); then order Emplace."],
	[&"ambush", "Ambush", &"order_barrage", "Hold fire until a vehicle is this close (off, 400 m, 250 m); men only inside 200 m. The gun fires at once if it is struck."],
]
## S8b: trucks.
const DROP := [&"drop_crates", "Drop", &"order_drop_crates", "Click a point: the truck drives there and sets down up to 4 crates (3 s each), where they feed mortars and men or stock an HQ inside its ring. I arms the tool; pressing I again changes the kind (shells, small arms, explosives). Nothing is paid here: the truck was paid for when it loaded at the base."]
## S10a: a selected truck loads construction material in the base.
const TRUCK_MATERIALS := [&"load_materials", "Materials", &"", "In the own base: the truck fills its free room (its cap less the cargo) with construction material, paid now. Builders draw from own trucks within 15 m."]
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
const TOOLS := [&"tend", &"carry", &"evacuate", &"build_aid", &"emplace", &"hitch", &"push_gun", &"drop_crates", &"move", &"attack_move", &"dig", &"barrage", &"face", &"reverse", &"suppress", &"crew", &"embark", &"panzerfaust", &"build_hq", &"build_dump", &"place_crate", &"pick_up", &"build_sandbags", &"build_wire", &"build_mg_nest", &"build_dugout", &"roof_hq", &"cut_wire", &"bangalore", &"lay_ap_mines", &"lay_at_mines", &"build_hedgehog", &"build_log_barrier", &"build_bunker", &"clear_mines", &"demolish"]
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
## S10a: the Build button shows in the Mission slot.
var _build := false
var _s10b := ""
## S12: the Aid button shows in the Mission slot.
var _aid_slot := false


func bind(v: MatchView) -> void:
	view = v
	mouse_filter = Control.MOUSE_FILTER_STOP
	tooltip_text = " "


func update_state(_s: Dictionary) -> void:
	var sel: Node = view.selection
	var want := "vehicle" if not sel.selected_vehicles().is_empty() and sel.selected_soldiers().is_empty() else "soldier"
	if want == "vehicle" and sel.gun_only():
		want = "gun"
	if want == "soldier" and sel.supply_only():
		want = "supply"
	# S10a: the build menu while it is open and someone can use it.
	var build: bool = want == "soldier" and sel.can(&"build_menu") and not sel.can(&"barrage")
	if sel.fort_menu and not build:
		sel.fort_menu = false
	if want == "soldier" and sel.fort_menu:
		want = "fort"
	# S12: the aid menu while the selection can use it.
	var aid: bool = want == "soldier" and sel.can(&"aid_menu")
	if sel.aid_menu and not (aid or (want == "fort" and sel.can(&"aid_menu"))):
		sel.aid_menu = false
	if want == "soldier" and sel.aid_menu:
		want = "aid"
	var aid_slot: bool = aid and not build and not sel.can(&"barrage")
	var pzf: bool = sel.item_count("panzerfaust") > 0 and sel.item_count("satchel") == 0
	var mounted: bool = not sel.unload_vehicles().is_empty()
	var truck: bool = not sel.truck_vehicles().is_empty()
	var gun := false
	for id in sel.selected_vehicles():
		if (view.vehicles_by_id.get(id, {}).get("ammo", {}) as Dictionary).has("ap"):
			gun = true
	# S10b: the able third-row entries (Demolish follows the satchels).
	var s10b := ""
	if want == "fort":
		for id in ROW_FORT_S10B + ROW_FORT_S12:
			if sel.can(id):
				s10b += String(id) + ","
	if size != _size or buttons.is_empty() or want != grid_mode or pzf != _pzf_slot or truck != _truck or mounted != _mounted or gun != _gun or build != _build or s10b != _s10b or aid_slot != _aid_slot:
		_aid_slot = aid_slot
		_size = size
		_build = build
		_s10b = s10b
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
	if grid_mode == "gun":
		_layout_gun(abw, bw, bh, g)
		return
	if grid_mode == "vehicle":
		for i in ROW_VEHICLE.size():
			_add(ROW_VEHICLE[i], Rect2(i * (abw + g), 0, abw, bh), "order")
		for i in ROW_VEHICLE_ORDERS.size():
			_add(ROW_VEHICLE_ORDERS[i], Rect2(i * (bw + g), bh + g, bw, bh), "order")
		# S3c: Bail out in the fifth slot of the orders row.
		_add(BAIL, Rect2(4.0 * (bw + g), bh + g, bw, bh), "order")
		# S8a: Unload in the fifth slot of the top row.
		_add(DROP if _truck else UNLOAD, Rect2(4.0 * (abw + g), 0, abw, bh), "order")
		# S10a: Materials on the third row of a truck (no ammo row there).
		if _truck and not _gun and view.selection.fort_enabled():
			_add(TRUCK_MATERIALS, Rect2(0, 2.0 * (bh + g), bw, bh), "order")
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
	if grid_mode == "fort":
		for i in ROW_FORT.size():
			_add(_fort_def(ROW_FORT[i]), Rect2(i * (abw + g), 0, abw, bh), "ability")
		for i in ROW_FORT_ORDERS.size():
			_add(_fort_order_def(ROW_FORT_ORDERS[i]), Rect2(i * (bw + g), bh + g, bw, bh), "order")
		# S10b: the able entries take the stance strip's row.
		var able: Array = (ROW_FORT_S10B + ROW_FORT_S12).filter(func(id: StringName) -> bool: return view.selection.can(id))
		if not able.is_empty():
			var rw := 5.0 * bw + 4.0 * g
			var n := maxi(able.size(), 5)
			var ew := (rw - (n - 1) * g) / n
			for i in able.size():
				_add(_fort_def(able[i]), Rect2(i * (ew + g), 2.0 * (bh + g), ew, bh), "ability")
			var fseg0 := (3.0 * bw + 2.0 * g) / 3.0
			for i in ROW_FIRE.size():
				_add(ROW_FIRE[i], Rect2(i * fseg0, 3.0 * (bh + g), fseg0, fh), "fire")
			return
	else:
		var top: Array = ROW_SUPPLY if grid_mode == "supply" else (ROW_AID if grid_mode == "aid" else ROW_ABILITY)
		for i in top.size():
			var def: Array = top[i]
			if grid_mode == "soldier":
				if i == 2 and _pzf_slot:
					def = PANZERFAUST
				elif i == 3 and _build:
					def = BUILD
				elif i == 3 and _aid_slot:
					def = AID
			_add(def, Rect2(i * (abw + g), 0, abw, bh), "ability")
		for i in ROW_ORDERS.size():
			var odef: Array = ROW_ORDERS[i]
			if grid_mode == "aid":
				odef = AID_BACK if i == 4 else ROW_ORDERS[i]
			_add(odef, Rect2(i * (bw + g), bh + g, bw, bh), "order")
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


## S10a: a blueprint button with its kg, time, men and capability from
## `rules.fortify.blueprints`.
func _fort_def(id: StringName) -> Array:
	if id == &"aid_menu":
		return AID
	if id == &"build_aid":
		return ROW_AID[4]
	if id == &"clear_mines" or id == &"demolish":
		return [id, FORT_LABELS.get(id, String(id)), &"", _s10b_order_tip(id)]
	var bp: Dictionary = view.selection.fort_blueprint(id)
	var tip := "Not in this mod."
	if not bp.is_empty() and bp.get("mine", false):
		# S10b: a minefield: one mine at each segment end.
		tip = "%s: one mine every %s m along a line up to %d m (click points, right-click lays). %s kg and %d s with %d men a mine (%s). Hidden from the enemy until found or set off. Limit %d mines." % [
			bp.get("name", ""), _num(bp.get("segment_m", 0.0)), int(bp.get("max_length_m", 0.0)), _num(bp.get("kg", 0.0)),
			int(round(float(bp.get("work_s", 0.0)))), int(bp.get("work_men", 0)), String(bp.get("capability", "")).replace("_", " "), int(bp.get("limit", 0))]
		return [id, FORT_LABELS.get(id, String(id)), &"", tip]
	if not bp.is_empty():
		var per := " a 2 m segment" if bp.get("shape", "") == "line" else ""
		var how := "Click points along the ground (up to %d m), right-click to build." % int(bp.get("max_length_m", 0.0)) if bp.get("shape", "") == "line" else "Click the spot; it faces the way the camera looks."
		if bp.get("shape", "") == "over_hq":
			how = "Click an own built HQ (within 5 m): a roof over its pit."
		tip = "%s: %s kg and %d s with %d men%s (%s). %s%s Limit %d." % [
			bp.get("name", ""), _num(bp.get("kg", 0.0)), int(round(float(bp.get("work_s", 0.0)))),
			int(bp.get("work_men", 0)), per, String(bp.get("capability", "")).replace("_", " "), how,
			" Dug first: its pit." if bp.get("pit", false) else "", int(bp.get("limit", 0))]
		if bp.get("blocks_vehicles", false):
			tip += " Stops vehicles; infantry walks through."
		# S10b R21: field engineers build heavy blueprints at half rate.
		if view.selection.fort_half_rate(bp):
			tip += " Field engineers build it at %d %% of their rate." % int(view.rules().get("fortify", {}).get("heavy_by_field_pct", 50))
	return [id, FORT_LABELS.get(id, String(id)), &"", tip]


## S10b: Clear mines and Demolish tooltips from `rules.fortify`.
func _s10b_order_tip(id: StringName) -> String:
	var f: Dictionary = view.rules().get("fortify", {})
	if id == &"clear_mines":
		return "Drag a lane (click points, right-click; first to last, up to %d m): two men sweep it slowly, find mines nearby and lift each found one (%d s at field-engineer pace). Assault and engineer squads." % [int(f.get("lane_max_m", 50)), int(f.get("lift_s", 20))]
	return "Click an obstacle or bunker (within %d m of the click): the nearest man with a satchel walks up, places it (%d s) and it bursts %d s later, wrecking the piece. Squads with satchels." % [int(f.get("demolish_reach_m", 25)), int(f.get("demolish_place_s", 10)), int(f.get("demolish_fuse_s", 10))]


## S10a: Materials names the carried kg per man and the price.
func _fort_order_def(def: Array) -> Array:
	if def[0] != &"load_materials":
		return def
	var f: Dictionary = view.rules().get("fortify", {})
	if f.is_empty():
		return def
	var tip := "In the own base: the selected building squads load construction material, %d kg a man, paid now (%d MP a tonne). Builders draw it first, then own construction crates and trucks within 15 m." % [int(f.get("carry_kg_per_man", 0)), int(f.get("mp_per_t", 0))]
	return [def[0], def[1], def[2], tip]


func _num(x: Variant) -> String:
	var v := float(x)
	return str(int(v)) if is_equal_approx(v, round(v)) else "%.1f" % v


## S9: the gun orders on top, Push and Ammo below.
func _layout_gun(abw: float, bw: float, bh: float, g: float) -> void:
	for i in ROW_GUN.size():
		_add(ROW_GUN[i], Rect2(i * (abw + g), 0, abw, bh), "order")
	_add([&"push_gun", "Push", &"order_move", "Click a point: the crew push the limbered gun there. Right-click on the ground does the same."], Rect2(0, bh + g, bw, bh), "order")
	_add(ROW_VEHICLE_ORDERS[2], Rect2(bw + g, bh + g, bw, bh), "order")
	if not _gun:
		return
	var aw := 4.0 * bw + 3.0 * g
	for i in ROW_AMMO.size():
		_add(ROW_AMMO[i], Rect2(i * aw / ROW_AMMO.size(), 2.0 * (bh + g), aw / ROW_AMMO.size(), bh), "stance")


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
	if id == &"build_menu":
		return sel.fort_menu
	if id == &"aid_menu":
		return sel.aid_menu
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
