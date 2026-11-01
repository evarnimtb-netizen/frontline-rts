class_name NetUi
extends RefCounted
## S2 presenter helpers shared by the menu, the lobby and the match HUD
## (docs/slices/S2-multiplayer.md "Presenter"): address and port parsing
## for Host / Join, the lobby's input-delay suggestion, reason codes to
## player-facing text, and the net status colours. Pure functions, so the
## headless test can check them without a network.

const DEFAULT_PORT := 47015
## Input delay D in ticks: the lobby override range and the formula's clamp.
const DELAY_MIN := 2
const DELAY_MAX := 6
const DELAY_AUTO_MAX := 4
## One sim tick (30 Hz) in ms.
const TICK_MS := 1000.0 / 30.0
## Hosting below this port needs privileges on some systems.
const HOST_PORT_MIN := 1024

## Reject / Bye / disconnect reason codes (relay-protocol.md) → text. An
## unknown code is shown as given, so a new relay reason still reads.
const REASONS := {
	"build_mismatch": "The two games are different builds. Both players need the same download (the same zip).",
	"content_mismatch": "The two games have different mod content. Both players need the same mods.",
	"protocol": "The other side speaks a different network protocol version. Update both games to the same build.",
	"setup_mismatch": "The two games resolved different match setups. Both players need the same build and mods.",
	"full": "The lobby is full (two players already).",
	"timed_out": "The connection timed out.",
	"no_answer": "Nobody answered at that address. Check the address and port, and that the host's firewall or router lets UDP through.",
	"connection_lost": "The connection to the relay was lost.",
	"host_left": "The host closed the lobby.",
	"opponent_left": "Your opponent left.",
	"left": "You left the match.",
	"flood": "The relay dropped the connection: too many orders at once.",
	"relay_closed": "The relay shut down.",
	"bind_failed": "Could not open that port. Another program (or another Frontline) may be using it.",
	"bad_address": "That address could not be resolved.",
}

const STATE_TEXT := {
	"ok": "Connection good",
	"unstable": "Connection unstable: no word from a player for over 2 s",
	"waiting": "Waiting for a player to reconnect",
	"paused": "Match paused",
	"desync": "Desync: the two games disagree",
}


## Parses the Join field: "1.2.3.4:47015", "[::1]:47015", "host.name:47015",
## or an address without a port (default port). A bare IPv6 address with
## no brackets takes the default port. Returns {ok, host, port, addr,
## error}; `addr` is the normalised "host:port" (IPv6 in brackets) that
## FrontlineNet.join() takes.
static func parse_join(text: String) -> Dictionary:
	var t := text.strip_edges()
	if t.is_empty():
		return _bad("Enter the host's address, e.g. 192.168.1.20:%d." % DEFAULT_PORT)
	if t.length() > 255:
		return _bad("That address is too long.")
	var host := t
	var port_s := ""
	if t.begins_with("["):
		var close := t.find("]")
		if close < 0:
			return _bad("Missing ] after the IPv6 address.")
		host = t.substr(1, close - 1)
		var rest := t.substr(close + 1)
		if not rest.is_empty():
			if not rest.begins_with(":"):
				return _bad("Expected :port after ].")
			port_s = rest.substr(1)
			if port_s.is_empty():
				return _bad("Missing port after the colon.")
		if not _is_ipv6(host):
			return _bad("\"%s\" is not a valid IPv6 address." % host)
	elif t.count(":") > 1:
		# Bare IPv6 (no port possible without brackets).
		if not _is_ipv6(t):
			return _bad("\"%s\" is not a valid IPv6 address. Write IPv6 with a port as [addr]:port." % t)
		host = t
	elif t.count(":") == 1:
		var i := t.find(":")
		host = t.substr(0, i)
		port_s = t.substr(i + 1)
		if port_s.is_empty():
			return _bad("Missing port after the colon.")
	if host.is_empty():
		return _bad("Missing the address before the port.")
	var port := DEFAULT_PORT
	if not port_s.is_empty():
		var p := parse_port(port_s)
		if not p.ok:
			return p
		port = p.port
	var is_v6 := host.contains(":")
	if not is_v6:
		if _looks_ipv4(host):
			if not host.is_valid_ip_address():
				return _bad("\"%s\" is not a valid IPv4 address (four numbers 0–255)." % host)
		elif not _is_hostname(host):
			return _bad("\"%s\" is not an IP address or host name." % host)
	if host in ["0.0.0.0", "::"]:
		return _bad("%s means \"any address\"; enter the host's real address." % host)
	var addr := ("[%s]:%d" if is_v6 else "%s:%d") % [host, port]
	return {"ok": true, "host": host, "port": port, "addr": addr, "error": ""}


## Parses a port field (1–65535, digits only). `hosting` also refuses the
## privileged range below HOST_PORT_MIN.
static func parse_port(text: String, hosting := false) -> Dictionary:
	var t := text.strip_edges()
	if t.is_empty():
		return _bad("Enter a port number (default %d)." % DEFAULT_PORT)
	if not t.is_valid_int() or t.begins_with("+") or t.begins_with("-") or t.length() > 5:
		return _bad("The port must be a number from 1 to 65535.")
	var p := t.to_int()
	if p < 1 or p > 65535:
		return _bad("The port must be a number from 1 to 65535.")
	if hosting and p < HOST_PORT_MIN:
		return _bad("Ports below %d are reserved; use %d–65535 (default %d)." % [HOST_PORT_MIN, HOST_PORT_MIN, DEFAULT_PORT])
	return {"ok": true, "port": p, "error": ""}


## The lobby's suggested D (S2 "Turns"): clamp(ceil((p99 RTT + jitter) /
## tick) + 1, 2, 4); the +1 is a one-tick playout buffer.
static func suggested_delay(p99_rtt_ms: float, jitter_ms: float) -> int:
	return clampi(ceili((p99_rtt_ms + jitter_ms) / TICK_MS) + 1, DELAY_MIN, DELAY_AUTO_MAX)


static func delay_ms(ticks: int) -> int:
	return roundi(ticks * TICK_MS)


## Player-facing text for a reason code ("" for none).
static func reason_text(code: String) -> String:
	if code.is_empty():
		return ""
	return REASONS.get(code, code)


## Status dot colour for a net state.
static func state_color(state: String) -> Color:
	match state:
		"ok":
			return UiPalette.OK
		"unstable", "waiting", "paused":
			return UiPalette.WARN
		"desync":
			return UiPalette.DANGER
	return UiPalette.TEXT_DIM


## "ping 42 ms" style text, or "–" when unknown (negative).
static func ping_text(ms: float) -> String:
	return "–" if ms < 0.0 else "%d ms" % roundi(ms)


static func _bad(msg: String) -> Dictionary:
	return {"ok": false, "host": "", "port": 0, "addr": "", "error": msg}


static func _looks_ipv4(s: String) -> bool:
	for c in s:
		if not (c == "." or (c >= "0" and c <= "9")):
			return false
	return true


static func _is_ipv6(s: String) -> bool:
	# A zone id (fe80::1%en0) is fine for link-local addresses.
	var base := s.get_slice("%", 0)
	return base.contains(":") and base.is_valid_ip_address()


## RFC 1123 host name: dot-separated labels of letters, digits and hyphens,
## no label starting or ending with a hyphen, at least one letter overall
## (so "1.2.3" is not taken as a name).
static func _is_hostname(s: String) -> bool:
	if s.length() > 253 or s.ends_with("."):
		return false
	var has_alpha := false
	for label in s.split("."):
		if label.is_empty() or label.length() > 63 or label.begins_with("-") or label.ends_with("-"):
			return false
		for c in label:
			var lower := c.to_lower()
			if lower >= "a" and lower <= "z":
				has_alpha = true
			elif not ((c >= "0" and c <= "9") or c == "-"):
				return false
	return has_alpha
