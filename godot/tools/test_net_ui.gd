extends SceneTree
## Headless test of the S2 presenter on MockNet (no sockets): address and
## port validation, the D suggestion, the Session net flags, the main
## menu's Multiplayer page (host / join input checks, lobby rules: open
## slot, factions must differ, slot 0 edits and slot 1 sees, D range,
## Ready, mismatch, countdown, disconnect), then the match HUD in net mode
## (readout placement at 1600 and 1280, status dot, pending-order markers,
## desync banner, waiting / reconnecting / paused-by panels, the pause menu
## not pausing, input blocking, disconnect screen) and the lobby → match
## hand-over. S18: the 2v2 lobby (mode, four rows, teams, AI fill, moving
## to an open slot, Ready rules) and ally map pings in the event feed.
##
##   Godot --headless --path godot -s res://tools/test_net_ui.gd
##
## Exits 0 when every check passes.

var _fails := 0
var _checks := 0
var _intents: Array = []


func _initialize() -> void:
	_run.call_deferred()
	create_timer(180.0).timeout.connect(func() -> void:
		printerr("test_net_ui: timed out")
		quit(2))


func _run() -> void:
	var session: Node = root.get_node("Session")
	_test_parse()
	_test_session_flags(session)
	await _test_menu(session)
	await _test_three_factions(session)
	await _test_match(session)
	await _test_handover(session)
	print("test_net_ui: %d/%d checks passed" % [_checks - _fails, _checks])
	quit(0 if _fails == 0 else 1)


# --- pure helpers ------------------------------------------------------------------

func _test_parse() -> void:
	var good := {
		"192.168.1.20:47015": "192.168.1.20:47015",
		" 10.0.0.5 ": "10.0.0.5:47015",
		"127.0.0.1:1": "127.0.0.1:1",
		"[::1]:47016": "[::1]:47016",
		"::1": "[::1]:47015",
		"[fe80::1%en0]:47015": "[fe80::1%en0]:47015",
		"2001:db8::7": "[2001:db8::7]:47015",
		"my-mac.tail1234.ts.net:47015": "my-mac.tail1234.ts.net:47015",
		"localhost": "localhost:47015",
	}
	for t in good:
		var r := NetUi.parse_join(t)
		_check(r.ok and r.addr == good[t], "join accepts %s → %s (%s)" % [t, good[t], r.addr if r.ok else r.error])
	for t in ["", "   ", "192.168.1.300:47015", "1.2.3:47015", ":47015", "1.2.3.4:", "1.2.3.4:0",
			"1.2.3.4:65536", "1.2.3.4:abc", "1.2.3.4:-5", "[::1", "[::1]47015", "[1.2.3.4]:5",
			"0.0.0.0:47015", "[::]:47015", "-host:1", "a b:1", "host_name:1", "::g", "1.2.3.4:47015:9",
			"x".repeat(300)]:
		var r := NetUi.parse_join(t)
		_check(not r.ok and r.error != "", "join rejects \"%s\" (%s)" % [t.left(30), r.error])
	_check(NetUi.parse_port("47015", true).port == 47015, "host port 47015 ok")
	for t in ["80", "0", "65536", "+5", "4.5", "", "1e4", "470150"]:
		_check(not NetUi.parse_port(t, true).ok, "host port \"%s\" rejected" % t)
	_check(NetUi.parse_port("80").ok, "port 80 is fine to join")
	_check(NetUi.suggested_delay(2.0, 1.0) == 2, "LAN ping suggests D = 2")
	_check(NetUi.suggested_delay(60.0, 10.0) == 2, "60 ms + 10 ms jitter suggests D = 2 (D covers the trip to the relay)")
	_check(NetUi.suggested_delay(120.0, 20.0) == 3, "120 ms + 20 ms jitter suggests D = 3")
	_check(NetUi.suggested_delay(400.0, 50.0) == 4, "the suggestion caps at 4")
	_check(NetUi.delay_ms(3) == 100, "D 3 = 100 ms")
	_check(NetUi.reason_text("build_mismatch").contains("same"), "reason codes become text")
	_check(NetUi.reason_text("weird_new_code") == "weird_new_code", "unknown codes show as given")
	_check(NetUi.ping_text(0.0) == "<1 ms" and NetUi.ping_text(-1.0) == "–" and NetUi.ping_text(38.0) == "38 ms", "ping text: under 1 ms, unknown, whole ms")
	_check(NetUi.reason_text("content_missing") != "content_missing" and NetUi.reason_text("bad_name") != "bad_name", "the bridge's extra reason codes have text")
	_check(NetUi.state_color("disconnected") == UiPalette.DANGER, "a lost link shows a red dot")


func _test_session_flags(session: Node) -> void:
	session._parse_autoplay_net("host:47015")
	_check(session.autoplay_net_role == "host" and session.autoplay_net_target == "47015", "--autoplay_net host:PORT")
	session._parse_autoplay_net("join:127.0.0.1:47015")
	_check(session.autoplay_net_role == "join" and session.autoplay_net_target == "127.0.0.1:47015", "--autoplay_net join:IP:PORT")
	session._parse_autoplay_net("join:[::1]:47016")
	_check(session.autoplay_net_target == "[::1]:47016", "--autoplay_net join:[::1]:PORT")
	session.autoplay_net_role = ""
	session.autoplay_net_target = ""
	session._parse_autoplay_net("join:999.1.1.1:5")
	_check(session.autoplay_net_role == "", "a bad --autoplay_net is ignored")
	# Net mode match setup: slot, D, 1x, the desync hook.
	var mock := MockNet.new()
	mock.auto = false
	mock.stage("lobby_guest")
	session.net = mock
	session.net_mode = true
	session.debug_inject_desync = 1000
	var ms: Dictionary = session.match_setup()
	_check(ms.get("net", false) and ms.local_player == 1 and ms.input_delay == int(mock.status().input_delay) and ms.input_delay >= 2 and ms.speed == 1.0 and ms.debug_inject_desync == 1000, "net match_setup carries slot, D, 1x and the desync hook")
	session.net_mode = false
	session.debug_inject_desync = -1
	_check(not session.match_setup().has("net"), "skirmish match_setup has no net keys")
	session.net = null


# --- menu ----------------------------------------------------------------------------

func _test_menu(session: Node) -> void:
	var mock := MockNet.new()
	mock.manual_clock = true
	session.net = mock
	var menu: Control = load("res://ui/menu/main_menu.tscn").instantiate()
	root.add_child(menu)
	await _frames(2)
	menu._show("multiplayer")
	var mp: Node = menu._mp  # untyped: MultiplayerPanel needs the autoloads
	mp.refresh()
	_check(mp.visible and mp.page == "connect", "Multiplayer opens on host / join")
	_check(mp.host_port.text == "47015", "host port defaults to 47015")

	# Host: input validation before any call.
	mp.host_port.text = "80"
	mp.on_host()
	_check(mp.host_error.text != "" and _calls(mock, "host") == 0, "port 80: error shown, host() not called")
	mp.host_port.text = "abc"
	mp.on_host()
	_check(mp.host_error.text != "" and _calls(mock, "host") == 0, "port abc: error shown, host() not called")
	mock.busy_ports = [47020]
	mp.host_port.text = "47020"
	mp.on_host()
	_check(_calls(mock, "host") == 1 and mp.host_error.text.contains("port"), "bind failure reported under Host")
	mp.on_leave()
	# Join: input validation before any call.
	mp.join_addr.text = "300.1.1.1:47015"
	mp.on_join()
	_check(mp.join_error.text != "" and _calls(mock, "join") == 0, "bad IPv4: error shown, join() not called")
	mp.join_addr.text = "[::1"
	mp.on_join()
	_check(mp.join_error.text != "" and _calls(mock, "join") == 0, "bad IPv6: error shown, join() not called")

	# Host a lobby.
	mp.name_edit.text = "Erik"
	mp.host_port.text = "47015"
	mp.on_host()
	mp.refresh()
	_check(_last_call(mock, "host") == [47015] and _last_call(mock, "set_name") == ["Erik"], "host(47015) after set_name(Erik)")
	_check(mp.page == "lobby" and mp.slot_rows[1].name.text.begins_with("Open"), "lobby: own slot, the other open")
	_check(mp.ready_button.disabled, "Ready disabled while alone")
	_check(mp.map_pick.visible and mp.seed_spin.visible and mp.delay_spin.visible and not mp.map_ro.visible, "slot 0 edits map, seed and D")
	mock.advance(1.5)
	mp.refresh()
	_check(mp.slot_rows[1].name.text == "Guest" and mp.slot_rows[1].ping.text == "38 ms", "opponent shows with name and ping")
	_check(mp.slot_rows[0].name.text.ends_with("(you)") and mp.slot_rows[0].faction.text == "Germany" and mp.slot_rows[1].faction.text == "USA", "slots show factions")
	_check(not mp.ready_button.disabled, "Ready enabled with two players, different factions")
	_check(mp.delay_note.text.contains("Suggested: %d" % int(mp.status.suggested_delay)), "the D suggestion is shown")
	# Slot 0 edits go to set_choice; D is held to 2–6.
	mp.seed_spin.value = 777
	_check(_last_call(mock, "set_choice")[0].get("seed") == 777, "seed edit sent with set_choice")
	mp.delay_spin.value = 9
	_check(int(mp.delay_spin.value) == 6 and _last_call(mock, "set_choice")[0].get("input_delay") == 6, "D held to at most 6")
	mp.delay_spin.value = 1
	_check(int(mp.delay_spin.value) == 2, "D held to at least 2")
	mp.delay_suggest.pressed.emit()
	_check(int(mp.delay_spin.value) == int(mp.status.suggested_delay), "Use suggested sets D")
	# Factions must differ.
	mp.pick_faction("usa")
	_check(mp.ready_button.disabled and mp.lobby_msg.text.contains("must differ"), "same faction: Ready disabled, reason shown")
	_check(mp.faction_btns.germany.text == "Germany", "no faction marked taken by both")
	mp.pick_faction("germany")
	_check(not mp.ready_button.disabled and mp.faction_btns.usa.text.contains("taken"), "back to different factions: the other one shows as taken")
	# Ready → opponent ready → countdown → in_match.
	mp.on_ready()
	_check(_calls(mock, "ready") == 1 and mp.ready_button.disabled and mp.slot_rows[0].ready.text == "READY", "Ready sent; own row shows READY")
	mock.advance(1.0)
	mp.refresh()
	_check(mp.countdown_box.visible and mp.countdown.text == "3", "both ready: 3-2-1 countdown shows 3")
	mock.advance(1.2)
	mp.refresh()
	_check(mp.countdown.text == "2", "countdown ticks to 2")
	mock.leave()
	mp.refresh()
	_check(mp.page == "connect", "leave → back to host / join")

	# Mismatch and reject reasons.
	mock.stage("lobby_mismatch")
	mp.refresh()
	_check(mp.lobby_msg.text == NetUi.reason_text("setup_mismatch"), "mismatch reason shown in the lobby")
	mock.stage("rejected")
	mp.refresh()
	_check(mp.page == "disconnected" and mp.disconnect_panel.reason.text == NetUi.reason_text("build_mismatch"), "reject reason on the disconnect page")
	mp.disconnect_panel.back_button.pressed.emit()
	_check(mp.page == "connect" and _calls(mock, "leave") >= 2, "Back leaves and returns to host / join")

	# Join as slot 1: read-only match settings.
	mp.join_addr.text = "192.168.1.20"
	mp.on_join()
	mp.refresh()
	_check(_last_call(mock, "join") == ["192.168.1.20:47015"] and mp.page == "connecting", "join(normalised address) → connecting")
	_check(mp.connecting_label.text.contains("192.168.1.20:47015"), "connecting page names the address")
	mock.advance(1.0)
	mp.refresh()
	_check(mp.page == "lobby" and int(mp.status.slot) == 1, "joined as slot 1")
	_check(not mp.map_pick.visible and mp.map_ro.visible and mp.seed_ro.visible and mp.delay_ro.visible, "slot 1 sees the match settings read-only")
	var before := _calls(mock, "set_choice")
	mp._send_choice()
	_check(_calls(mock, "set_choice") == before, "slot 1 does not send seed / map / D")
	mock.stage("disconnected")
	mp.refresh()
	_check(mp.page == "disconnected" and mp.disconnect_panel.reason.text == NetUi.reason_text("opponent_left"), "lobby disconnect shows the reason")

	# S18: the 2v2 lobby. The host alone in four slots fills them with AI.
	mock.stage("lobby_host")
	mp.refresh()
	_check(not mp.team_box.visible and not mp.slot_rows[2].panel.visible, "1v1 lobby: two rows, no team pick")
	mp.pick_mode("2v2")
	mp.refresh()
	_check(_last_call(mock, "set_choice")[0].get("players") == 4 and String(mp.status.map) == "harrow_cross", "2v2: four slots on Harrow Cross")
	_check(mp.slot_rows[3].panel.visible and mp.team_box.visible and mp.slot_rows[0].team.text == "Team 1", "2v2: four rows, team pick, the host on Team 1")
	_check(mp.slot_rows[1].action.visible and mp.slot_rows[1].action.text == "Add AI", "the host can fill an open slot with AI")
	_check(mp.ready_button.disabled and mp.lobby_msg.text.contains("open slot"), "open slots: Ready disabled, reason shown")
	for i in [1, 2, 3]:
		mp._on_slot_action(i)
		mp.refresh()
	var ai: Array = _last_call(mock, "set_choice")[0].get("ai", [])
	_check(ai.size() == 3 and int(ai[0].team) == 0 and int(ai[1].team) == 1 and int(ai[2].team) == 1, "AI fill: slot 1 on the host's team, slots 2 and 3 on the other")
	_check(mp.slot_rows[1].name.text.begins_with("AI") and mp.slot_rows[1].ready.text == "AI" and mp.slot_rows[3].team.text == "Team 2", "AI rows show AI, difficulty and team")
	_check(not mp.ready_button.disabled, "every slot filled, two teams: Ready enabled")
	mp._on_slot_action(2)
	mp.refresh()
	_check(mp.slot_rows[2].action.text == "Add AI" and mp.slot_rows[3].action.text == "Remove AI", "Remove AI opens the slot again")
	mp.pick_mode("1v1")
	mp.refresh()
	_check(not mp.slot_rows[2].panel.visible and _last_call(mock, "set_choice")[0].get("ai") == [], "back to 1v1 drops the AI fill")
	# As the guest in slot 2 of a full 2v2.
	mock.stage("lobby_2v2_guest")
	mp.refresh()
	_check(int(mp.status.slot) == 2 and not mp.mode_pick.visible and mp.mode_ro.text == "2v2", "guest sees the 2v2 read-only")
	_check(not mp.slot_rows[1].action.visible and not mp.slot_rows[3].action.visible, "a guest cannot touch AI slots")
	mp.pick_team(0)
	mp.refresh()
	_check(_last_call(mock, "set_choice")[0].get("team") == 0 and mp.slot_rows[2].team.text == "Team 1", "team pick sent and shown")
	mock._slots[3].team = 0
	mp.refresh()
	_check(mp.ready_button.disabled and mp.lobby_msg.text.contains("one team"), "everyone on one team: Ready disabled")
	mock._slots[3].team = 1
	mp.pick_team(1)
	mp.refresh()
	_check(not mp.ready_button.disabled, "two teams: Ready enabled")
	mp.on_ready()
	mock.advance(1.0)
	mp.refresh()
	_check(mp.countdown_box.visible, "2v2 all ready: countdown")
	# A joiner moves to an open slot.
	mock.stage("lobby_2v2_open")
	mock._slot = 2
	mock._slots[2] = mock._slot_dict(mock._name, "usa", 40.0)
	mock._slots[0].name = "Host"
	mp.refresh()
	_check(mp.slot_rows[1].action.visible and mp.slot_rows[1].action.text == "Move here", "a joiner can move to an open slot")
	mp._on_slot_action(1)
	mp.refresh()
	_check(_last_call(mock, "take_slot") == [1] and int(mp.status.slot) == 1, "Move here → take_slot(1)")
	mock.leave()
	menu.queue_free()
	session.net = null
	await _frames(2)


# --- S15: three factions ------------------------------------------------------------

## The menu options the rules give with the USSR on (Germany-USA, Germany-USSR).
func _three_options() -> Dictionary:
	return {
		"factions": [{"id": "germany", "name": "Germany", "color": ""}, {"id": "usa", "name": "USA", "color": ""}, {"id": "ussr", "name": "USSR", "color": "#9a5a4a"}],
		"opponents": {"germany": ["usa", "ussr"], "usa": ["germany"], "ussr": ["germany"]},
		"mirror": false,
	}


func _test_three_factions(session: Node) -> void:
	# Control: the shipped options are today's menu (two factions, no picker).
	session.set_options(session.default_options())
	var menu: Control = load("res://ui/menu/main_menu.tscn").instantiate()
	root.add_child(menu)
	await _frames(2)
	_check(menu._faction_btns.size() == 2 and not menu._faction_btns.has("ussr"), "USSR off: the menu lists two factions")
	_check(not menu._opp_row.visible and not menu._opp_section.visible, "USSR off: the opponent picker is hidden")
	menu.queue_free()
	await _frames(2)
	# USSR on.
	session.set_options(_three_options())
	_check(session.faction_ids() == ["germany", "usa", "ussr"], "options: three factions in order")
	_check(UiPalette.faction_name("ussr") == "USSR" and UiPalette.FACTION_SHORT.ussr == "SOV" and UiPalette.faction_color("ussr").to_html(false) == "9a5a4a", "USSR: name, short name SOV, colour #9a5a4a")
	menu = load("res://ui/menu/main_menu.tscn").instantiate()
	root.add_child(menu)
	await _frames(2)
	_check(menu._faction_btns.size() == 3 and menu._faction_btns.ussr.text == "USSR", "the menu lists Germany, USA and USSR")
	_check(menu._faction == "germany" and menu._opp_row.visible and menu._opp_btns.size() == 2, "Germany: an opponent picker with USA and USSR")
	_check(menu._opponent == "usa" and menu._opp_btns.usa.button_pressed, "the default opponent (USA) is pressed")
	menu._opp_btns.ussr.pressed.emit()
	_check(menu._opponent == "ussr", "the picker chooses the USSR")
	menu._faction_btns.ussr.pressed.emit()
	_check(menu._faction == "ussr" and not menu._opp_row.visible and menu._opponent == "germany", "USSR: one opponent (Germany), the picker is hidden")
	menu._faction_btns.usa.pressed.emit()
	_check(menu._opponent == "germany" and not menu._opp_row.visible, "USA: Germany only")
	menu.queue_free()
	await _frames(2)
	# Lobby: the buttons follow the options; USA against USSR is refused.
	var mock := MockNet.new()
	mock.manual_clock = true
	session.net = mock
	menu = load("res://ui/menu/main_menu.tscn").instantiate()
	root.add_child(menu)
	await _frames(2)
	menu._show("multiplayer")
	var mp: Node = menu._mp
	mp.name_edit.text = "Erik"
	mp.host_port.text = "47015"
	mp.on_host()
	mock.advance(1.5)
	mp.refresh()
	_check(mp.faction_btns.size() == 3 and mp.faction_btns.ussr.text == "USSR", "lobby: three faction buttons")
	mp.pick_faction("ussr")
	mp.refresh()
	_check(mp.slot_rows[0].faction.text == "USSR" and mp.slot_rows[1].faction.text == "USA", "lobby: the USSR against the guest's USA")
	_check(mp.ready_button.disabled and mp.lobby_msg.text.contains("cannot fight"), "lobby: USSR against USA is refused with its reason")
	mock.ready()
	_check(String(mock.status().mismatch).contains("cannot fight"), "the mock net refuses the pair at Ready, as the bridge does")
	mp.pick_faction("germany")
	mp.refresh()
	_check(not mp.ready_button.disabled and mp.lobby_msg.text.contains("Press Ready"), "Germany against USA: Ready")
	_check(session.pair_refusal("germany", "ussr") == "" and session.pair_refusal("usa", "ussr").contains("cannot fight") and session.pair_refusal("usa", "usa").contains("must differ"), "pair_refusal: allowed, refused, same faction")
	mock.leave()
	menu.queue_free()
	session.net = null
	session.set_options(session.default_options())
	await _frames(2)


# --- match ---------------------------------------------------------------------------

func _test_match(session: Node) -> void:
	var mock := MockNet.new()
	mock.auto = false
	mock.manual_clock = true
	mock.set_name("Erik")
	mock.stage("net_ok")
	session.net = mock
	session.net_mode = true
	var m: Node = load("res://match/match_mock.tscn").instantiate()
	root.add_child(m)
	await _frames(4)
	var view: Node = m.view
	var src: Node = m.source
	src.time_scale = 0.0
	view.intent.connect(func(k: StringName, d: Dictionary) -> void: _intents.append([k, d]))
	_check(view.net_mode and view.net_readout.visible, "net state → readout visible")
	_check(view.player(1).name == "Sam", "players carry the lobby names")

	# S18 H5: an ally's map ping shows in the feed (Space jumps there);
	# own pings go out as the map_ping intent.
	mock.mock_ally_ping(1, Vector3(10, 0, -20))
	await _frames(3)
	var fl: Array = view.event_feed.lines
	_check(not fl.is_empty() and String(fl[0].text) == "Sam pinged the map" and view.event_feed.last_position() == Vector3(10, 0, -20), "an ally's ping shows in the feed")
	_intents.clear()
	view.map_ping(Vector3(5, 0, 5))
	_check(_intents.size() == 1 and _intents[0][0] == &"map_ping" and _intents[0][1].target == Vector3(5, 0, 5), "own ping → map_ping intent")
	_check(String(view.event_feed.lines[0].text) == "You pinged the map", "own ping shows in the feed")

	# Readout placement at both HUD sizes.
	var hud: Control = view.hud
	for sz in [Vector2(1600, 900), Vector2(1280, 800)]:
		hud.force_size = sz
		await _frames(2)
		var ro: Control = view.net_readout
		var span: Vector2 = view.top_bar.free_span()
		var lv: int = ro.pick_level()
		var w: float = ro.width_of(lv)
		_check(ro.size.y == hud.rect_of(view.top_bar).size.y and ro.position.y == 0.0, "%dx%d: readout in the top bar" % [sz.x, sz.y])
		_check(w <= ro.size.x + 0.5 and ro.position.x >= span.x - 0.5 and ro.position.x + ro.size.x <= span.y + 0.5, "%dx%d: readout fits between tickets and clock (wording %d, %.0f of %.0f px)" % [sz.x, sz.y, lv, w, ro.size.x])
		var tb: Control = view.top_bar
		var last_chip: Rect2 = tb.chip_rect(4)
		_check(ro.position.x > last_chip.end.x + float(hud.m.ticket_w), "%dx%d: readout right of the enemy ticket bar" % [sz.x, sz.y])
		_check(ro.position.x + ro.size.x < tb.size.x - 112.0 - 40.0, "%dx%d: readout clear of the clock" % [sz.x, sz.y])
		if sz.x == 1600:
			_check(lv <= 1, "1600: the readout uses words")
	hud.force_size = Vector2.ZERO
	hud.relayout()
	await _frames(1)
	_check(NetUi.state_color("ok") == UiPalette.OK and NetUi.state_color("desync") == UiPalette.DANGER and NetUi.state_color("unstable") == UiPalette.WARN, "status dot colours")
	_check(view.net_readout._get_tooltip(Vector2.ZERO).contains("Input delay D = 3 ticks = 100 ms"), "readout tooltip spells out D")

	# Pending orders: marker at click time, held until executed.
	var cam: Camera3D = m.camera
	var ids: PackedInt32Array = src.squad_ids(0, "rifle")
	cam.jump_to(src.squad_pos(ids[0]))
	await _frames(2)
	view.selection.select_squads(ids.slice(0, 1))
	var g: Vector3 = src.squad_pos(ids[0]) + Vector3(20, 0, -20)
	g.y = src.height_at(g.x, g.z)
	view.selection.order_to_ground(g)
	await _frames(1)
	_check(view.pending_draw.size() == 1 and view.pending_draw[0].kind == "move", "move order: one pending marker")
	_check(mock.pending.size() == 1, "the source holds the order as pending")
	await _frames(2)
	_check(view.pending_draw.size() == 1, "marker stays while the order is in flight")
	mock.advance(0.2)
	await _frames(2)
	_check(view.pending_draw.is_empty(), "marker gone once the order executes")
	# A click marker the source never confirms lasts LOCAL_PENDING_S.
	view._note_pending(&"attack_move_units", {"units": PackedInt32Array([1]), "target": g})
	view._update_pending({"net": {}})
	_check(view.pending_draw.size() == 1 and view.pending_draw[0].kind == "attack_move", "click-time marker before the state has it")
	await create_timer(0.7).timeout
	await _frames(1)
	_check(view.pending_draw.is_empty(), "an unconfirmed click marker expires")
	view.emit_intent(&"stop", {"squads": ids.slice(0, 1)})
	await _frames(1)
	_check(view.pending_draw.is_empty(), "stop (no target) gets no marker")

	# Pause menu in net mode: opening does not pause.
	_intents.clear()
	view.pause_menu.open()
	_check(_last(&"pause") == null and view.pause_menu.net_pause_button.visible, "net: the menu opens without pausing, offers Pause match")
	_check(not view.pause_menu.reload_button.visible, "net: no debug data reload")
	view.pause_menu.net_pause_button.pressed.emit()
	await _frames(2)
	_check(_last(&"pause") != null and _last(&"pause").paused and not view.pause_menu.visible, "Pause match sends pause and closes the menu")
	_check(view.net_overlay.mode == "paused" and view.net_overlay.lines[0] == "PAUSED BY YOU", "own pause: PAUSED BY YOU")
	_check(view.input_blocked(), "orders blocked while paused")
	view.net_overlay.resume_button.pressed.emit()
	await _frames(2)
	_check(view.net_overlay.mode == "" and not view.input_blocked(), "Resume on the panel resumes")

	# Paused by the other player.
	mock.stage("net_paused_by_them")
	await _frames(2)
	_check(view.net_overlay.mode == "paused" and view.net_overlay.lines[0] == "PAUSED BY SAM" and view.net_overlay.resume_button.visible, "paused by Sam, with Resume")

	# Waiting for a player, with the timer.
	mock.stage("net_waiting")
	await _frames(2)
	var ov: Control = view.net_overlay
	_check(ov.mode == "waiting" and ov.lines[0] == "WAITING FOR SAM (47 s)", "waiting overlay names the player and the time (%s)" % ov.lines[0])
	mock.advance(5.0)
	await _frames(2)
	_check(ov.lines[0] == "WAITING FOR SAM (42 s)", "the timer counts down")
	_check(view.net_readout.visible and NetUi.state_color(view.net.state) == UiPalette.WARN, "waiting: amber dot")
	mock.stage("net_reconnecting")
	await _frames(2)
	_check(ov.lines[0].begins_with("RECONNECTING"), "own drop reads Reconnecting")

	# Desync banner.
	mock.stage("net_desync")
	await _frames(2)
	_check(ov.mode == "desync" and ov.lines[0] == "DESYNC at tick 1030" and ov.lines[2].begins_with("Dump: ") and ov.lines[2].ends_with("slot0"), "desync banner: tick and dump path")
	_check(ov.copy_button.visible and ov.leave_button.visible, "banner offers Copy path and Leave")
	for sz in [Vector2(1600, 900), Vector2(1280, 800)]:
		hud.force_size = sz
		await _frames(2)
		var br: Rect2 = ov.banner_rect
		var mmr: Rect2 = hud.rect_of(view.minimap)
		var fe: Rect2 = hud.rect_of(view.event_feed)
		_check(not br.intersects(mmr) and not br.intersects(fe) and br.position.y >= hud.m.tb, "%dx%d: banner clear of the top bar, minimap and feed" % [sz.x, sz.y])
		_check(Rect2(Vector2.ZERO, sz).encloses(br) and br.encloses(Rect2(ov.leave_button.position, ov.leave_button.size)), "%dx%d: banner on screen, buttons inside it" % [sz.x, sz.y])
		var font: Font = ov.get_theme_default_font()
		_check(HudStyle.text_w(font, ov.lines[2], HudStyle.FONT) <= br.size.x - 14.0 - 12.0 - 104.0 - 14.0 + 0.5, "%dx%d: dump path fits left of the buttons" % [sz.x, sz.y])
	hud.force_size = Vector2.ZERO
	hud.relayout()
	ov.copy_button.pressed.emit()
	_check(ov.copy_button.text == "Copied", "Copy path copies")
	_check(view.input_blocked(), "orders blocked on desync")
	ov.leave_button.pressed.emit()
	_check(view.pause_menu.visible and view.pause_menu._confirm.visible and view.pause_menu._confirm_ok.text == "LEAVE MATCH", "Leave asks first")
	view.pause_menu.close()

	# Unstable: the match runs on, the dot turns amber.
	mock.stage("net_unstable")
	await _frames(2)
	_check(ov.mode == "" and not view.input_blocked() and NetUi.state_color(view.net.state) == UiPalette.WARN, "unstable: no overlay, orders allowed, amber dot")

	# Disconnect screen.
	mock.stage("net_disconnect")
	await _frames(2)
	_check(view.net_disconnect.visible and view.net_disconnect.panel.reason.text == NetUi.reason_text("connection_lost"), "disconnect screen with the reason")
	_check(view.input_blocked(), "disconnect blocks input")

	# Without `net` the presenter is the skirmish one.
	session.net_mode = false
	await _frames(2)
	_check(not view.net_mode and not view.net_readout.visible and view.net_overlay.mode == "" and not view.net_disconnect.visible, "no net key: readout and panels hidden")
	_intents.clear()
	view.pause_menu.open()
	_check(_last(&"pause") != null and _last(&"pause").paused and not view.pause_menu.net_pause_button.visible, "skirmish: the menu still pauses")
	view.pause_menu.close()

	# Back to menu from the disconnect screen leaves the lobby.
	session.net_mode = true
	await _frames(2)
	view.net_disconnect.panel.back_button.pressed.emit()
	await _frames(3)
	_check(_calls(mock, "leave") >= 1 and not session.net_mode, "Back to menu leaves the net session")
	await _frames(3)
	for c in root.get_children():
		if not c.name in ["AudioManager", "Settings", "Session"]:
			c.queue_free()
	await _frames(2)


## Lobby → in_match → the match scene in net mode (mock scene for the mock).
func _test_handover(session: Node) -> void:
	var mock := MockNet.new()
	mock.manual_clock = true
	mock.set_name("Erik")
	session.net = mock
	mock.host(47015)
	mock.advance(1.5)
	mock.ready()
	mock.advance(1.0)
	mock.advance(3.1)
	_check(mock.status().state == "in_match", "mock: both ready → countdown → in_match")
	var menu: Control = load("res://ui/menu/main_menu.tscn").instantiate()
	root.add_child(menu)
	current_scene = menu
	await _frames(2)
	menu._show("multiplayer")
	await _frames(8)
	_check(session.net_mode, "in_match hands over to the match in net mode")
	var cs := current_scene
	_check(cs != null and cs.scene_file_path == "res://match/match_mock.tscn", "the mock net plays on match_mock.tscn")
	await _frames(4)
	if cs != null and cs.has_method("_mock_net"):
		_check(cs.view.net_mode, "the match view is in net mode")
	session.leave_net()
	session.net = null


# --- helpers -----------------------------------------------------------------------------

func _frames(n: int) -> void:
	for i in n:
		await process_frame


func _check(ok: bool, what: String) -> void:
	_checks += 1
	if not ok:
		_fails += 1
		printerr("FAIL: " + what)
	else:
		print("ok: " + what)


func _calls(mock: MockNet, name: String) -> int:
	return mock.calls.filter(func(c: Array) -> bool: return c[0] == name).size()


func _last_call(mock: MockNet, name: String) -> Variant:
	for i in range(mock.calls.size() - 1, -1, -1):
		if mock.calls[i][0] == name:
			return mock.calls[i][1]
	return null


func _last(kind: StringName) -> Variant:
	for i in range(_intents.size() - 1, -1, -1):
		if _intents[i][0] == kind:
			return _intents[i][1]
	return null
