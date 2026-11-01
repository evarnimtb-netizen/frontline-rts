extends RefCounted
## The S2 part of `--autoplay` in a net match (`--autoplay_net`, S2
## "Done when" 4). autoplay.gd still plays the side; this adds the net
## screenshots and the hash record, saved in Session.autoplay_dir() as
## s2_<role>_<name>.png (role = host or join):
##
##   s2_<role>_readout.png     the match at 0:25 with the net readout, the
##                             camera on our squads
##   s2_<role>_pending.png     at 0:40, a move order for one of our MG or
##                             rifle squads 22 m to its side, shot on the
##                             first frame the bridge lists it in
##                             pending_orders (sent, not run yet). At D = 2
##                             that is one or two frames.
##   s2_<role>_waiting.png     "Waiting for <name>" or "Reconnecting"
##                             (only when the other side goes silent)
##   s2_<role>_desync.png      the desync banner once the dump is written
##                             (only on a desync)
##   s2_<role>_disconnect.png  the disconnect screen (only when the link
##                             ends before the match does)
##
## and s2_hashes_<role>.json, every hash this client published
## ({slot, final_hash, hashes: [{tick, hash}]}, the format
## tools/compare_hashes.py reads), written when the run ends.
##
## Exit codes (Session.quit_game): 0 the match ended normally, 6 desync,
## 7 disconnected; autoplay.gd's 3 (a shot missing) still applies.

const READOUT_AT_S := 25.0
## Wall seconds the waiting overlay is up before its shot (the bar drains).
const WAITING_SHOT_AFTER_S := 4.0
## Wall seconds to wait for the desync dump before shooting the banner anyway.
const DUMP_WAIT_S := 8.0
const PENDING_AT_S := 40.0
## Game seconds to wait for the order to show in pending_orders before
## trying again (at most PENDING_TRIES times).
const PENDING_WAIT_S := 2.0
const PENDING_TRIES := 3

var ap: Node
var view: MatchView
var source: Node
var camera: Camera3D
var role := "host"
var _waiting_since := -1.0
var _desync_since := -1.0
var _disc_since := -1.0
var _ending := false
## The pending-shot order's target (INF = none in flight) and when it went.
var _pend_target := Vector3.INF
var _pend_at := 0.0
var _pend_tries := 0


func _init(autoplay: Node) -> void:
	ap = autoplay
	view = ap.view
	source = ap.source
	camera = ap.camera
	role = Session.autoplay_net_role if Session.autoplay_net_role != "" else "net"
	# The S1b HUD shots resize the window to 1600 and 1280; two instances
	# side by side keep their size, so those two are skipped here.
	ap.shots["s1b_hud_1600"] = true
	ap.shots["s1b_hud_1280"] = true


func shot_name(what: String) -> String:
	return "s2_%s_%s.png" % [role, what]


## Required shots for a normal run (the conditional ones are not).
func missing() -> Array:
	var out := []
	for k in ["readout", "pending"]:
		if not ap.shots.has("net_" + k):
			out.append(shot_name(k))
	return out


## Called by autoplay.gd every state before anything else. Returns true when
## the net side took over this frame (a halt being recorded or the run
## ending), so autoplay.gd does not play.
func step(s: Dictionary, t: float) -> bool:
	if _ending:
		return true
	var net: Dictionary = s.get("net", {})
	var st := String(net.get("state", "ok"))
	if st == "desync":
		if _desync_since < 0.0:
			_desync_since = ap.wall
			print("autoplay_net: desync at tick %d" % int(net.get("desync_tick", -1)))
		if String(net.get("desync_dump", "")) != "" or String(net.get("desync_dump_error", "")) != "" \
				or ap.wall - _desync_since > DUMP_WAIT_S:
			print("autoplay_net: desync dump %s%s" % [net.get("desync_dump", ""), net.get("desync_dump_error", "")])
			_end_with("desync", 6, s)
		return true
	if String(net.get("disconnect_reason", "")) != "" and String(s.get("result", {}).get("status", "running")) == "running":
		if _disc_since < 0.0:
			_disc_since = ap.wall
			print("autoplay_net: disconnected: %s" % net.disconnect_reason)
		if ap.wall - _disc_since > 1.0:
			_end_with("disconnect", 7, s)
		return true
	if st == "waiting":
		if _waiting_since < 0.0:
			_waiting_since = ap.wall
			print("autoplay_net: waiting for %s (%.0f s left)" % [net.get("waiting_for", "?"), float(net.get("waiting_s_left", 0.0))])
		if not ap.shots.has("net_waiting") and ap.wall - _waiting_since > WAITING_SHOT_AFTER_S:
			_shoot("waiting")
		return true
	if _waiting_since >= 0.0:
		print("autoplay_net: back from waiting after %.1f s (state %s)" % [ap.wall - _waiting_since, st])
		_waiting_since = -1.0
	if ap._busy:
		return true
	if not ap.shots.has("net_readout") and t >= READOUT_AT_S:
		# Our men in view, so the shot is more than the top bar.
		var own: Array = ap._own(s)
		if not own.is_empty():
			var c := Vector3.ZERO
			for sq in own:
				c += sq.pos / float(own.size())
			camera.height = 70.0
			camera.pitch_deg = -50.0
			camera.jump_to(c)
		_shoot("readout")
		return true
	if not ap.shots.has("net_pending") and _pend_tries < PENDING_TRIES:
		return _pending_step(s, t)
	return false


## The pending-order shot: send a move like a click would (MatchView
## emit_intent, so the presenter's click-time marker starts too), then shoot
## once the bridge's pending_orders has it.
func _pending_step(s: Dictionary, t: float) -> bool:
	if _pend_target == Vector3.INF:
		if t < PENDING_AT_S + _pend_tries * 20.0:
			return false
		for sq in ap._own(s):
			if not sq.type in ["mg", "rifle"] or ap.s1b.reserved.has(int(sq.id)) or not sq.order in ["idle", "setup", "hold"]:
				continue
			var to_c: Vector3 = (Vector3.ZERO - sq.pos).normalized()
			var side := Vector3(-to_c.z, 0, to_c.x)
			_pend_target = ap._ground(sq.pos + side * 22.0)
			_pend_at = t
			_pend_tries += 1
			camera.height = 34.0
			camera.pitch_deg = -58.0
			camera.jump_to(sq.pos.lerp(_pend_target, 0.6))
			view.emit_intent(&"move", {"squads": PackedInt32Array([sq.id]), "target": _pend_target})
			print("autoplay_net: pending-shot move for %s#%d to %s" % [sq.type, sq.id, _pend_target])
			return true
		return false
	for p in s.get("pending_orders", []):
		var pos: Vector3 = p.pos
		if Vector2(pos.x, pos.z).distance_to(Vector2(_pend_target.x, _pend_target.z)) < 1.0:
			print("autoplay_net: pending marker at %s, %.0f ms after the order (D %d)" % [pos, (t - _pend_at) * 1000.0, int(s.get("net", {}).get("input_delay", 0))])
			_pend_target = Vector3.INF
			_shoot("pending", 0.0)
			return true
	if t - _pend_at > PENDING_WAIT_S:
		print("autoplay_net: the pending-shot order never showed in pending_orders (try %d)" % _pend_tries)
		_pend_target = Vector3.INF
	return false


## Before autoplay.gd quits: the hash record.
func finish(s: Dictionary) -> void:
	write_hashes(s)


func write_hashes(s: Dictionary) -> void:
	if not source.has_method("get_net_hashes"):
		return
	var list: Array = source.get_net_hashes()
	var hashes := []
	for pair in list:
		hashes.append({"tick": int(pair[0]), "hash": String(pair[1])})
	var out := {
		"slot": int(s.get("local_player", 0)),
		"role": role,
		"final_hash": hashes[hashes.size() - 1].hash if not hashes.is_empty() else "",
		"final_tick": hashes[hashes.size() - 1].tick if not hashes.is_empty() else -1,
		"result": s.get("result", {}),
		"net": s.get("net", {}),
		"hashes": hashes,
	}
	var path: String = ap.dir.path_join("s2_hashes_%s.json" % role)
	DirAccess.make_dir_recursive_absolute(path.get_base_dir())
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		push_error("autoplay_net: cannot write %s" % path)
		return
	f.store_string(JSON.stringify(out, "  "))
	f.close()
	print("autoplay_net: %d hashes (last tick %d) in %s" % [hashes.size(), out.final_tick, path])


func _shoot(what: String, delay := 0.6) -> void:
	ap._busy = true
	ap.shots["net_" + what] = true
	if delay > 0.0:
		await ap.get_tree().create_timer(delay, true, false, true).timeout
	await Session.save_screenshot(ap.dir.path_join(shot_name(what)))
	ap._busy = false


func _end_with(what: String, code: int, s: Dictionary) -> void:
	_ending = true
	ap._busy = true
	ap.shots["net_" + what] = true
	await ap.get_tree().create_timer(0.8, true, false, true).timeout
	await Session.save_screenshot(ap.dir.path_join(shot_name(what)))
	write_hashes(view.state if not view.state.is_empty() else s)
	print("autoplay_net: done (%s); screenshots %s" % [what, ap.shots.keys()])
	Session.quit_game(code)
