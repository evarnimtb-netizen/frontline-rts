extends Control
## Match menu (Esc): resume, settings, surrender, quit to menu. Surrender and
## quit ask first. Opening it requests a pause through MatchView (the source
## decides what pausing means; a vs-AI match stops the sim, §10 lockstep
## matches will not).
##
## Net mode (S2): opening the menu does not pause the match; "Pause match"
## / "Resume match" does, for both players (either may resume). Surrender
## and Leave name the other player instead of the AI.

var view: MatchView
var _main: VBoxContainer
var _settings: SettingsPanel
var _confirm: VBoxContainer
var _confirm_text: Label
var _confirm_ok: Button
var _head: Label
var _resume_btn: Button
## Net mode only: pauses or resumes the match for both players.
var net_pause_button: Button
var _pending: StringName = &""


func bind(v: MatchView) -> void:
	view = v
	visible = false
	mouse_filter = Control.MOUSE_FILTER_STOP
	var dim := ColorRect.new()
	dim.color = Color(0, 0, 0, 0.55)
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(dim)
	var center := CenterContainer.new()
	center.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	center.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(center)
	var panel := PanelContainer.new()
	panel.custom_minimum_size = Vector2(420, 0)
	center.add_child(panel)
	var stack := VBoxContainer.new()
	panel.add_child(stack)

	_main = VBoxContainer.new()
	_main.add_theme_constant_override("separation", 10)
	stack.add_child(_main)
	_head = Label.new()
	_head.text = "PAUSED"
	_head.theme_type_variation = &"HeaderLabel"
	_main.add_child(_head)
	_resume_btn = _button(_main, "Resume", func() -> void: close())
	net_pause_button = _button(_main, "Pause match", _toggle_net_pause)
	net_pause_button.visible = false
	_button(_main, "Settings", func() -> void: _page("settings"))
	_button(_main, "Surrender", func() -> void: _ask(&"surrender"))
	_button(_main, "Quit to menu", func() -> void: _ask(&"quit_to_menu"))

	_settings = SettingsPanel.new()
	_settings.closed.connect(_page.bind("main"))
	stack.add_child(_settings)

	_confirm = VBoxContainer.new()
	_confirm.add_theme_constant_override("separation", 12)
	stack.add_child(_confirm)
	_confirm_text = Label.new()
	_confirm_text.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_confirm_text.custom_minimum_size.x = 380
	_confirm.add_child(_confirm_text)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 10)
	_confirm.add_child(row)
	_confirm_ok = Button.new()
	_confirm_ok.theme_type_variation = &"PrimaryButton"
	_confirm_ok.pressed.connect(_do_confirm)
	row.add_child(_confirm_ok)
	var cancel := Button.new()
	cancel.text = "Cancel"
	cancel.pressed.connect(_page.bind("main"))
	row.add_child(cancel)
	_page("main")


func _button(parent: Control, text: String, cb: Callable) -> Button:
	var b := Button.new()
	b.text = text
	b.custom_minimum_size = Vector2(0, 44)
	b.pressed.connect(cb)
	parent.add_child(b)
	return b


func _page(which: String) -> void:
	_main.visible = which == "main"
	_settings.visible = which == "settings"
	_confirm.visible = which == "confirm"
	if which == "settings":
		_settings.refresh()
	elif which == "main" and is_visible_in_tree():
		(_main.get_child(1) as Button).grab_focus()


func _ask(what: StringName) -> void:
	_pending = what
	var foe := "The AI"
	if view.net_mode:
		var n := String(view.player(1 - view.local_player).get("name", ""))
		foe = n if n != "" else "Your opponent"
	if what == &"surrender":
		_confirm_text.text = "Surrender this match? %s wins and the match ends." % foe
		_confirm_ok.text = "SURRENDER"
	elif view.net_mode:
		_confirm_text.text = "Leave the match and return to the main menu? You disconnect; %s wins once the relay's 60 s timeout runs out. Surrender first to end it at once." % foe
		_confirm_ok.text = "LEAVE MATCH"
	else:
		_confirm_text.text = "Leave the match and return to the main menu? The match counts as a loss."
		_confirm_ok.text = "QUIT TO MENU"
	_page("confirm")
	_confirm_ok.grab_focus()


func _do_confirm() -> void:
	var what := _pending
	_pending = &""
	# Surrender resumes so the source can finish the match; quitting
	# leaves the scene anyway.
	close(what == &"surrender")
	if what == &"surrender":
		view.emit_intent(&"surrender", {})
	elif what == &"quit_to_menu":
		view.emit_intent(&"quit_to_menu", {})


func open() -> void:
	if visible or view.ended:
		return
	visible = true
	_refresh_net()
	_page("main")
	# A net match runs on while the menu is open (S2): pausing is a
	# separate, explicit choice for both players.
	if not view.net_mode:
		view.emit_intent(&"pause", {"paused": true})


## Hides the menu; `resume` = false leaves the pause state to the caller.
func close(resume := true) -> void:
	if not visible:
		return
	visible = false
	if resume and not view.net_mode:
		view.emit_intent(&"pause", {"paused": false})


## Opens the menu on the Leave confirmation (the desync banner's Leave).
func ask_leave() -> void:
	if not visible:
		if view.ended:
			return
		visible = true
		if not view.net_mode:
			view.emit_intent(&"pause", {"paused": true})
	_ask(&"quit_to_menu")
	_confirm_ok.grab_focus()


func _refresh_net() -> void:
	var net := view.net_mode
	_head.text = "MATCH MENU" if net else "PAUSED"
	_resume_btn.text = "Back to the match" if net else "Resume"
	net_pause_button.visible = net
	if net:
		var st := String(view.net.get("state", "ok"))
		net_pause_button.text = "Resume match" if st == "paused" else "Pause match (both players)"
		net_pause_button.disabled = st in ["waiting", "desync"]


func _toggle_net_pause() -> void:
	var resume := String(view.net.get("state", "ok")) == "paused"
	view.emit_intent(&"pause", {"paused": not resume})
	close(false)


func toggle() -> void:
	if visible:
		if not _main.visible:
			_page("main")
		else:
			close()
	else:
		open()


func update_state(_s: Dictionary) -> void:
	if visible and view.net_mode:
		_refresh_net()
