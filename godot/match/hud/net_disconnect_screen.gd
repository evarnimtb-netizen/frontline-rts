class_name NetDisconnectScreen
extends Control
## Net mode: the link to the relay is gone for good (state
## `net.disconnect_reason` set). A dimmed screen with the reason and
## "Back to menu" (DisconnectPanel, shared with the lobby). Like the end
## screen it owns input while shown; the end screen wins if the match
## ended first.

var view: MatchView
var panel: DisconnectPanel


func bind(v: MatchView) -> void:
	view = v
	visible = false
	mouse_filter = Control.MOUSE_FILTER_STOP
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var dim := ColorRect.new()
	dim.color = Color(0.03, 0.035, 0.04, 0.78)
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(dim)
	var center := CenterContainer.new()
	center.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	center.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(center)
	var box := PanelContainer.new()
	box.custom_minimum_size = Vector2(520, 0)
	center.add_child(box)
	panel = DisconnectPanel.new()
	panel.back.connect(func() -> void: view.emit_intent(&"quit_to_menu", {}))
	box.add_child(panel)


func update_state(_s: Dictionary) -> void:
	var reason := String(view.net.get("disconnect_reason", "")) if view.net_mode else ""
	var show := reason != "" and not view.ended
	if show and not visible:
		panel.set_reason(reason, true)
		view.selection.cancel_tool()
		view.pause_menu.close(false)
		AudioManager.play_ui(&"error")
	visible = show
