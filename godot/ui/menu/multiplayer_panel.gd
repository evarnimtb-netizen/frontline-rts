class_name MultiplayerPanel
extends VBoxContainer
## Main menu → Multiplayer (S2 Presenter). Four pages, picked each frame
## from the net status (Session.net, FrontlineNet or MockNet):
##
##   connect       name; Host (port, default 47015) or Join (address:port,
##                 IPv4, [IPv6] or host name), validated before any call
##   connecting    "Connecting to …" and Cancel
##   lobby         both slots (name, faction, ping, ready); own faction;
##                 slot 0 edits seed, map and input delay D (2–6, with the
##                 computed suggestion), slot 1 sees them; factions must
##                 differ; Ready; mismatch / reject reason; 3-2-1 countdown
##                 over the page; Leave
##   disconnected  the reason, then back to the connect page
##
## "in_match" hands over to Session.start_net_match(). The panel never
## decides lobby rules on its own beyond disabling Ready when they cannot
## pass (no opponent, same faction); the relay is the judge.

const NAME_MAX := 24
## Example addresses shown instead of the machine's own in screenshots.
const CAPTURE_ADDRESSES := ["192.168.1.20", "100.101.102.103"]

var page := ""
## Last status read (tests read it too).
var status: Dictionary = {}
## Test / screenshot hook: shown instead of the machine's LAN addresses.
var addresses_override: Variant = null

var name_edit: LineEdit
var host_port: LineEdit
var host_button: Button
var host_error: Label
var host_hint: Label
var join_addr: LineEdit
var join_button: Button
var join_error: Label

var connecting_label: Label

var lobby_where: Label
var slot_rows: Array = []
var faction_btns := {}
var map_pick: OptionButton
var map_ro: Label
var seed_spin: SpinBox
var seed_ro: Label
var seed_dice: Button
var delay_spin: SpinBox
var delay_ro: Label
var delay_note: Label
var delay_suggest: Button
var lobby_msg: Label
var ready_button: Button
var leave_button: Button
var countdown: Label
var countdown_box: Control

var disconnect_panel: DisconnectPanel

var _pages := {}
## What we asked for, for the lobby's "where" line.
var where := ""
var _map_ids: Array = []
## Last values pushed to the widgets from the status (so typing is not
## overwritten every frame).
var _shown := {}


func _ready() -> void:
	add_theme_constant_override("separation", 0)
	_pages.connect = _build_connect()
	_pages.connecting = _build_connecting()
	_pages.lobby = _build_lobby()
	disconnect_panel = DisconnectPanel.new()
	disconnect_panel.back.connect(_on_disconnect_back)
	_pages.disconnected = disconnect_panel
	for k in _pages:
		add_child(_pages[k])
	_show_page("connect")


func _process(_delta: float) -> void:
	if is_visible_in_tree():
		refresh()


## Reads the net status and updates the page (called every frame while
## visible; tests call it directly).
func refresh() -> void:
	var net: Variant = Session.net
	if net == null:
		_show_page("connect")
		return
	status = net.status()
	var st: String = status.get("state", "idle")
	match st:
		"connecting":
			_show_page("connecting")
			connecting_label.text = "Connecting to %s …" % where
		"lobby", "countdown":
			_show_page("lobby")
			_refresh_lobby()
		"in_match":
			if page != "starting":
				page = "starting"
				Session.start_net_match()
		"disconnected":
			_show_page("disconnected")
			disconnect_panel.set_reason(String(status.get("error", "")))
		_:
			_show_page("connect")


func _show_page(which: String) -> void:
	if page == which:
		return
	page = which
	for k in _pages:
		_pages[k].visible = k == which
	if which == "lobby":
		_shown.clear()


# --- connect page -----------------------------------------------------------------

func _build_connect() -> Control:
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 14)
	box.add_child(_head("MULTIPLAYER"))
	box.add_child(_dim("One match, two players. Host on this machine, or join a friend's game by address."))

	box.add_child(_section("Your name"))
	name_edit = LineEdit.new()
	name_edit.max_length = NAME_MAX
	name_edit.text = Session.player_name
	name_edit.custom_minimum_size.x = 260
	name_edit.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	name_edit.text_changed.connect(func(t: String) -> void: Session.player_name = t.strip_edges())
	box.add_child(name_edit)

	var cols := HBoxContainer.new()
	cols.add_theme_constant_override("separation", 16)
	box.add_child(cols)

	var hostc := _card(cols)
	hostc.add_child(_sub("HOST"))
	hostc.add_child(_dim("Runs the relay in this game. Your friend joins your address."))
	var hrow := HBoxContainer.new()
	hrow.add_theme_constant_override("separation", 8)
	hrow.add_child(_label("Port"))
	host_port = LineEdit.new()
	host_port.text = str(NetUi.DEFAULT_PORT)
	host_port.max_length = 5
	host_port.custom_minimum_size.x = 96
	host_port.text_submitted.connect(func(_t: String) -> void: on_host())
	host_port.text_changed.connect(func(_t: String) -> void: host_error.text = "")
	hrow.add_child(host_port)
	hostc.add_child(hrow)
	host_error = _error_label()
	hostc.add_child(host_error)
	host_hint = _dim("")
	hostc.add_child(host_hint)
	var hfill := Control.new()
	hfill.size_flags_vertical = Control.SIZE_EXPAND_FILL
	hostc.add_child(hfill)
	host_button = Button.new()
	host_button.text = "HOST GAME"
	host_button.theme_type_variation = &"PrimaryButton"
	host_button.custom_minimum_size = Vector2(0, 48)
	host_button.pressed.connect(on_host)
	hostc.add_child(host_button)

	var joinc := _card(cols)
	joinc.add_child(_sub("JOIN"))
	joinc.add_child(_dim("The host's address and port: LAN, Tailscale or a relay."))
	join_addr = LineEdit.new()
	join_addr.placeholder_text = "192.168.1.20:%d" % NetUi.DEFAULT_PORT
	join_addr.max_length = 300
	join_addr.text = Session.last_join
	join_addr.text_submitted.connect(func(_t: String) -> void: on_join())
	join_addr.text_changed.connect(func(_t: String) -> void: join_error.text = "")
	joinc.add_child(join_addr)
	join_error = _error_label()
	joinc.add_child(join_error)
	joinc.add_child(_dim("IPv6: [addr]:port. Without a port, %d." % NetUi.DEFAULT_PORT))
	var jfill := Control.new()
	jfill.size_flags_vertical = Control.SIZE_EXPAND_FILL
	joinc.add_child(jfill)
	join_button = Button.new()
	join_button.text = "JOIN GAME"
	join_button.theme_type_variation = &"PrimaryButton"
	join_button.custom_minimum_size = Vector2(0, 48)
	join_button.pressed.connect(on_join)
	joinc.add_child(join_button)

	box.add_child(_dim("Over the internet: Tailscale (recommended) or a relay on a small server; see docs/net/playing-online.md."))
	update_host_hint()
	return box


## Host button: validates the port, then FrontlineNet.host(port).
func on_host() -> void:
	var p := NetUi.parse_port(host_port.text, true)
	if not p.ok:
		host_error.text = p.error
		AudioManager.play_ui(&"error")
		return
	host_error.text = ""
	var net: Variant = Session.ensure_net()
	_apply_name(net)
	where = "port %d" % p.port
	if not net.host(p.port):
		# The status says why (bind_failed, ...); the refresh shows it.
		host_error.text = NetUi.reason_text(String(net.status().get("error", "bind_failed")))
		AudioManager.play_ui(&"error")
		return
	update_host_hint(p.port)
	AudioManager.play_ui(&"confirm")
	refresh()


## Join button: validates and normalises the address, then join(addr).
func on_join() -> void:
	var a := NetUi.parse_join(join_addr.text)
	if not a.ok:
		join_error.text = a.error
		AudioManager.play_ui(&"error")
		return
	join_error.text = ""
	Session.last_join = join_addr.text.strip_edges()
	var net: Variant = Session.ensure_net()
	_apply_name(net)
	where = a.addr
	if not net.join(a.addr):
		join_error.text = NetUi.reason_text(String(net.status().get("error", "bad_address")))
		AudioManager.play_ui(&"error")
		return
	AudioManager.play_ui(&"confirm")
	refresh()


func _apply_name(net: Object) -> void:
	var n := name_edit.text.strip_edges()
	Session.player_name = n if not n.is_empty() else "Player"
	if net.has_method("set_name"):
		net.set_name(Session.player_name)


func update_host_hint(port := -1) -> void:
	var addrs := lan_addresses()
	var p := port if port > 0 else NetUi.DEFAULT_PORT
	if addrs.is_empty():
		host_hint.text = "No LAN address found. Friends on this Mac join 127.0.0.1:%d." % p
	else:
		var shown := PackedStringArray()
		for a in addrs.slice(0, 2):
			shown.append("%s:%d" % [a, p])
		host_hint.text = "Friends join: " + "  or  ".join(shown)


## This machine's addresses a friend could join: private IPv4 (LAN) and
## Tailscale's 100.64/10, LAN first.
func lan_addresses() -> Array:
	if addresses_override != null:
		return addresses_override
	var lan: Array = []
	var ts: Array = []
	for a in IP.get_local_addresses():
		if a.contains(":"):
			continue
		var o := a.split(".")
		if o.size() != 4:
			continue
		var o0 := o[0].to_int()
		var o1 := o[1].to_int()
		if o0 == 10 or (o0 == 192 and o1 == 168) or (o0 == 172 and o1 >= 16 and o1 <= 31):
			lan.append(a)
		elif o0 == 100 and o1 >= 64 and o1 <= 127:
			ts.append(a)
	lan.sort()
	ts.sort()
	return lan + ts


# --- connecting page ------------------------------------------------------------

func _build_connecting() -> Control:
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 16)
	box.add_child(_head("CONNECTING"))
	connecting_label = Label.new()
	box.add_child(connecting_label)
	box.add_child(_dim("Waiting for the relay to answer. It gives up after a few seconds if nobody does."))
	var cancel := Button.new()
	cancel.text = "Cancel"
	cancel.custom_minimum_size = Vector2(140, 44)
	cancel.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	cancel.pressed.connect(on_leave)
	box.add_child(cancel)
	return box


# --- lobby page -------------------------------------------------------------------

func _build_lobby() -> Control:
	# The countdown is a second child of the same container, over the page.
	var outer := MarginContainer.new()
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 12)
	outer.add_child(box)

	var top := HBoxContainer.new()
	top.add_child(_head("LOBBY"))
	var tfill := Control.new()
	tfill.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	top.add_child(tfill)
	lobby_where = _dim("")
	lobby_where.autowrap_mode = TextServer.AUTOWRAP_OFF
	lobby_where.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	top.add_child(lobby_where)
	box.add_child(top)

	box.add_child(_section("Players"))
	for i in 2:
		slot_rows.append(_slot_row(box))

	box.add_child(_section("Your faction"))
	var fac_row := HBoxContainer.new()
	fac_row.add_theme_constant_override("separation", 10)
	var group := ButtonGroup.new()
	for f in Session.FACTIONS:
		var b := Button.new()
		b.text = UiPalette.FACTION_NAMES[f]
		b.toggle_mode = true
		b.button_group = group
		b.custom_minimum_size = Vector2(150, 40)
		b.add_theme_color_override("font_pressed_color", UiPalette.faction_color(f).lightened(0.35))
		var stripe := ColorRect.new()
		stripe.color = UiPalette.faction_color(f)
		stripe.mouse_filter = Control.MOUSE_FILTER_IGNORE
		stripe.set_anchors_and_offsets_preset(Control.PRESET_LEFT_WIDE)
		stripe.offset_right = 5
		b.add_child(stripe)
		b.pressed.connect(pick_faction.bind(f))
		faction_btns[f] = b
		fac_row.add_child(b)
	box.add_child(fac_row)

	box.add_child(_section("Match (set by the host)"))
	var grid := GridContainer.new()
	grid.columns = 3
	grid.add_theme_constant_override("h_separation", 14)
	grid.add_theme_constant_override("v_separation", 8)
	box.add_child(grid)

	map_pick = OptionButton.new()
	_map_ids = Session.MAPS.keys()
	for id in _map_ids:
		map_pick.add_item(Session.MAPS[id].name)
	map_pick.custom_minimum_size.x = 200
	map_pick.item_selected.connect(func(_i: int) -> void: _send_choice())
	map_ro = Label.new()
	_grid_row(grid, "Map", map_pick, map_ro, Control.new())

	seed_spin = SpinBox.new()
	seed_spin.min_value = 0
	seed_spin.max_value = 2147483647
	seed_spin.step = 1
	seed_spin.custom_minimum_size.x = 140
	seed_spin.value_changed.connect(func(_v: float) -> void: _send_choice())
	seed_ro = Label.new()
	seed_dice = Button.new()
	seed_dice.text = "Random"
	seed_dice.custom_minimum_size.x = 96
	seed_dice.pressed.connect(func() -> void: seed_spin.value = randi() % 100000)
	_grid_row(grid, "Seed", seed_spin, seed_ro, seed_dice)

	delay_spin = SpinBox.new()
	delay_spin.min_value = NetUi.DELAY_MIN
	delay_spin.max_value = NetUi.DELAY_MAX
	delay_spin.step = 1
	delay_spin.suffix = "ticks"
	delay_spin.custom_minimum_size.x = 140
	delay_spin.value_changed.connect(func(_v: float) -> void: _send_choice())
	delay_ro = Label.new()
	delay_suggest = Button.new()
	delay_suggest.text = "Use suggested"
	delay_suggest.pressed.connect(func() -> void: delay_spin.value = int(status.get("suggested_delay", 3)))
	_grid_row(grid, "Input delay", delay_spin, delay_ro, delay_suggest)
	delay_note = _dim("")
	box.add_child(delay_note)

	lobby_msg = Label.new()
	lobby_msg.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	lobby_msg.custom_minimum_size.x = 440
	box.add_child(lobby_msg)

	var bottom := HBoxContainer.new()
	bottom.add_theme_constant_override("separation", 12)
	leave_button = Button.new()
	leave_button.text = "Leave"
	leave_button.custom_minimum_size = Vector2(140, 50)
	leave_button.pressed.connect(on_leave)
	bottom.add_child(leave_button)
	var bfill := Control.new()
	bfill.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	bottom.add_child(bfill)
	ready_button = Button.new()
	ready_button.text = "READY"
	ready_button.theme_type_variation = &"PrimaryButton"
	ready_button.custom_minimum_size = Vector2(220, 50)
	ready_button.pressed.connect(on_ready)
	bottom.add_child(ready_button)
	box.add_child(bottom)

	# Countdown: a big numeral over the whole page.
	countdown_box = ColorRect.new()
	(countdown_box as ColorRect).color = Color(UiPalette.BG, 0.9)
	countdown_box.mouse_filter = Control.MOUSE_FILTER_STOP
	countdown_box.visible = false
	outer.add_child(countdown_box)
	var cbox := VBoxContainer.new()
	cbox.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	cbox.alignment = BoxContainer.ALIGNMENT_CENTER
	countdown_box.add_child(cbox)
	var cap := Label.new()
	cap.text = "MATCH STARTS IN"
	cap.theme_type_variation = &"HeaderLabel"
	cap.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	cbox.add_child(cap)
	countdown = Label.new()
	countdown.theme_type_variation = &"TitleLabel"
	countdown.add_theme_font_size_override("font_size", 160)
	countdown.add_theme_color_override("font_color", UiPalette.ACCENT)
	countdown.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	cbox.add_child(countdown)
	return outer


## One player row: [slot tag] name (you) · faction stripe and name · ping ·
## ready. Returns its parts as a Dictionary.
func _slot_row(parent: Control) -> Dictionary:
	var panel := PanelContainer.new()
	panel.theme_type_variation = &"CardPanel"
	parent.add_child(panel)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 12)
	panel.add_child(row)
	var stripe := ColorRect.new()
	stripe.custom_minimum_size = Vector2(5, 30)
	row.add_child(stripe)
	var tag := Label.new()
	tag.theme_type_variation = &"DimLabel"
	tag.custom_minimum_size.x = 52
	tag.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	row.add_child(tag)
	var nm := Label.new()
	nm.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	nm.clip_text = true
	nm.text_overrun_behavior = TextServer.OVERRUN_TRIM_ELLIPSIS
	nm.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	row.add_child(nm)
	var fac := Label.new()
	fac.custom_minimum_size.x = 90
	fac.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	row.add_child(fac)
	var ping := Label.new()
	ping.theme_type_variation = &"DimLabel"
	ping.custom_minimum_size.x = 76
	ping.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
	ping.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	row.add_child(ping)
	var rdy := Label.new()
	rdy.custom_minimum_size.x = 96
	rdy.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
	rdy.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	row.add_child(rdy)
	return {"panel": panel, "stripe": stripe, "tag": tag, "name": nm, "faction": fac, "ping": ping, "ready": rdy}


func _refresh_lobby() -> void:
	var slot := int(status.get("slot", -1))
	var slots: Array = status.get("slots", [])
	var host := slot == 0
	lobby_where.text = ("Hosting on %s" if host else "Joined %s") % where if where != "" else ""
	var mine: Dictionary = slots[slot] if slot >= 0 and slot < slots.size() else {}
	var theirs: Dictionary = {}
	for i in 2:
		var r: Dictionary = slot_rows[i]
		var s: Dictionary = slots[i] if i < slots.size() else {}
		var present := not String(s.get("name", "")).is_empty()
		if i != slot and present:
			theirs = s
		r.tag.text = "HOST" if i == 0 else "GUEST"
		if not present:
			r.stripe.color = Color(UiPalette.BORDER, 0.6)
			r.name.text = "Open: waiting for a player …"
			r.name.add_theme_color_override("font_color", UiPalette.TEXT_DIM)
			r.faction.text = ""
			r.ping.text = ""
			r.ready.text = ""
			continue
		var f: String = s.get("faction", "")
		r.stripe.color = UiPalette.faction_color(f)
		r.name.text = String(s.name) + ("  (you)" if i == slot else "")
		r.name.add_theme_color_override("font_color", UiPalette.TEXT)
		r.faction.text = UiPalette.FACTION_NAMES.get(f, f)
		r.faction.add_theme_color_override("font_color", UiPalette.faction_color(f).lightened(0.35))
		var ping := float(s.get("ping_ms", -1.0))
		r.ping.text = "" if i == slot and ping <= 0.0 else NetUi.ping_text(ping)
		var rd: bool = s.get("ready", false)
		r.ready.text = "READY" if rd else "not ready"
		r.ready.add_theme_color_override("font_color", UiPalette.OK if rd else UiPalette.TEXT_DIM)

	# Own faction; the opponent's is marked taken.
	var my_f: String = mine.get("faction", "")
	var their_f: String = theirs.get("faction", "")
	for f in faction_btns:
		var b: Button = faction_btns[f]
		b.set_pressed_no_signal(f == my_f)
		b.text = UiPalette.FACTION_NAMES[f] + ("  (taken)" if f == their_f and f != my_f else "")

	# Slot 0 edits the match; slot 1 sees it.
	var map_id: String = status.get("map", "kessel_ridge")
	var seed_v := int(status.get("seed", 1))
	var d := int(status.get("input_delay", 3))
	var sug := int(status.get("suggested_delay", 3))
	for c in [map_pick, seed_spin, seed_dice, delay_spin, delay_suggest]:
		c.visible = host
	for c in [map_ro, seed_ro, delay_ro]:
		c.visible = not host
	_sync(map_pick, "map", _map_ids.find(map_id))
	_sync(seed_spin, "seed", seed_v)
	_sync(delay_spin, "delay", d)
	map_ro.text = Session.MAPS.get(map_id, {}).get("name", map_id)
	seed_ro.text = str(seed_v)
	delay_ro.text = "%d ticks" % d
	var ping_s := ""
	if not theirs.is_empty():
		ping_s = " from a %s ping" % NetUi.ping_text(float(theirs.get("ping_ms", -1.0)))
	delay_note.text = "D %d = %d ms between an order and the units reacting. Suggested: %d%s. Fixed for the match." % [d, NetUi.delay_ms(d), sug, ping_s]
	delay_suggest.disabled = d == sug

	# Ready, and why not.
	var msg := String(status.get("mismatch", ""))
	var err := NetUi.reason_text(String(status.get("error", "")))
	var blocker := ""
	if theirs.is_empty():
		blocker = "Waiting for a second player to join."
	elif my_f == their_f:
		blocker = "Both players picked %s. Factions must differ: pick the other one." % UiPalette.FACTION_NAMES.get(my_f, my_f)
	var i_ready: bool = mine.get("ready", false)
	if not msg.is_empty() or not err.is_empty():
		lobby_msg.text = msg if not msg.is_empty() else err
		lobby_msg.add_theme_color_override("font_color", UiPalette.DANGER)
	elif not blocker.is_empty():
		lobby_msg.text = blocker
		lobby_msg.add_theme_color_override("font_color", UiPalette.WARN if not theirs.is_empty() else UiPalette.TEXT_DIM)
	elif i_ready:
		lobby_msg.text = "Ready. Waiting for %s." % theirs.get("name", "your opponent") if not theirs.get("ready", false) else "Both ready."
		lobby_msg.add_theme_color_override("font_color", UiPalette.TEXT_DIM)
	else:
		lobby_msg.text = "Press Ready when you are set. Changing anything takes both players out of Ready."
		lobby_msg.add_theme_color_override("font_color", UiPalette.TEXT_DIM)
	ready_button.disabled = not blocker.is_empty() or i_ready
	ready_button.text = "READY  ✓" if i_ready else "READY"

	# Countdown.
	var counting: bool = status.get("state", "") == "countdown"
	countdown_box.visible = counting
	if counting:
		countdown.text = str(maxi(ceili(float(status.get("countdown_s", 0.0))), 1))


## Pushes a status value into a widget only when it changed since the last
## push and the player is not editing that widget.
func _sync(c: Control, key: String, v: Variant) -> void:
	if _shown.get(key) == v:
		return
	if c is SpinBox and (c as SpinBox).get_line_edit().has_focus():
		return
	_shown[key] = v
	if c is SpinBox:
		(c as SpinBox).set_value_no_signal(v)
	elif c is OptionButton and int(v) >= 0:
		(c as OptionButton).select(int(v))


func pick_faction(f: String) -> void:
	var net: Variant = Session.net
	if net == null:
		return
	AudioManager.play_ui(&"click")
	net.set_choice({"faction": f})
	refresh()


func _send_choice() -> void:
	var net: Variant = Session.net
	if net == null or int(status.get("slot", -1)) != 0:
		return
	var choice := {
		"seed": int(seed_spin.value),
		"map": _map_ids[maxi(map_pick.selected, 0)],
		"input_delay": int(delay_spin.value),
	}
	_shown.seed = choice.seed
	_shown.delay = choice.input_delay
	_shown.map = map_pick.selected
	net.set_choice(choice)


func on_ready() -> void:
	var net: Variant = Session.net
	if net == null or ready_button.disabled:
		return
	AudioManager.play_ui(&"confirm")
	net.ready()
	refresh()


func on_leave() -> void:
	Session.leave_net()
	_show_page("connect")


func _on_disconnect_back() -> void:
	Session.leave_net()
	_show_page("connect")


# --- widgets ------------------------------------------------------------------------

func _grid_row(grid: GridContainer, label: String, edit: Control, ro: Label, extra: Control) -> void:
	var l := Label.new()
	l.text = label
	l.custom_minimum_size.x = 110
	grid.add_child(l)
	var cell := HBoxContainer.new()
	cell.add_child(edit)
	cell.add_child(ro)
	grid.add_child(cell)
	# In a cell of its own: a grid skips hidden children, so hiding the
	# button itself (slot 1) would shift the next row's cells.
	var extra_cell := HBoxContainer.new()
	extra_cell.add_child(extra)
	grid.add_child(extra_cell)


func _card(parent: Control) -> VBoxContainer:
	var panel := PanelContainer.new()
	panel.theme_type_variation = &"CardPanel"
	panel.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	panel.custom_minimum_size.x = 290
	parent.add_child(panel)
	var v := VBoxContainer.new()
	v.add_theme_constant_override("separation", 10)
	panel.add_child(v)
	return v


func _head(text: String) -> Label:
	var l := Label.new()
	l.text = text
	l.theme_type_variation = &"HeaderLabel"
	return l


func _sub(text: String) -> Label:
	var l := Label.new()
	l.text = text
	l.theme_type_variation = &"HeaderLabel"
	l.add_theme_font_size_override("font_size", 20)
	return l


func _section(text: String) -> Label:
	var l := Label.new()
	l.text = text.to_upper()
	l.theme_type_variation = &"DimLabel"
	l.add_theme_font_size_override("font_size", 13)
	return l


func _label(text: String) -> Label:
	var l := Label.new()
	l.text = text
	l.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	return l


func _dim(text: String) -> Label:
	var l := Label.new()
	l.text = text
	l.theme_type_variation = &"DimLabel"
	l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	l.custom_minimum_size.x = 200
	return l


func _error_label() -> Label:
	var l := Label.new()
	l.add_theme_color_override("font_color", UiPalette.DANGER)
	l.add_theme_font_size_override("font_size", 15)
	l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	l.custom_minimum_size.x = 200
	return l
