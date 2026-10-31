class_name MatchInput
extends RefCounted
## Match input actions, registered at runtime so the defaults live in one
## place and can be rebound through InputMap later (§5.12.2). Physical
## keycodes, so the layout does not move the keys.
##
## Camera keys (arrows, Q/E, wheel, middle-drag) stay in rts_camera.gd.

const ACTIONS := {
	&"order_move": KEY_M,
	&"order_attack_move": KEY_A,
	&"order_stop": KEY_S,
	&"order_retreat": KEY_R,
	&"order_dig": KEY_D,
	&"order_barrage": KEY_B,
	&"dig_undo_point": KEY_BACKSPACE,
	&"jump_to_event": KEY_SPACE,
	&"match_menu": KEY_ESCAPE,
	&"match_menu_alt": KEY_F10,
}

## Hotkey labels for buttons and tooltips.
const LABELS := {
	&"order_move": "M",
	&"order_attack_move": "A",
	&"order_stop": "S",
	&"order_retreat": "R",
	&"order_dig": "D",
	&"order_barrage": "B",
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
