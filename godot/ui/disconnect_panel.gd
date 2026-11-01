class_name DisconnectPanel
extends VBoxContainer
## "Disconnected" page (S2 Presenter): the reason, then back to the menu.
## Shared by the multiplayer lobby (a lobby or connection that ended) and
## the match (the link to the relay is gone). Reason codes become text
## through NetUi.reason_text; an unknown code shows as given.

signal back

## Reasons where checking the address or the network is the useful hint.
const ADDRESS_CODES := ["no_answer", "bad_address", "bind_failed", "timed_out", "connection_lost"]

var title: Label
var reason: Label
var hint: Label
var back_button: Button


func _init() -> void:
	add_theme_constant_override("separation", 14)
	title = Label.new()
	title.text = "DISCONNECTED"
	title.theme_type_variation = &"HeaderLabel"
	title.add_theme_color_override("font_color", UiPalette.DANGER)
	add_child(title)
	reason = Label.new()
	reason.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	reason.custom_minimum_size.x = 440
	add_child(reason)
	hint = Label.new()
	hint.theme_type_variation = &"DimLabel"
	hint.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	hint.custom_minimum_size.x = 440
	add_child(hint)
	back_button = Button.new()
	back_button.text = "BACK TO MENU"
	back_button.theme_type_variation = &"PrimaryButton"
	back_button.custom_minimum_size = Vector2(240, 50)
	back_button.size_flags_horizontal = Control.SIZE_SHRINK_END
	back_button.pressed.connect(func() -> void: back.emit())
	add_child(back_button)


## Shows `code` (a reason code or free text). `in_match`: the match is
## over for this player; the hint says what happens to it.
## `detail`: more about the reason (a build mismatch names both builds).
func set_reason(code: String, in_match := false, detail := "") -> void:
	var text := NetUi.reason_text(code)
	reason.text = text if not text.is_empty() else "The connection ended."
	if detail != "":
		reason.text += " " + detail
	title.text = "CAN'T JOIN" if code in NetUi.REJECT_CODES else "DISCONNECTED"
	back_button.text = "BACK TO MENU" if in_match else "BACK"
	if in_match:
		hint.text = "The match cannot continue on this machine. If only your link failed and your opponent is still connected, they win after the relay's 60 s timeout. If the host's game closed, the match is over for both."
	elif code in ADDRESS_CODES:
		hint.text = "Check the address and port, then try again. Over the internet, see %s." % NetUi.GUIDE
	else:
		hint.text = ""
	hint.visible = hint.text != ""
