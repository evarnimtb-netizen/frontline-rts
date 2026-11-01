class_name DataErrorBanner
extends PanelContainer
## Debug builds (S3b quick iteration): "Reload data & restart" (F5, pause
## menu) found broken mod data. Shows the loader's message (file, key, fix)
## across the top of the HUD instead of ending the match; the match keeps
## running on the data it started with. Dismiss hides it; a reload that
## succeeds restarts the scene, which drops it.

var view: MatchView
var _text: Label
var message := ""


func bind(v: MatchView) -> void:
	view = v
	visible = false
	mouse_filter = Control.MOUSE_FILTER_STOP
	set_anchors_and_offsets_preset(Control.PRESET_CENTER_TOP)
	custom_minimum_size = Vector2(720, 0)
	offset_top = 64.0
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.10, 0.05, 0.04, 0.92)
	sb.border_color = HudStyle.DANGER
	sb.set_border_width_all(2)
	sb.set_corner_radius_all(4)
	sb.set_content_margin_all(12)
	add_theme_stylebox_override(&"panel", sb)
	var stack := VBoxContainer.new()
	stack.add_theme_constant_override("separation", 8)
	add_child(stack)
	var head := Label.new()
	head.text = "MOD DATA ERROR - match not restarted"
	head.add_theme_color_override("font_color", HudStyle.DANGER)
	stack.add_child(head)
	_text = Label.new()
	_text.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_text.custom_minimum_size.x = 690
	_text.add_theme_color_override("font_color", HudStyle.TEXT)
	_text.add_theme_font_size_override("font_size", HudStyle.FONT_SMALL)
	stack.add_child(_text)
	var row := HBoxContainer.new()
	row.alignment = BoxContainer.ALIGNMENT_END
	stack.add_child(row)
	var hint := Label.new()
	hint.text = "Fix the file, then press F5 again."
	hint.add_theme_color_override("font_color", HudStyle.DIM)
	hint.add_theme_font_size_override("font_size", HudStyle.FONT_SMALL)
	hint.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	row.add_child(hint)
	var close := Button.new()
	close.text = "Dismiss"
	close.pressed.connect(hide_error)
	row.add_child(close)


func show_error(text: String) -> void:
	message = text
	# The loader lists every problem; long lists are cut to stay on screen.
	var lines := text.split("\n")
	if lines.size() > 14:
		lines = lines.slice(0, 14)
		lines.append("... (%d more lines)" % (text.split("\n").size() - 14))
	_text.text = "\n".join(lines)
	visible = true
	AudioManager.play_ui(&"error")


func hide_error() -> void:
	visible = false
	message = ""
