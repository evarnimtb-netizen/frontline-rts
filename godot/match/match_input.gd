class_name MatchInput
extends RefCounted
## Match input actions, registered at runtime so the defaults live in one
## place and can be rebound through InputMap later (§5.12.2). Physical
## keycodes, so the layout does not move the keys.
##
## Defaults are the owner's S1b hotkeys (docs/slices/S1b-hud.md): orders
## RMB move, Ctrl+RMB attack-move, H stop, R retreat, Y dig, B barrage,
## L regroup; abilities G frag, T smoke, F satchel; stance X stand,
## C crouch, Z prone (the direct-control keys; pressing the active stance
## again returns to Auto). WASD, arrows and Q/E stay on the camera
## (rts_camera.gd). M arms the move tool (no default button needs it).
## S3 vehicles: O face tool, N next ammo (Auto → AP → HE), J repair;
## Alt+right-click reverses (a mouse chord, no action). F5 (debug builds
## only) reloads the mod data and restarts the match (S3b). S3c: K arms the
## Crew tool (click an abandoned vehicle: the selected infantry re-crew it;
## owner question S3c-6 (A)); Bail out has no key (a button that asks for
## a second click). S8a (owner question S8a-7 (A)): V arms the Board tool
## (click an own half-track: the selected squads board it; right-click on
## it does the same), U unloads the selected half-tracks (or the selected
## mounted squads' half-tracks) by the door, Shift+U over the sides.

const ACTIONS := {
	&"order_move": KEY_M,
	&"order_stop": KEY_H,
	&"order_retreat": KEY_R,
	&"order_dig": KEY_Y,
	&"order_barrage": KEY_B,
	&"order_regroup": KEY_L,
	&"ability_frag": KEY_G,
	&"ability_smoke": KEY_T,
	&"ability_satchel": KEY_F,
	&"stance_stand": KEY_X,
	&"stance_crouch": KEY_C,
	&"stance_prone": KEY_Z,
	&"order_face": KEY_O,
	&"ammo_next": KEY_N,
	&"order_repair": KEY_J,
	# S3b Suppress (area fire), owner question S3b-1 (A).
	&"order_suppress": KEY_P,
	# S3c re-crew tool, owner question S3c-6 (A).
	&"order_crew": KEY_K,
	# S8a transport, owner question S8a-7 (A).
	&"order_embark": KEY_V,
	&"order_disembark": KEY_U,
	# S8b: Drop Crates (S8b-19 (A)).
	&"order_drop_crates": KEY_I,
	&"focus_call_in": KEY_TAB,
	&"dig_undo_point": KEY_BACKSPACE,
	&"jump_to_event": KEY_SPACE,
	&"match_menu": KEY_ESCAPE,
	&"match_menu_alt": KEY_F10,
	# Debug builds only (S3b): reload mods/ TOML and restart the match.
	&"debug_reload_data": KEY_F5,
}

## Hotkey labels for buttons and tooltips. Attack-move has no action: it
## is Ctrl+right-click (or its button, which arms the tool).
const LABELS := {
	&"order_move": "RMB",
	&"order_attack_move": "Ctrl+RMB",
	&"order_stop": "H",
	&"order_retreat": "R",
	&"order_dig": "Y",
	&"order_barrage": "B",
	&"order_regroup": "L",
	&"ability_frag": "G",
	&"ability_smoke": "T",
	&"ability_satchel": "F",
	&"stance_stand": "X",
	&"stance_crouch": "C",
	&"stance_prone": "Z",
	&"order_reverse": "Alt+RMB",
	&"order_face": "O",
	&"ammo_next": "N",
	&"order_repair": "J",
	&"order_suppress": "P",
	&"order_crew": "K",
	&"order_embark": "V",
	&"order_disembark": "U",
	&"order_drop_crates": "I",
	&"focus_call_in": "Tab",
}


static func ensure_actions() -> void:
	for action in ACTIONS:
		if InputMap.has_action(action):
			continue
		InputMap.add_action(action)
		var ev := InputEventKey.new()
		ev.physical_keycode = ACTIONS[action]
		InputMap.action_add_event(action, ev)
	for i in range(1, 10):
		var action := StringName("group_%d" % i)
		if InputMap.has_action(action):
			continue
		InputMap.add_action(action)
		var ev := InputEventKey.new()
		ev.physical_keycode = KEY_0 + i
		InputMap.action_add_event(action, ev)


## Group number 1..9 for a key event, or 0.
static func group_of(event: InputEvent) -> int:
	for i in range(1, 10):
		if event.is_action_pressed(StringName("group_%d" % i), false, false):
			return i
	return 0
