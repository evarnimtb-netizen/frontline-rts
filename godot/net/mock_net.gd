class_name MockNet
extends RefCounted
## GDScript stand-in for the bridge's FrontlineNet (S2 presenter work,
## docs/slices/S2-multiplayer.md). It has the same calls the presenter
## makes, so the menu, the lobby and the match HUD can be built and tested
## without a network; integration swaps in the real class (see
## godot/docs/net_api.md for the assumed API).
##
## Lobby API (what the real class must offer):
##   host(port) -> bool          start the in-game relay and join it
##   join("addr:port") -> bool   IPv6 as "[addr]:port"
##   set_name(name)              before host/join (Hello carries the name)
##   set_choice({faction, seed, map, input_delay, team, players, ai})
##                               slot 0 sets all; a joiner only the faction
##                               and his team (the rest ignored). S18:
##                               team -1 none; players 2..4; ai [{slot,
##                               faction, team, difficulty}] (the AI fill)
##   take_slot(slot)             S18: a joiner moves to an open slot
##   ready()                     sends Ready (setup hash)
##   leave()                     Bye; back to "idle"
##   status() -> Dictionary      {state, slot, slots, players, seed, map,
##                               input_delay, suggested_delay, mismatch,
##                               countdown_s, error}; a slot is {name,
##                               faction, ping_ms, ready, team, ai}
## Match side (the real bridge puts these in FrontlineMatch.get_state()):
##   match_state() -> {net: {...}, pending_orders: [{pos, kind}]}
##
## Mock only: `auto` plays a scripted opponent (joins 1.2 s after hosting,
## readies 0.8 s after you, 3 s countdown); `manual_clock` with advance()
## drives time for tests; stage() puts the mock in any state at once.

const COUNTDOWN_S := 3.0
const OPPONENT_JOIN_S := 1.2
const CONNECT_S := 0.8
const OPPONENT_READY_S := 0.8
const DEFAULT_NAME := "Player"
## Mirrors Session.FACTIONS / MAPS (no autoload reference: tests load this
## class before the autoloads exist).
const FACTIONS := ["germany", "usa"]
const MAPS := ["kessel_ridge", "linden_brook", "harrow_cross"]

## Scripted opponent on/off (off: only stage() and explicit calls move it).
var auto := true
## Tests set this and call advance(); otherwise the wall clock runs.
var manual_clock := false
var clock_s := 0.0
## Ports host() treats as taken (bind_failed), for tests.
var busy_ports: Array[int] = []
## Every call, in order, as [name, args] (tests).
var calls: Array = []

var _state := "idle"
var _slot := -1
var _slots: Array = []
var _seed := 1
var _map := "kessel_ridge"
var _delay := 3
var _suggested := 3
var _mismatch := ""
var _error := ""
var _host_port := 0
var _countdown_end := 0.0
var _name := DEFAULT_NAME
## S18: slots in the match.
var _players := 2
## Scheduled script steps: [time_s, Callable].
var _todo: Array = []
var _t0_ms := Time.get_ticks_msec()

## Match side.
var net := {}
var pending: Array = []


func _init() -> void:
	reset_match()


# --- lobby API ---------------------------------------------------------------

func set_name(n: String) -> void:
	calls.append(["set_name", [n]])
	n = n.strip_edges().left(24)
	_name = n if not n.is_empty() else DEFAULT_NAME


func host(port: int) -> bool:
	calls.append(["host", [port]])
	_reset_lobby()
	if port < 1 or port > 65535:
		_fail("bind_failed")
		return false
	if port in busy_ports:
		_fail("bind_failed")
		return false
	_state = "lobby"
	_slot = 0
	_host_port = port
	_slots = [_slot_dict(_name, "germany", 0.0), _empty_slot()]
	if auto:
		_after(OPPONENT_JOIN_S, func() -> void:
			if _state == "lobby" and _slots[1].name == "":
				_slots[1] = _slot_dict("Guest", _other(_slots[0].faction), 38.0)
				_slots[0].ping_ms = 38.0
				_suggest(38.0))
	return true


func join(addr: String) -> bool:
	calls.append(["join", [addr]])
	_reset_lobby()
	var p := NetUi.parse_join(addr)
	if not p.ok:
		_fail("bad_address")
		return false
	_state = "connecting"
	if auto:
		_after(CONNECT_S, func() -> void:
			if _state != "connecting":
				return
			if p.port == 1:
				# Mock convention: port 1 never answers.
				_fail("no_answer")
				return
			_state = "lobby"
			_slot = 1
			_slots = [_slot_dict("Host", "germany", 42.0), _slot_dict(_name, "usa", 42.0)]
			_suggest(42.0))
	return true


func set_choice(choice: Dictionary) -> void:
	calls.append(["set_choice", [choice.duplicate()]])
	if _state != "lobby" or _slot < 0:
		return
	var changed := false
	var f: String = choice.get("faction", _slots[_slot].faction)
	if f in FACTIONS and f != _slots[_slot].faction:
		_slots[_slot].faction = f
		changed = true
	if choice.has("team") and int(choice.team) != int(_slots[_slot].team):
		_slots[_slot].team = clampi(int(choice.team), -1, 3)
		changed = true
	if _slot == 0 and choice.has("players"):
		var n := clampi(int(choice.players), 2, 4)
		# Never below a taken player slot.
		for i in range(_slots.size() - 1, -1, -1):
			if _slots[i].name != "":
				n = maxi(n, i + 1)
				break
		if n != _players:
			_players = n
			changed = true
		while _slots.size() < _players:
			_slots.append(_empty_slot())
		while _slots.size() > _players:
			_slots.pop_back()
	if _slot == 0 and choice.has("ai"):
		for i in _slots.size():
			if int(_slots[i].ai) >= 0:
				_slots[i] = _empty_slot()
		for a in choice.ai:
			var k := int(a.get("slot", 0))
			if k > 0 and k < _slots.size() and _slots[k].name == "" and String(a.get("faction", "")) in FACTIONS:
				_slots[k] = _ai_slot(a.faction, int(a.get("team", -1)), int(a.get("difficulty", 1)))
		changed = true
	if _slot == 0:
		if choice.has("seed") and int(choice.seed) != _seed:
			_seed = int(choice.seed)
			changed = true
		if choice.has("map") and String(choice.map) in MAPS and choice.map != _map:
			_map = choice.map
			changed = true
		if choice.has("input_delay"):
			var d := clampi(int(choice.input_delay), NetUi.DELAY_MIN, NetUi.DELAY_MAX)
			if d != _delay:
				_delay = d
				changed = true
	if changed:
		# Like the relay: any change takes both players out of Ready.
		for s in _slots:
			s.ready = false
		_mismatch = ""


## S18: a joiner moves to an open slot (not slot 0, not an AI's).
func take_slot(slot: int) -> void:
	calls.append(["take_slot", [slot]])
	if _state != "lobby" or _slot <= 0 or slot <= 0 or slot >= _slots.size():
		return
	if _slots[slot].name != "" or int(_slots[slot].ai) >= 0:
		return
	_slots[slot] = _slots[_slot]
	_slots[_slot] = _empty_slot()
	_slot = slot
	for s in _slots:
		if int(s.ai) < 0:
			s.ready = false


func ready() -> void:
	calls.append(["ready", []])
	if _state != "lobby" or _slot < 0:
		return
	if _players > 2:
		_ready_team()
		return
	if _slots[1].name == "":
		_mismatch = "Waiting for a second player."
		return
	if _slots[0].faction == _slots[1].faction:
		_mismatch = "Both players picked %s; factions must differ." % UiPalette.FACTION_NAMES.get(_slots[0].faction, _slots[0].faction)
		return
	_mismatch = ""
	_slots[_slot].ready = true
	if auto:
		var other := 1 - _slot
		_after(OPPONENT_READY_S, func() -> void:
			if _state == "lobby" and _slots[_slot].ready:
				_slots[other].ready = true
				_maybe_start())
	_maybe_start()


## S18: Ready in a lobby of three or four slots: every slot filled, at
## least two teams (no team: alone).
func _ready_team() -> void:
	var teams := {}
	for i in _slots.size():
		var s: Dictionary = _slots[i]
		if s.name == "" and int(s.ai) < 0:
			_mismatch = "Waiting for every slot to be filled."
			return
		teams[int(s.team) if int(s.team) >= 0 else 10 + i] = true
	if teams.size() < 2:
		_mismatch = "the players must be on at least two teams"
		return
	_mismatch = ""
	_slots[_slot].ready = true
	if auto:
		_after(OPPONENT_READY_S, func() -> void:
			if _state != "lobby" or not _slots[_slot].ready:
				return
			for s in _slots:
				s.ready = true
			_maybe_start())
	_maybe_start()


func leave() -> void:
	calls.append(["leave", []])
	_reset_lobby()
	reset_match()


func status() -> Dictionary:
	_run_todo()
	var cd := 0.0
	if _state == "countdown":
		cd = maxf(_countdown_end - now(), 0.0)
		if cd <= 0.0:
			_state = "in_match"
	return {
		"state": _state,
		"slot": _slot,
		"slots": _slots.duplicate(true),
		"players": _players,
		"seed": _seed,
		"map": _map,
		"input_delay": _delay,
		"suggested_delay": _suggested,
		"mismatch": _mismatch,
		"countdown_s": cd,
		"error": _error,
		"error_detail": "",
		"host_port": _host_port,
	}


# --- match side ----------------------------------------------------------------

func reset_match() -> void:
	net = {
		"ping_ms": 38.0,
		"input_delay": _delay,
		"delay_ms": NetUi.delay_ms(_delay),
		"restamps": 0,
		"stalls": 0,
		"state": "ok",
		"paused_by": "",
		"waiting_for": "",
		"waiting_s_left": 0.0,
		"desync_tick": -1,
		"desync_dump": "",
		"disconnect_reason": "",
		"pings": [],
	}
	pending.clear()


## What the real bridge adds to its match state: `net` and
## `pending_orders` (orders sent but not yet executed by the sim).
func match_state() -> Dictionary:
	var t := now()
	pending = pending.filter(func(p: Dictionary) -> bool: return p.exec_at > t)
	var out: Array = []
	for p in pending:
		out.append({"pos": p.pos, "kind": p.kind})
	var n := net.duplicate()
	# Pings are handed out once, like the bridge's.
	net.pings = []
	if n.state == "waiting" and n.has("waiting_until"):
		n.waiting_s_left = maxf(float(n.waiting_until) - t, 0.0)
	n.erase("waiting_until")
	var names: Array = []
	for s in _slots:
		names.append(s.name)
	return {"net": n, "pending_orders": out, "player_names": names}


## Mock of an order going out: it executes after D ticks plus half the
## ping, or never while the match is paused or halted.
func mock_order(kind: StringName, pos: Vector3) -> void:
	var halted: bool = net.state in ["paused", "waiting", "desync"]
	var wait_s := INF if halted else float(net.delay_ms) / 1000.0 + float(net.ping_ms) / 2000.0
	pending.append({"pos": pos, "kind": String(kind), "exec_at": now() + wait_s})


## Mock of an ally in `slot` pinging the map at `pos` (S18 H5).
func mock_ally_ping(slot: int, pos: Vector3) -> void:
	var who := String(_slots[slot].name) if slot >= 0 and slot < _slots.size() else ""
	net.pings.append({"slot": slot, "name": who, "pos": pos})


## Mock of the local player pausing or resuming.
func mock_pause(paused: bool) -> void:
	if paused and net.state == "ok":
		net.state = "paused"
		net.paused_by = _name
	elif not paused and net.state == "paused":
		net.state = "ok"
		net.paused_by = ""
		# Orders held over the pause go out after it.
		for p in pending:
			p.exec_at = now() + float(net.delay_ms) / 1000.0


# --- mock control ----------------------------------------------------------------

func now() -> float:
	return clock_s if manual_clock else (Time.get_ticks_msec() - _t0_ms) / 1000.0


func advance(dt: float) -> void:
	manual_clock = true
	clock_s += dt
	_run_todo()


## Puts the mock in a named state at once (tests and screenshots):
##   lobby states: idle, connecting, lobby_host (alone), lobby (two players,
##   slot 0), lobby_guest (slot 1), lobby_same_faction, lobby_mismatch,
##   lobby_ready (both ready), countdown, disconnected, rejected; S18:
##   lobby_2v2 (host, an AI ally, a guest and an AI on team 2),
##   lobby_2v2_guest (the same, as the guest in slot 2), lobby_2v2_open
##   (host alone in four slots);
##   match states: net_ok, net_unstable, net_waiting, net_reconnecting,
##   net_paused, net_paused_by_them, net_desync, net_disconnect.
func stage(what: String) -> void:
	_todo.clear()
	match what:
		"idle":
			_reset_lobby()
		"connecting":
			_reset_lobby()
			_state = "connecting"
		"lobby_host":
			_reset_lobby()
			_state = "lobby"
			_slot = 0
			_slots = [_slot_dict(_name, "germany", 0.0), _empty_slot()]
		"lobby", "lobby_guest", "lobby_same_faction", "lobby_mismatch", "lobby_ready", "countdown":
			_reset_lobby()
			_state = "lobby"
			_slot = 1 if what == "lobby_guest" else 0
			_seed = 1944
			_delay = 3
			_slots = [_slot_dict("Erik", "germany", 41.0), _slot_dict("Sam", "usa", 41.0)]
			if _slot == 0:
				_slots[0].name = _name
			else:
				_slots[1].name = _name
			_suggest(41.0)
			if what == "lobby_same_faction":
				_slots[1].faction = "germany"
			elif what == "lobby_mismatch":
				_mismatch = NetUi.reason_text("setup_mismatch")
			elif what == "lobby_ready":
				_slots[1].ready = true
			elif what == "countdown":
				_slots[0].ready = true
				_slots[1].ready = true
				_state = "countdown"
				_countdown_end = now() + COUNTDOWN_S - 0.6
		"lobby_2v2", "lobby_2v2_guest", "lobby_2v2_open":
			_reset_lobby()
			_state = "lobby"
			_players = 4
			_map = "harrow_cross"
			_seed = 1944
			_delay = 3
			_slot = 2 if what == "lobby_2v2_guest" else 0
			_slots = [_slot_dict("Erik", "germany", 41.0), _ai_slot("germany", 0, 1), _slot_dict("Sam", "usa", 44.0), _ai_slot("usa", 1, 2)]
			_slots[0].team = 0
			_slots[2].team = 1
			_slots[_slot].name = _name
			if what == "lobby_2v2_open":
				for i in [1, 2, 3]:
					_slots[i] = _empty_slot()
			_suggest(44.0)
		"disconnected":
			_reset_lobby()
			_fail("opponent_left")
		"rejected":
			_reset_lobby()
			_fail("build_mismatch")
		_:
			_stage_match(what)


func _stage_match(what: String) -> void:
	reset_match()
	_state = "in_match"
	_slot = 0
	_slots = [_slot_dict(_name, "germany", 41.0), _slot_dict("Sam", "usa", 41.0)]
	net.ping_ms = 41.0
	net.input_delay = 3
	net.delay_ms = NetUi.delay_ms(3)
	net.restamps = 2
	net.stalls = 1
	match what:
		"net_unstable":
			net.state = "unstable"
			net.ping_ms = 180.0
			net.restamps = 14
			net.stalls = 6
		"net_waiting":
			net.state = "waiting"
			net.waiting_for = "Sam"
			net.waiting_slot = 1
			net.waiting_until = now() + 47.0
			net.waiting_s_left = 47.0
		"net_reconnecting":
			net.state = "waiting"
			net.waiting_for = _name
			net.waiting_slot = 0
			net.waiting_until = now() + 52.0
			net.waiting_s_left = 52.0
		"net_paused":
			net.state = "paused"
			net.paused_by = _name
		"net_paused_by_them":
			net.state = "paused"
			net.paused_by = "Sam"
		"net_desync":
			net.state = "desync"
			net.desync_tick = 1030
			net.desync_dump = OS.get_user_data_dir().path_join("desync/1944-7f3a2c/slot0")
		"net_disconnect":
			net.disconnect_reason = "connection_lost"


func _maybe_start() -> void:
	var all_ready := _slots.size() >= 2
	for s in _slots:
		all_ready = all_ready and s.ready
	if _state == "lobby" and all_ready:
		_state = "countdown"
		_countdown_end = now() + COUNTDOWN_S
		net.input_delay = _delay
		net.delay_ms = NetUi.delay_ms(_delay)


func _suggest(ping_ms: float) -> void:
	_suggested = NetUi.suggested_delay(ping_ms * 1.25, 6.0)
	_delay = _suggested


func _reset_lobby() -> void:
	_state = "idle"
	_slot = -1
	_slots = []
	_players = 2
	_mismatch = ""
	_error = ""
	_host_port = 0
	_todo.clear()


func _fail(code: String) -> void:
	_state = "disconnected"
	_error = code


func _after(s: float, f: Callable) -> void:
	_todo.append([now() + s, f])


func _run_todo() -> void:
	var t := now()
	var due := _todo.filter(func(e: Array) -> bool: return e[0] <= t)
	if due.is_empty():
		return
	_todo = _todo.filter(func(e: Array) -> bool: return e[0] > t)
	for e in due:
		e[1].call()


func _slot_dict(n: String, faction: String, ping: float) -> Dictionary:
	return {"name": n, "faction": faction, "ping_ms": ping, "ready": false, "team": -1, "ai": -1}


func _empty_slot() -> Dictionary:
	return {"name": "", "faction": "", "ping_ms": -1.0, "ready": false, "team": -1, "ai": -1}


## S18: an AI fill slot (difficulty 0 easy, 1 normal, 2 hard).
func _ai_slot(faction: String, team: int, difficulty: int) -> Dictionary:
	return {"name": "", "faction": faction, "ping_ms": -1.0, "ready": true, "team": team, "ai": clampi(difficulty, 0, 2)}


func _other(f: String) -> String:
	return "usa" if f == "germany" else "germany"
