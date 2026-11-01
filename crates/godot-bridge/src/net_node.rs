//! `FrontlineNet`: the multiplayer lobby object for GDScript (S2
//! "Presenter"). A `RefCounted` (not a `Node`, so its `ready()` does not
//! clash with the `ready` signal); `Session.ensure_net()` keeps one across
//! the menu → match scene change. It wraps a [`NetSession`]; every call is
//! a quick, non-blocking step on the main thread. API in
//! `crates/godot-bridge/README.md` ("FrontlineNet").

use std::path::{Path, PathBuf};
use std::sync::Arc;

use godot::classes::{IRefCounted, Os, ProjectSettings, RefCounted};
use godot::prelude::*;

use crate::match_setup::{find_mods_dir, load_defs, mods_candidates};
use crate::net_session::{ChoiceUpdate, NetSession};

#[derive(GodotClass)]
#[class(base = RefCounted)]
pub struct FrontlineNet {
    base: Base<RefCounted>,
    /// `mods/ww2` directory; empty = next to the project, the executable or
    /// the source tree (as `FrontlineMatch.mods_dir`).
    #[var]
    mods_dir: GString,
    pub(crate) session: NetSession,
}

#[godot_api]
impl IRefCounted for FrontlineNet {
    fn init(base: Base<RefCounted>) -> Self {
        let mut session = NetSession::new();
        let user = Os::singleton().get_user_data_dir().to_string();
        if !user.is_empty() {
            session.user_dir = Some(PathBuf::from(user));
        }
        FrontlineNet {
            base,
            mods_dir: GString::new(),
            session,
        }
    }
}

#[godot_api]
impl FrontlineNet {
    /// The name `Hello` carries (call before `host`/`join`).
    #[func]
    fn set_name(&mut self, name: GString) {
        self.session.set_name(&name.to_string());
    }

    /// Starts the in-game relay on `port` (all interfaces, IPv4 and IPv6)
    /// and joins it as slot 0. False: `status().error` says why.
    #[func]
    fn host(&mut self, port: i32) -> bool {
        if !self.load_defs() {
            return false;
        }
        let Ok(port) = u16::try_from(port) else {
            self.session.report("bind_failed");
            return false;
        };
        self.session.host(port).is_ok()
    }

    /// Like `host`, but the relay listens on IPv4 loopback only: two
    /// instances on one machine (`--autoplay_net`) without the macOS
    /// "accept incoming connections" prompt. Join it at `127.0.0.1:port`.
    #[func]
    fn host_loopback(&mut self, port: i32) -> bool {
        if !self.load_defs() {
            return false;
        }
        let Ok(port) = u16::try_from(port) else {
            self.session.report("bind_failed");
            return false;
        };
        self.session
            .host_on(std::net::SocketAddr::from(([127, 0, 0, 1], port)))
            .is_ok()
    }

    /// Joins `addr` (`1.2.3.4:47015`, `[::1]:47015`, `host.name:47015`).
    /// False: refused at once (`status().error`).
    #[func]
    fn join(&mut self, addr: GString) -> bool {
        if !self.load_defs() {
            return false;
        }
        self.session.join(&addr.to_string()).is_ok()
    }

    /// Partial lobby choice: any of `faction`, `seed`, `map`,
    /// `input_delay`. Slot 1's seed, map and delay are ignored by the relay.
    #[func]
    fn set_choice(&mut self, choice: VarDictionary) {
        let s = |k: &str| choice.get(k).map(|v| v.to_string());
        let n = |k: &str| choice.get(k).and_then(|v| v.try_to::<i64>().ok());
        self.session.set_choice(ChoiceUpdate {
            faction: s("faction"),
            seed: n("seed").map(|v| v.max(0) as u64),
            map: s("map"),
            d: n("input_delay").map(|v| v.clamp(0, 255) as u8),
        });
    }

    /// Sends `Ready` (the hash of the setup this client would play).
    #[func]
    fn ready(&mut self) {
        self.session.ready();
    }

    /// Same as `ready()`, for callers that hold this object as a `Node`
    /// type or want to avoid the name.
    #[func]
    fn set_ready(&mut self) {
        self.session.ready();
    }

    /// `Bye`, then idle. Safe in any state.
    #[func]
    fn leave(&mut self) {
        self.session.leave();
    }

    /// Scripted runs (S3 `--test_unlock`): the next match plays the test
    /// rules override (vehicles unlocked at once). Call before `host` /
    /// `join`; both players need it (the setups must agree).
    #[func]
    fn set_test_unlock(&mut self, on: bool) {
        self.session.set_test_unlock(on);
    }

    /// Scripted runs (`--start_manpower N`): both players start with N
    /// manpower (both instances need the same N). 0 or less: the rules'.
    #[func]
    fn set_start_manpower(&mut self, manpower: i64) {
        self.session.start_manpower = (manpower > 0).then(|| manpower.min(u32::MAX as i64) as u32);
    }

    /// Dev hook (`--debug_inject_desync TICK`): the next match steps TICK
    /// with one extra local-only `Stop`. 0 or less turns it off.
    #[func]
    fn set_debug_inject_desync(&mut self, tick: i64) {
        self.session.inject_desync = (tick > 0).then_some(tick as u64);
    }

    /// The build id `Hello` carries (`<commit>[-dirty]-p<protocol>-s<save>`).
    /// Both players need the same one; the menu shows it.
    #[func]
    fn build_id(&self) -> GString {
        frontline_net::build_id().as_str().into()
    }

    /// Lobby snapshot (`godot/docs/net_api.md`): state, slot, slots (name,
    /// faction, ping_ms, ready), seed, map, input_delay, suggested_delay,
    /// mismatch, countdown_s, error, error_detail, host_port.
    #[func]
    fn status(&mut self) -> VarDictionary {
        let v = self.session.view();
        let mut slots = VarArray::new();
        for s in &v.slots {
            slots.push(
                &vdict! {
                    "name" => s.name.as_str(),
                    "faction" => s.faction.as_str(),
                    "ping_ms" => s.ping_ms,
                    "ready" => s.ready,
                }
                .to_variant(),
            );
        }
        vdict! {
            "state" => v.state,
            "slot" => v.slot as i64,
            "slots" => &slots,
            "seed" => v.seed as i64,
            "map" => v.map.as_str(),
            "input_delay" => v.input_delay as i64,
            "suggested_delay" => v.suggested_delay as i64,
            "mismatch" => v.mismatch.as_str(),
            "countdown_s" => v.countdown_s,
            "error" => v.error.as_str(),
            "error_detail" => v.error_detail.as_str(),
            "host_port" => v.host_port as i64,
        }
    }
}

impl FrontlineNet {
    /// Loads `mods/ww2` afresh at every host and join (its content hash
    /// goes in `Hello`), so a lobby always plays the data on disk now (S3b:
    /// edited TOML is never stale; a player whose data differs is refused
    /// with `content_mismatch`).
    fn load_defs(&mut self) -> bool {
        let explicit = self.mods_dir.to_string();
        let project = ProjectSettings::singleton()
            .globalize_path("res://")
            .to_string();
        let candidates = mods_candidates(
            (!explicit.is_empty()).then(|| Path::new(&explicit)),
            Some(Path::new(&project)),
        );
        let loaded = find_mods_dir(&candidates)
            .ok_or_else(|| format!("cannot find mods/ww2 (looked in {candidates:?})"))
            .and_then(|dir| {
                load_defs(&dir, self.session.test_unlock())
                    .map_err(|e| format!("{}: {e}", dir.display()))
            });
        match loaded {
            Ok(defs) => {
                self.session.set_defs(Arc::new(defs));
                true
            }
            Err(e) => {
                godot_error!("FrontlineNet: {e}");
                self.session.report("content_missing");
                false
            }
        }
    }
}
