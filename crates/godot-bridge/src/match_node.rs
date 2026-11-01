//! `FrontlineMatch`: the S1 skirmish source node (§4.1, S1 "Presenter").
//!
//! It is the bridge-backed source `godot/match/match.gd` talks to, with the
//! same three methods as the mock (`get_state`, `handle_intent`,
//! `height_at`; schema in `godot/docs/match_state.md`), and it draws the
//! world the sim owns:
//!
//! * **Setup.** `start_match(setup)` turns the menu choice into a
//!   [`frontline_content::MatchSetup`] by reading `mods/ww2` (file IO is
//!   fine here, on the presenter side) and starts the sim thread
//!   ([`MatchHandle`]); the AI players run inside the sim.
//! * **Input.** Intents become the local player's `Command`s, queued for
//!   the next tick ([`crate::intents`]).
//! * **Terrain.** One `RenderingServer` instance per 64 m chunk; changed
//!   detail tiles rebuild only the chunks they touch, a few per frame
//!   ([`TerrainStore`]).
//! * **Units.** Soldiers and corpses are skinned MultiMesh instances (LOD
//!   layers plus shadow-only casters, as in the bench node) with stance and
//!   death clips; rounds are tracer streaks, mortar shells dark streaks,
//!   and each visible shot adds a short muzzle flash. One buffer upload per
//!   MultiMesh per frame, no Node per unit.
//! * **Fog of war.** Frames carry the local player's view; enemies,
//!   their squads, rounds and shots the player does not see are dropped
//!   here, before drawing, the HUD and audio (§5.12.3).
//!
//! Explosions (light, dust, smoke), flags, trees and sounds are drawn by
//! GDScript from the state's `effects` and `get_map_info()`.
//!
//! **Vehicles** (S3). The state carries `vehicles` (fog-filtered in
//! [`crate::match_frame`], interpolated here like soldiers), `shells`,
//! `armour_events`, the vehicle rules and types, and the S3 call-in, feed
//! and effect keys (`godot/docs/match_state.md` "Vehicles (S3)").
//! GDScript draws them (`match/vehicle_view.gd`, `match/vehicle_fx.gd`)
//! with the meshes of [`crate::meshgen`]. `start_match({scene: NAME})`
//! starts a staged scene of `frontline_sim::scenarios` instead of a
//! skirmish (`--autoplay_scene armour_<name>`); the state then has
//! `scene` (`ready`, `ready_tick`, `max_ticks`), and `get_map_info()` the
//! scene's `focus`.
//!
//! **Net mode** (S2). `attach_net(net)` hands the node a [`FrontlineNet`]
//! whose lobby started a match; `start_match({net: true, ...})` then takes
//! the setup and the link from it and runs the sim thread in net mode
//! ([`MatchHandle::spawn_net`]). `get_state()` adds `net` (link readout),
//! `pending_orders` (own orders sent but not run yet) and the lobby names.
//!
//! The node must sit at the world origin with an identity transform.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::Ordering;
use std::time::Instant;

use frontline_content::{ArmourZone, ItemKind, MatchSetup, SeatRole, SquadRole, WeaponClass};
use frontline_sim::snapshot::{NONE_U8, WRECK_OWNER};
use frontline_sim::{
    ArmourEventKind, CommandKind, Fx, NavPhase, OrderKind, Outcome, SeatSnap, SquadId, World,
    vehicle_flags,
};
use godot::classes::base_material_3d::{Flags as MatFlags, ShadingMode};
use godot::classes::image::Format as ImageFormat;
use godot::classes::light_3d::Param as LightParam;
use godot::classes::rendering_server::ShadowCastingSetting;
use godot::classes::{
    ArrayMesh, DirectionalLight3D, Engine, INode3D, Image, ImageTexture, Material, Node3D, Os,
    ProjectSettings, RenderingServer, Shader, ShaderMaterial, StandardMaterial3D,
};
use godot::prelude::*;

use crate::gfx::{Layer, build_mesh, soldier_material};
use crate::intents::{
    GROUP_SPACING_M, MIN_DIG_PIECE_M, ammo_choice_name, call_in_by_id, call_in_type, dig_command,
    dig_profile, item_name, role_name, sim_to_fx, split_polyline, spread_targets,
    stance_order_name, unit_command, unit_set, vehicle_command,
};
use crate::match_events::{
    EffectCue, EventTracker, UiEvent, ammo_word, effect_cues, vehicle_short,
};
use crate::match_frame::{ArmourView, MatchFrame, SquadView, TickEvents};
use crate::match_net::{NetLink, NetMatchInfo, NetState};
use crate::match_setup::{MenuChoice, find_mods_dir, mods_candidates, resolve, short_id};
use crate::match_thread::MatchHandle;
use crate::meshgen;
use crate::net_node::FrontlineNet;
use crate::present::{
    CasterCull, Frustum, carry_over, hash01, interp_pair, lerp, lerp_angle, model_yaw,
    sim_to_godot, write_streak_instance, write_yaw_instance,
};
use crate::rig;
use crate::terrain_view::TerrainStore;

const SOLDIER_SHADER: &str = "res://shaders/soldier_skinned.gdshader";
const TRACER_SHADER: &str = "res://shaders/tracer.gdshader";
const TERRAIN_SHADER: &str = "res://shaders/terrain.gdshader";
/// Planar jump (m) between two snapshots treated as a new entity.
const MAX_JUMP_M: f32 = 10.0;
/// Frames kept for interpolation (oldest first).
const HIST: usize = 3;
/// Seconds a muzzle flash shows.
const FLASH_S: f32 = 0.06;
/// Seconds between `dig` sound cues of one digging squad.
const DIG_CUE_S: f32 = 1.1;
/// Presenter frames the sim waits before its first tick, so start-up work
/// (shader compiles, the first terrain upload) does not make it late.
const WARMUP_FRAMES: u32 = 8;
/// Pending-order markers kept at most (the oldest go first).
const MAX_PENDING_MARKERS: usize = 64;
/// Armour events kept for the next `get_state()` at most.
const MAX_ARMOUR_BACKLOG: usize = 256;
/// State names of `ComponentKind::index`, `SeatRole::index`,
/// `ArmourZone::index` and `ArmourEventKind` (`match_state.md`).
const COMPONENT_KEYS: [&str; 7] = [
    "engine",
    "track_l",
    "track_r",
    "gun",
    "ring",
    "ammo_rack",
    "transmission",
];
const SEAT_KEYS: [&str; 5] = ["driver", "gunner", "commander", "loader", "radio_mg"];
const ZONE_KEYS: [&str; 8] = [
    "hull_front",
    "hull_side",
    "hull_rear",
    "hull_top",
    "turret_front",
    "turret_side",
    "turret_rear",
    "turret_top",
];
const ARMOUR_KINDS: [&str; 12] = [
    "fired",
    "ricochet",
    "no_pen",
    "pen",
    "component_hit",
    "crew_hit",
    "disabled",
    "knocked_out",
    "bogged",
    // S3c.
    "abandoned",
    "crew_out",
    "recrewed",
];

/// S3c `abandon_cause` names (`bail::AB_*`, index 0: not abandoned).
const ABANDON_KEYS: [&str; 7] = [
    "",
    "ammo_fire",
    "engine_fire",
    "crew",
    "useless",
    "under_fire",
    "order",
];

/// A vehicle's state badge (`match_state.md`): the first that applies.
/// Enemies get only wreck, burning, moving or idle.
fn vehicle_badge(flags: u16, speed: f32, own: bool) -> &'static str {
    use vehicle_flags::*;
    if flags & WRECK != 0 {
        return "wreck";
    }
    // S3c: before burning (an abandoned tank may still burn: the flag says
    // so, the badge says nobody is in it).
    if flags & ABANDONED != 0 {
        return "abandoned";
    }
    if flags & BURNING != 0 {
        return "burning";
    }
    if !own {
        return if speed.abs() > 1.0 { "moving" } else { "idle" };
    }
    for (bit, name) in [
        (BOGGED, "bogged"),
        (IMMOBILISED, "immobilised"),
        (REPAIRING, "repairing"),
        (CROSSING, "crossing"),
        (REVERSING, "reversing"),
        (MOVING, "moving"),
    ] {
        if flags & bit != 0 {
            return name;
        }
    }
    "idle"
}

/// `rules.vehicle_types` (`match_state.md`): every vehicle type by its
/// key (the def id without the mod prefix), with the box sizes the meshes,
/// footprints, picking and card silhouettes are built from.
fn vehicle_types(setup: &MatchSetup) -> VarDictionary {
    let fx = |v: frontline_content::Fixed| Fx::from(v).to_f32();
    let mut out = VarDictionary::new();
    for v in &setup.vehicles {
        let hull = &v.hull;
        let tur = &v.turret;
        let gun = v.main_gun();
        let seats: PackedStringArray = SeatRole::ALL
            .iter()
            .filter(|r| v.seat(**r).is_some())
            .map(|r| GString::from(SEAT_KEYS[r.index()]))
            .collect();
        let mut ammo = VarArray::new();
        for load in &gun.ammo {
            let Some(a) = setup.ammo.get(load.ammo as usize) else {
                continue;
            };
            ammo.push(
                &vdict! {
                    "kind" => match ammo_word(setup, load.ammo) { "he" => "he", _ => "ap" },
                    "name" => a.name.as_str(),
                    "max" => load.count as i64,
                }
                .to_variant(),
            );
        }
        let faction = setup
            .factions
            .get(v.faction as usize)
            .map_or("", |f| short_id(&f.id));
        let d = vdict! {
            "id" => v.id.as_str(),
            "name" => v.name.as_str(),
            "short" => v.short_name.as_str(),
            "faction" => faction,
            "role" => "tank",
            "cost" => v.cost as i64,
            "pop" => v.pop as i64,
            "recall_s" => v.recall_cooldown_ticks as f32 / setup.tick_hz.max(1) as f32,
            "speed" => fx(v.mobility.speed),
            "sight_m" => fx(v.sight),
            "hull_length" => 2.0 * fx(hull.half.x),
            "hull_width" => 2.0 * fx(hull.half.y),
            "hull_height" => 2.0 * fx(hull.half.z),
            "hull_ground" => fx(hull.center.z) - fx(hull.half.z),
            "turret_x" => fx(v.turret_pivot.x),
            "turret_base" => fx(v.turret_pivot.z),
            "turret_length" => 2.0 * fx(tur.half.x),
            "turret_width" => 2.0 * fx(tur.half.y),
            "turret_height" => 2.0 * fx(tur.half.z),
            "turret_offset_x" => fx(tur.center.x),
            "gun_length" => fx(gun.muzzle.x),
            "gun_height" => fx(gun.muzzle.z),
            "glacis_deg" => v.armour[ArmourZone::HullFront.index()].slope_deg as f32,
            "seats" => &seats,
            "ammo" => &ammo,
        };
        out.set(short_id(&v.id), &d);
    }
    out
}

#[derive(GodotClass)]
#[class(base = Node3D)]
pub struct FrontlineMatch {
    base: Base<Node3D>,
    /// Workers in the sim's pool.
    #[export]
    sim_threads: i32,
    /// `mods/ww2` directory; empty = next to the project, the executable or
    /// the source tree.
    #[export]
    mods_dir: GString,
    #[export]
    lod0_distance: f32,
    #[export]
    lod1_distance: f32,
    #[export]
    draw_distance: f32,
    #[export]
    tracer_seconds: f32,
    #[export]
    interp_margin_ms: f32,
    /// Terrain chunks rebuilt per frame at most (the rest wait).
    #[export]
    chunk_rebuilds_per_frame: i32,
    #[export]
    sun: Option<Gd<DirectionalLight3D>>,
    #[export]
    soldier_shader: Option<Gd<Shader>>,
    #[export]
    tracer_shader: Option<Gd<Shader>>,
    /// Terrain look; without one the chunks use plain vertex colours.
    #[export]
    terrain_shader: Option<Gd<Shader>>,
    /// Player slot of this client (set by `start_match`).
    #[var]
    local_player: i32,
    rt: Option<MatchRuntime>,
    last_error: GString,
    /// The lobby object of a net match (`attach_net`).
    net: Option<Gd<FrontlineNet>>,
}

#[godot_api]
impl INode3D for FrontlineMatch {
    fn init(base: Base<Node3D>) -> Self {
        FrontlineMatch {
            base,
            sim_threads: 2,
            mods_dir: GString::new(),
            lod0_distance: 25.0,
            lod1_distance: 80.0,
            draw_distance: 1200.0,
            tracer_seconds: 0.015,
            interp_margin_ms: 12.0,
            chunk_rebuilds_per_frame: 4,
            sun: None,
            soldier_shader: None,
            tracer_shader: None,
            terrain_shader: None,
            local_player: 0,
            rt: None,
            last_error: GString::new(),
            net: None,
        }
    }

    fn ready(&mut self) {
        // Draw before the match root reads the state in its own _process.
        self.base_mut().set_process_priority(-10);
    }

    fn process(&mut self, delta: f64) {
        let t0 = Instant::now();
        let cam = self.base().get_viewport().and_then(|vp| vp.get_camera_3d());
        let (cam_pos, cam_fwd, frustum) = match &cam {
            Some(cam) => {
                let xf = cam.get_global_transform();
                let p = xf.origin;
                let f = -xf.basis.col_c().normalized();
                let planes = cam
                    .get_frustum()
                    .iter_shared()
                    .map(|pl| [pl.normal.x, pl.normal.y, pl.normal.z, pl.d])
                    .collect();
                ([p.x, p.y, p.z], [f.x, f.y, f.z], Frustum { planes })
            }
            None => ([0.0, 100.0, 0.0], [0.0, -1.0, 0.0], Frustum::all()),
        };
        let mut cull = CasterCull {
            frustum: frustum.clone(),
            cam: cam_pos,
            cam_fwd,
            ..CasterCull::default()
        };
        if let Some(sun) = &self.sun
            && sun.is_visible_in_tree()
            && sun.has_shadow()
        {
            let l = -sun.get_global_transform().basis.col_c().normalized();
            cull.light = [l.x, l.y, l.z];
            cull.max_depth = sun.get_param(LightParam::SHADOW_MAX_DISTANCE);
            cull.enabled = true;
        }
        let lods = [
            self.lod0_distance,
            self.lod1_distance,
            self.draw_distance,
            self.tracer_seconds,
        ];
        let margin_s = (self.interp_margin_ms.max(0.0) / 1000.0) as f64;
        let rebuilds = self.chunk_rebuilds_per_frame.max(1) as usize;
        if let Some(rt) = self.rt.as_mut() {
            // One tick behind real time, plus the margin. Above 1x speed
            // (scripted runs) ticks come faster: a fixed 1x delay would put
            // the render time before the oldest kept frame, and nothing
            // would interpolate.
            let speed = rt.sim.speed().max(0.25) as f64;
            rt.delay = rt.period as f64 / speed + margin_s;
            rt.update(&frustum, &cull, delta as f32, lods, rebuilds);
            rt.present_us = t0.elapsed().as_secs_f32() * 1e6;
            if !rt.sim.is_alive() && !rt.sim_dead_reported {
                rt.sim_dead_reported = true;
                godot_error!(
                    "FrontlineMatch: the sim thread panicked and stopped: {}",
                    rt.sim.panic_message().unwrap_or_default()
                );
            }
        }
    }

    fn exit_tree(&mut self) {
        self.stop_match();
    }
}

#[godot_api]
impl FrontlineMatch {
    /// Starts a skirmish from `setup` (`Session.setup`: faction, difficulty,
    /// map, seed; optional ai_vs_ai, tickets, start_manpower). Returns false
    /// and sets `get_last_error()` on failure.
    #[func]
    fn start_match(&mut self, setup: VarDictionary) -> bool {
        self.stop_match();
        if Engine::singleton().is_editor_hint() {
            return false;
        }
        match self.try_start(&setup) {
            Ok(()) => {
                self.last_error = GString::new();
                true
            }
            Err(e) => {
                godot_error!("FrontlineMatch: {e}");
                self.last_error = GString::from(e.as_str());
                false
            }
        }
    }

    /// S3b quick iteration: re-reads `mods/` from disk and resolves `setup`
    /// as `start_match` would, without starting anything. Returns "" when
    /// the data is good, else the readable error (file, key, fix) for a
    /// banner. The debug "Reload data & restart" (F5) calls it before it
    /// restarts the match, so broken TOML never ends the running match.
    #[func]
    fn check_mods(&self, setup: VarDictionary) -> GString {
        let r = (|| -> Result<(), String> {
            if setup
                .get("net")
                .and_then(|v| v.try_to::<bool>().ok())
                .unwrap_or(false)
            {
                return Err("a net match cannot reload its data".into());
            }
            let mods = self.find_mods()?;
            resolve(&mods, &menu_choice(&setup))
                .map(|_| ())
                .map_err(|e| format!("{}: {e}", mods.display()))
        })();
        match r {
            Ok(()) => GString::new(),
            Err(e) => GString::from(e.as_str()),
        }
    }

    /// Net mode: the `FrontlineNet` whose lobby started the match (call
    /// before `start_match({net: true, ...})`). Anything else (a mock, null)
    /// detaches.
    #[func]
    fn attach_net(&mut self, net: Variant) {
        self.net = net.try_to::<Gd<FrontlineNet>>().ok();
    }

    /// Stops the sim thread and frees render resources.
    #[func]
    fn stop_match(&mut self) {
        if let Some(mut rt) = self.rt.take() {
            rt.free();
        }
    }

    #[func]
    fn is_running(&self) -> bool {
        self.rt.is_some()
    }

    #[func]
    fn get_last_error(&self) -> GString {
        self.last_error.clone()
    }

    /// One frame of match state (`godot/docs/match_state.md`). `events` and
    /// `effects` hold what is new since the previous call.
    #[func]
    fn get_state(&mut self) -> VarDictionary {
        match self.rt.as_mut() {
            Some(rt) => rt.state_dict(),
            None => VarDictionary::new(),
        }
    }

    /// Turns a player intent into commands for the next tick.
    #[func]
    fn handle_intent(&mut self, kind: StringName, data: VarDictionary) {
        if let Some(rt) = self.rt.as_mut() {
            rt.handle_intent(&kind.to_string(), &data);
        }
    }

    /// Ground height (Godot Y) at Godot (x, z), including dug terrain.
    #[func]
    fn height_at(&self, x: f32, z: f32) -> f32 {
        self.rt
            .as_ref()
            .map_or(0.0, |rt| rt.terrain.height_at_godot(x, z))
    }

    /// Same as `height_at` (the camera script's name for it).
    #[func]
    fn get_height_at(&self, x: f32, z: f32) -> f32 {
        self.height_at(x, z)
    }

    /// How deep the ground at Godot (x, z) is dug below the undug surface
    /// (m): trenches and craters, as the player sees them. Scripted runs use
    /// it to find trenches.
    #[func]
    fn dug_depth_at(&self, x: f32, z: f32) -> f32 {
        self.rt.as_ref().map_or(0.0, |rt| {
            rt.terrain
                .dug_depth(x + rt.terrain.half(), -z + rt.terrain.half())
        })
    }

    /// Static map layout for presenter scenery: size, woods, points, bases
    /// (Godot space), and the local player.
    #[func]
    fn get_map_info(&self) -> VarDictionary {
        match &self.rt {
            Some(rt) => rt.map_info(),
            None => VarDictionary::new(),
        }
    }

    /// Ascending ids of `owner`'s squads the local player knows (own, or
    /// spotted enemy), optionally of one type (`rifle`, `mg`, ...).
    #[func]
    fn squad_ids(&self, owner: i32, squad_type: GString) -> PackedInt32Array {
        let Some(rt) = &self.rt else {
            return PackedInt32Array::new();
        };
        let ty = squad_type.to_string();
        rt.newest()
            .squads
            .iter()
            .filter(|s| s.owner as i32 == owner && s.visible)
            .filter(|s| ty.is_empty() || rt.role(s.type_idx) == ty)
            .map(|s| s.id as i32)
            .collect()
    }

    /// Godot position of a known squad's leader (ZERO when unknown).
    #[func]
    fn squad_pos(&self, id: i32) -> Vector3 {
        let Some(rt) = &self.rt else {
            return Vector3::ZERO;
        };
        rt.newest()
            .squads
            .iter()
            .find(|s| s.id as i32 == id && s.visible)
            .map_or(Vector3::ZERO, |s| rt.godot(s.anchor))
    }

    /// Sim speed relative to real time (0.25–8; scripted runs).
    #[func]
    fn set_speed(&mut self, speed: f32) {
        if let Some(rt) = &self.rt {
            rt.sim.set_speed(speed);
        }
    }

    #[func]
    fn get_speed(&self) -> f32 {
        self.rt.as_ref().map_or(1.0, |rt| rt.sim.speed())
    }

    /// Presenter and sim counters for debugging and the autoplay log.
    #[func]
    fn get_stats(&self) -> VarDictionary {
        use std::sync::atomic::Ordering::Relaxed;
        let Some(rt) = &self.rt else {
            return VarDictionary::new();
        };
        let c = &rt.sim.counters;
        let f = rt.newest();
        vdict! {
            "tick" => f.tick as i64,
            "sim_alive" => rt.sim.is_alive(),
            "sim_step_ms_last" => c.last_step_us.load(Relaxed) as f32 / 1000.0,
            "sim_step_ms_max" => c.max_step_us.load(Relaxed) as f32 / 1000.0,
            "sim_step_max_tick" => c.max_step_tick.load(Relaxed) as i64,
            "sim_late_ticks" => c.late_ticks.load(Relaxed) as i64,
            "commands_sent" => c.commands_sent.load(Relaxed) as i64,
            "soldiers" => f.soldiers.len() as i64,
            "corpses" => f.corpses.len() as i64,
            "rounds" => f.rounds.len() as i64,
            "soldiers_drawn" => rt.soldier_layers.iter().map(|l| l.count as i64).sum::<i64>(),
            "tracers_drawn" => rt.tracer_layer.count as i64,
            "thrown_trails_drawn" => rt.thrown_layer.count as i64,
            "detail_tiles" => rt.terrain.tile_count() as i64,
            "chunks_pending" => rt.dirty_chunks.len() as i64,
            "chunk_rebuilds" => rt.chunk_rebuilds as i64,
            "presenter_us" => rt.present_us,
            "speed" => rt.sim.speed(),
            "vehicles_own" => f.vehicles.iter().filter(|v| v.owner == rt.local).count() as i64,
            "vehicles_enemy" => f
                .vehicles
                .iter()
                .filter(|v| v.owner != rt.local && v.owner != WRECK_OWNER)
                .count() as i64,
            "wrecks" => f.vehicles.iter().filter(|v| v.owner == WRECK_OWNER).count() as i64,
            "shells" => f.shells.len() as i64,
        }
    }

    /// Net mode: the hashes this client published, oldest first, as
    /// `[tick, hash as 16 hex digits]` pairs (the last
    /// `match_net::HASH_KEEP`). Two-instance runs write them out and
    /// compare. Empty in a skirmish.
    #[func]
    fn get_net_hashes(&self) -> VarArray {
        let mut out = VarArray::new();
        if let Some(rt) = &self.rt {
            for (tick, hash) in crate::match_net::hashes(&rt.sim.counters) {
                let pair = varray![tick as i64, format!("{hash:016x}").as_str()];
                out.push(&pair.to_variant());
            }
        }
        out
    }
}

/// The menu choice in a `Session.match_setup()` dictionary.
fn menu_choice(d: &VarDictionary) -> MenuChoice {
    let s = |k: &str, def: &str| {
        d.get(k)
            .map(|v| v.to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| def.to_owned())
    };
    let n = |k: &str| d.get(k).and_then(|v| v.try_to::<i64>().ok());
    MenuChoice {
        faction: s("faction", "germany"),
        difficulty: s("difficulty", "normal"),
        map: s("map", "kessel_ridge"),
        seed: n("seed").unwrap_or(1).max(0) as u64,
        ai_vs_ai: d
            .get("ai_vs_ai")
            .and_then(|v| v.try_to::<bool>().ok())
            .unwrap_or(false),
        tickets: n("tickets").filter(|&t| t > 0).map(|t| t as u32),
        start_manpower: n("start_manpower").filter(|&t| t >= 0).map(|t| t as u32),
        slots: None,
        test_unlock: d
            .get("test_unlock")
            .and_then(|v| v.try_to::<bool>().ok())
            .unwrap_or(false),
    }
}

impl FrontlineMatch {
    /// The `mods/ww2` directory: `mods_dir`, else next to the project, the
    /// executable or the source tree (`mods_candidates`).
    fn find_mods(&self) -> Result<std::path::PathBuf, String> {
        let explicit = self.mods_dir.to_string();
        let project = ProjectSettings::singleton()
            .globalize_path("res://")
            .to_string();
        let candidates = mods_candidates(
            (!explicit.is_empty()).then(|| std::path::Path::new(&explicit)),
            Some(std::path::Path::new(&project)),
        );
        find_mods_dir(&candidates)
            .ok_or_else(|| format!("cannot find mods/ww2 (looked in {candidates:?})"))
    }

    fn try_start(&mut self, d: &VarDictionary) -> Result<(), String> {
        let choice = menu_choice(d);
        let is_net = d
            .get("net")
            .and_then(|v| v.try_to::<bool>().ok())
            .unwrap_or(false);
        if is_net {
            return self.try_start_net(d);
        }
        // Every new match reads the mod from disk afresh (no cache), so an
        // edited TOML takes effect at the next start (S3b).
        let mods = self.find_mods()?;
        let setup = resolve(&mods, &choice).map_err(|e| format!("{}: {e}", mods.display()))?;
        let threads = self.sim_threads.max(1) as usize;
        let scene = d
            .get("scene")
            .map(|v| v.to_string())
            .filter(|v| !v.is_empty());
        if let Some(name) = scene {
            // A staged scene (S3 "Done when" 7): built from the resolved
            // setup by the sim's own staging code; player 0 watches.
            let sc = frontline_sim::scenarios::scene(&name, setup, threads)
                .map_err(|e| format!("scene {name:?}: {e}"))?;
            let info = SceneInfo {
                name: name.clone(),
                focus: sc.focus.to_f32(),
                max_ticks: sc.max_ticks,
            };
            self.start_runtime(sc.world, 0, None, Some(name))?;
            if let Some(rt) = self.rt.as_mut() {
                for c in sc.commands {
                    rt.sim.send(c.kind);
                }
                rt.scene = Some(info);
            }
            return Ok(());
        }
        let world =
            World::new_match(setup, threads).map_err(|e| format!("invalid match setup: {e}"))?;
        self.start_runtime(world, 0, None, None)
    }

    /// Net mode: the setup and the link come from the attached
    /// `FrontlineNet`; the dictionary adds only `debug_inject_desync`.
    fn try_start_net(&mut self, d: &VarDictionary) -> Result<(), String> {
        let mut net = self
            .net
            .clone()
            .ok_or("net match without attach_net(FrontlineNet)")?;
        let names = net.bind().session.names();
        let nm = {
            let mut b = net.bind_mut();
            if let Some(t) = d
                .get("debug_inject_desync")
                .and_then(|v| v.try_to::<i64>().ok())
                .filter(|&t| t > 0)
            {
                b.session.inject_desync = Some(t as u64);
            }
            b.session.take_match()?
        };
        if let Some(want) = d.get("local_player").and_then(|v| v.try_to::<i64>().ok())
            && want != nm.slot as i64
        {
            godot_warn!(
                "FrontlineMatch: local_player {want} but the relay gave slot {}",
                nm.slot
            );
        }
        let mut info = nm.info;
        let user = Os::singleton().get_user_data_dir().to_string();
        if !user.is_empty() {
            info.user_dir = Some(std::path::PathBuf::from(user));
        }
        let world = World::new_match(nm.setup, self.sim_threads.max(1) as usize)
            .map_err(|e| format!("invalid match setup: {e}"))?;
        let slot = nm.slot;
        self.start_runtime(world, slot, Some((nm.link, info, names)), None)
    }

    fn start_runtime(
        &mut self,
        world: World,
        local: u8,
        net: Option<(NetLink, NetMatchInfo, [String; 2])>,
        scene_name: Option<String>,
    ) -> Result<(), String> {
        let world3d = self
            .base()
            .get_world_3d()
            .ok_or("node is not in a 3D world")?;
        let soldier_sh =
            shader_or_load(&self.soldier_shader, SOLDIER_SHADER).ok_or("missing soldier shader")?;
        let tracer_sh =
            shader_or_load(&self.tracer_shader, TRACER_SHADER).ok_or("missing tracer shader")?;
        let terrain_sh = shader_or_load(&self.terrain_shader, TERRAIN_SHADER);
        let scene = scene_name.clone();
        let rt = MatchRuntime::new(
            world,
            local,
            scene,
            net,
            world3d.get_scenario(),
            [soldier_sh, tracer_sh],
            terrain_sh,
        )
        .map_err(|e| format!("could not start the sim thread: {e}"))?;
        self.local_player = local as i32;
        self.rt = Some(rt);
        Ok(())
    }
}

fn shader_or_load(sh: &Option<Gd<Shader>>, path: &str) -> Option<Gd<Shader>> {
    sh.clone()
        .or_else(|| godot::tools::try_load::<Shader>(path).ok())
}

/// One terrain chunk: its instance and the mesh it shows.
struct Chunk {
    inst: Rid,
    _mesh: Gd<ArrayMesh>,
}

struct Flash {
    head: [f32; 3],
    dir: [f32; 3],
    born: f64,
}

struct MatchRuntime {
    sim: MatchHandle,
    setup: MatchSetup,
    local: u8,
    hz: f32,
    period: f32,
    delay: f64,
    half: f32,
    epoch: Instant,
    clock: f64,
    hist: [MatchFrame; HIST],
    nhist: usize,
    spare: MatchFrame,
    pair: Option<(u64, u64)>,
    start: Vec<[f32; 4]>,
    snapped: Vec<u32>,
    /// Per sim id: (walk phase, pose phase).
    phases: Vec<[f32; 2]>,
    /// Interpolated leader position (Godot) per drawn squad id.
    anchors: BTreeMap<u32, [f32; 3]>,
    /// Interpolated position (Godot, feet) per soldier of the drawn frame,
    /// ascending id: what the state's `soldiers[].pos` reports, so picking
    /// matches the picture.
    drawn: Vec<(u32, [f32; 3])>,
    terrain: TerrainStore,
    scenario: Rid,
    terrain_mat: Gd<Material>,
    chunks: Vec<Option<Chunk>>,
    dirty_chunks: BTreeSet<(i32, i32)>,
    chunk_rebuilds: u64,
    soldier_layers: [Layer; 3],
    soldier_casters: Layer,
    tracer_layer: Layer,
    shell_layer: Layer,
    /// Pale motion trails behind thrown items (their bodies go in
    /// `shell_layer`).
    thrown_layer: Layer,
    flash_layer: Layer,
    flashes: Vec<Flash>,
    /// Faction colour index per player slot (shader team tint).
    team_tint: Vec<f32>,
    events_in: Vec<TickEvents>,
    tracker: EventTracker,
    ui_events: Vec<UiEvent>,
    cues: Vec<EffectCue>,
    dig_cue: BTreeMap<u32, f32>,
    minimap: Option<Gd<ImageTexture>>,
    present_us: f32,
    sim_dead_reported: bool,
    warmup: u32,
    user_paused: bool,
    /// Net mode: the players' names and the pending-order markers.
    net: Option<NetView>,
    /// S3: vehicles of the drawn frame, interpolated like the soldiers.
    drawn_vehicles: Vec<DrawnVehicle>,
    /// Seconds the render time lies before the newest frame's tick (shells
    /// are drawn this far back along their flight).
    back_s: f32,
    /// Visible armour events not yet handed out by `get_state()`.
    armour_out: Vec<(u64, ArmourView)>,
    /// A staged scene (`start_match({scene})`).
    scene: Option<SceneInfo>,
    /// `rules.vehicle_types`, built once.
    vehicle_types: VarDictionary,
    _resources: Vec<Gd<Resource>>,
}

/// A vehicle as drawn this frame (Godot space).
#[derive(Copy, Clone, Debug)]
struct DrawnVehicle {
    id: u32,
    pos: [f32; 3],
    /// Model yaw ([`model_yaw`] of the interpolated heading).
    yaw: f32,
    pitch: f32,
    roll: f32,
    turret_yaw: f32,
    gun_pitch: f32,
}

/// The staged scene a runtime plays.
struct SceneInfo {
    name: String,
    /// Sim xy the camera looks at.
    focus: [f32; 2],
    max_ticks: u64,
}

/// The presenter's side of a net match.
struct NetView {
    names: [String; 2],
    /// `(send ordinal of the intent's last command, kind, Godot position)`,
    /// oldest first: drawn until that command ran.
    pending: VecDeque<(u64, &'static str, Vector3)>,
}

/// The pending-marker kind of a positional intent
/// (`godot/match/match_view.gd` `PENDING_KINDS`).
fn pending_kind(intent: &str) -> Option<&'static str> {
    Some(match intent {
        "move" | "move_units" | "reverse_units" => "move",
        "attack_move" | "attack_move_units" => "attack_move",
        "barrage" => "barrage",
        "throw" | "throw_each" => "throw",
        "dig" | "dig_units" => "dig",
        _ => return None,
    })
}

impl MatchRuntime {
    fn new(
        world: World,
        local: u8,
        scene: Option<String>,
        net: Option<(NetLink, NetMatchInfo, [String; 2])>,
        scenario: Rid,
        [soldier_sh, tracer_sh]: [Gd<Shader>; 2],
        terrain_sh: Option<Gd<Shader>>,
    ) -> std::io::Result<MatchRuntime> {
        let mut rs = RenderingServer::singleton();
        let setup = world.match_setup().cloned().expect("match world");
        let terrain = TerrainStore::new(world.terrain());
        let half = terrain.half();
        let map = terrain.size_m();
        let aabb = Aabb::new(
            Vector3::new(-half - 10.0, -50.0, -half - 10.0),
            Vector3::new(map + 20.0, 300.0, map + 20.0),
        );
        let mut resources: Vec<Gd<Resource>> = Vec::new();

        let terrain_mat: Gd<Material> = match terrain_sh {
            Some(sh) => {
                let mut m = ShaderMaterial::new_gd();
                m.set_shader(&sh);
                m.upcast()
            }
            None => {
                let mut m = StandardMaterial3D::new_gd();
                m.set_flag(MatFlags::ALBEDO_FROM_VERTEX_COLOR, true);
                m.set_roughness(1.0);
                m.upcast()
            }
        };

        let (soldier_mat, anim_tex) = soldier_material(&soldier_sh);
        resources.push(anim_tex.upcast());
        let cap = setup.rules.pop_cap as usize * setup.players.len() + 256;
        let mut lod1_mesh = Rid::Invalid;
        let soldier_layers: [Layer; 3] = std::array::from_fn(|lod| {
            let mesh = build_mesh(
                &rig::soldier_mesh(lod as u8),
                Some(soldier_mat.clone().upcast()),
            );
            let layer = Layer::new(
                &mut rs,
                mesh.get_rid(),
                scenario,
                if lod == 0 { 16 } else { 64 },
                cap,
                ShadowCastingSetting::OFF,
                aabb,
            );
            if lod == 1 {
                lod1_mesh = mesh.get_rid();
            }
            resources.push(mesh.upcast());
            layer
        });
        let soldier_casters = Layer::new(
            &mut rs,
            lod1_mesh,
            scenario,
            64,
            cap,
            ShadowCastingSetting::SHADOWS_ONLY,
            aabb,
        );

        let mut tracer_mat = ShaderMaterial::new_gd();
        tracer_mat.set_shader(&tracer_sh);
        let tracer = build_mesh(&meshgen::tracer_mesh(), Some(tracer_mat.clone().upcast()));
        let rounds_cap = setup.rules.projectile_cap as usize;
        let tracer_layer = Layer::new(
            &mut rs,
            tracer.get_rid(),
            scenario,
            64,
            rounds_cap.max(64),
            ShadowCastingSetting::OFF,
            aabb,
        );
        let flash_layer = Layer::new(
            &mut rs,
            tracer.get_rid(),
            scenario,
            32,
            512,
            ShadowCastingSetting::OFF,
            aabb,
        );
        resources.push(tracer.upcast());
        let mut shell_mat = StandardMaterial3D::new_gd();
        shell_mat.set_shading_mode(ShadingMode::UNSHADED);
        shell_mat.set_albedo(Color::from_rgb(0.07, 0.07, 0.06));
        let shell = build_mesh(&meshgen::tracer_mesh(), Some(shell_mat.upcast()));
        let shell_layer = Layer::new(
            &mut rs,
            shell.get_rid(),
            scenario,
            16,
            256,
            ShadowCastingSetting::OFF,
            aabb,
        );
        resources.push(shell.upcast());
        let mut trail_mat = ShaderMaterial::new_gd();
        trail_mat.set_shader(&tracer_sh);
        trail_mat.set_shader_parameter("color", &Color::from_rgb(0.72, 0.7, 0.62).to_variant());
        trail_mat.set_shader_parameter("energy", &1.4f32.to_variant());
        let trail = build_mesh(&meshgen::tracer_mesh(), Some(trail_mat.upcast()));
        let thrown_layer = Layer::new(
            &mut rs,
            trail.get_rid(),
            scenario,
            16,
            frontline_sim::MAX_THROWN.max(16),
            ShadowCastingSetting::OFF,
            aabb,
        );
        resources.push(trail.upcast());

        let team_tint = setup.players.iter().map(|p| p.faction as f32).collect();
        let per_side = terrain.chunks_per_side();
        let mut dirty_chunks = BTreeSet::new();
        for cy in 0..per_side {
            for cx in 0..per_side {
                dirty_chunks.insert((cx, cy));
            }
        }
        let minimap = minimap_texture(&terrain, &setup, 256);
        let hz = setup.tick_hz.max(1) as f32;
        // A skirmish waits a few frames before its first tick; a net match
        // runs on the relay's clock and catches up instead.
        let (mut sim, net, warmup) = match net {
            Some((link, info, names)) => (
                MatchHandle::spawn_net(world, local, link, info)?,
                Some(NetView {
                    names,
                    pending: VecDeque::new(),
                }),
                0,
            ),
            None => {
                let sim = MatchHandle::spawn_scene(world, local, scene)?;
                sim.set_paused(true);
                (sim, None, WARMUP_FRAMES)
            }
        };
        let mut hist = [
            MatchFrame::empty(),
            MatchFrame::empty(),
            MatchFrame::empty(),
        ];
        sim.take_current(&mut hist[HIST - 1]);
        let mut rt = MatchRuntime {
            sim,
            local,
            hz,
            period: 1.0 / hz,
            delay: 1.35 / hz as f64,
            half,
            epoch: Instant::now(),
            clock: 0.0,
            hist,
            nhist: 1,
            spare: MatchFrame::empty(),
            pair: None,
            start: Vec::new(),
            snapped: Vec::new(),
            phases: Vec::new(),
            anchors: BTreeMap::new(),
            drawn: Vec::new(),
            terrain,
            scenario,
            terrain_mat,
            chunks: (0..per_side * per_side).map(|_| None).collect(),
            dirty_chunks,
            chunk_rebuilds: 0,
            soldier_layers,
            soldier_casters,
            tracer_layer,
            shell_layer,
            thrown_layer,
            flash_layer,
            flashes: Vec::new(),
            team_tint,
            events_in: Vec::new(),
            tracker: EventTracker::new(local),
            ui_events: Vec::new(),
            cues: Vec::new(),
            dig_cue: BTreeMap::new(),
            minimap,
            present_us: 0.0,
            sim_dead_reported: false,
            warmup,
            user_paused: false,
            net,
            drawn_vehicles: Vec::new(),
            back_s: 0.0,
            armour_out: Vec::new(),
            scene: None,
            vehicle_types: vehicle_types(&setup),
            _resources: resources,
            setup,
        };
        // The whole map once, before the first frame is shown.
        let all = rt.dirty_chunks.len();
        rt.rebuild_chunks(all);
        let first = std::mem::replace(&mut rt.hist[HIST - 1], MatchFrame::empty());
        rt.tracker.update(&first, &rt.setup, &mut rt.ui_events);
        rt.hist[HIST - 1] = first;
        Ok(rt)
    }

    fn newest(&self) -> &MatchFrame {
        &self.hist[HIST - 1]
    }

    fn role(&self, type_idx: u16) -> &'static str {
        self.setup
            .squad_types
            .get(type_idx as usize)
            .map_or("rifle", |t| role_name(t.role))
    }

    /// Sim `[x, y, z]` → Godot.
    fn godot(&self, p: [f32; 3]) -> Vector3 {
        let g = sim_to_godot(p, self.half);
        Vector3::new(g[0], g[1], g[2])
    }

    /// Sim `[x, y]` → Godot on the ground.
    fn ground(&self, p: [f32; 2]) -> Vector3 {
        let h = self.terrain.height_at(p[0], p[1]);
        self.godot([p[0], p[1], h])
    }

    fn secs(&self, t: Instant) -> f64 {
        t.saturating_duration_since(self.epoch).as_secs_f64()
    }

    fn pull(&mut self) {
        // Frames: newest wins; the HUD event tracker sees every one pulled.
        while self.sim.take_newest(&mut self.spare) {
            self.hist.rotate_left(1);
            std::mem::swap(&mut self.hist[HIST - 1], &mut self.spare);
            self.nhist = (self.nhist + 1).min(HIST);
            let f = std::mem::replace(&mut self.hist[HIST - 1], MatchFrame::empty());
            self.tracker.update(&f, &self.setup, &mut self.ui_events);
            self.hist[HIST - 1] = f;
        }
        // Events: every tick's, in order.
        self.events_in.clear();
        self.sim.drain_events(&mut self.events_in);
        let mut dirty = std::mem::take(&mut self.dirty_chunks);
        for ev in &self.events_in {
            for t in &ev.tiles {
                self.terrain.apply(t, &mut dirty);
            }
        }
        self.dirty_chunks = dirty;
        effect_cues(&self.events_in, &self.setup, &mut self.cues);
        for ev in &self.events_in {
            self.tracker.call_ins(ev, &self.setup, &mut self.ui_events);
            self.tracker.armour(ev, &self.setup, &mut self.ui_events);
            self.armour_out
                .extend(ev.armour.iter().map(|a| (ev.tick, a.clone())));
        }
        if self.armour_out.len() > MAX_ARMOUR_BACKLOG {
            let drop = self.armour_out.len() - MAX_ARMOUR_BACKLOG;
            self.armour_out.drain(..drop);
        }

        // Bound the cue backlog if nobody reads the state.
        if self.cues.len() > 512 {
            let drop = self.cues.len() - 512;
            self.cues.drain(..drop);
        }
        if self.ui_events.len() > 64 {
            let drop = self.ui_events.len() - 64;
            self.ui_events.drain(..drop);
        }
    }

    fn rebuild_chunks(&mut self, max: usize) {
        let mut rs = RenderingServer::singleton();
        let per_side = self.terrain.chunks_per_side();
        for _ in 0..max {
            let Some((cx, cy)) = self.dirty_chunks.pop_first() else {
                break;
            };
            let md = self.terrain.chunk_mesh(cx, cy);
            let mesh = build_mesh(&md, Some(self.terrain_mat.clone()));
            let slot = &mut self.chunks[(cy * per_side + cx) as usize];
            match slot {
                Some(c) => {
                    rs.instance_set_base(c.inst, mesh.get_rid());
                    c._mesh = mesh;
                }
                None => {
                    let inst = rs.instance_create2(mesh.get_rid(), self.scenario);
                    rs.instance_geometry_set_cast_shadows_setting(inst, ShadowCastingSetting::ON);
                    *slot = Some(Chunk { inst, _mesh: mesh });
                }
            }
            self.chunk_rebuilds += 1;
        }
    }

    fn update(
        &mut self,
        frustum: &Frustum,
        cull: &CasterCull,
        dt: f32,
        [lod0, lod1, draw, tracer_s]: [f32; 4],
        rebuilds: usize,
    ) {
        let mut rs = RenderingServer::singleton();
        self.clock += dt as f64;
        if self.warmup > 0 {
            self.warmup -= 1;
            if self.warmup == 0 && !self.user_paused {
                self.sim.set_paused(false);
            }
        }
        self.pull();
        self.rebuild_chunks(rebuilds);

        // Render time and the snapshot pair around it.
        let r = self.secs(Instant::now()) - self.delay;
        let base = HIST - self.nhist;
        let mut stamps = [0.0f64; HIST];
        for (st, f) in stamps.iter_mut().zip(&self.hist[base..]) {
            *st = self.secs(f.sched);
        }
        let paused = self.newest().paused;
        let (ai, bi, alpha) = if paused {
            (HIST - 1, HIST - 1, 1.0)
        } else {
            let (a, b, al) = interp_pair(&stamps[..self.nhist], r);
            (base + a, base + b, al)
        };
        let stamp_b = stamps[bi - base];
        let key = (self.hist[ai].tick, self.hist[bi].tick);
        if self.pair != Some(key) {
            self.pair = Some(key);
            let (a, b) = (&self.hist[ai], &self.hist[bi]);
            self.snapped.clear();
            carry_over(
                &a.ids,
                &a.soldiers,
                &b.ids,
                &b.soldiers,
                MAX_JUMP_M,
                &mut self.start,
                &mut self.snapped,
            );
            if let Some(&max_id) = b.ids.iter().chain(b.corpses.iter().map(|c| &c.id.0)).max() {
                while self.phases.len() <= max_id as usize {
                    let id = self.phases.len() as u32;
                    self.phases.push([hash01(id), hash01(id ^ 0xA5A5_1234)]);
                }
            }
            for &id in &self.snapped {
                self.phases[id as usize] = [hash01(id), hash01(id ^ 0xA5A5_1234)];
            }
        }

        let MatchRuntime {
            hist,
            start,
            phases,
            anchors,
            drawn,
            soldier_layers,
            soldier_casters,
            tracer_layer,
            shell_layer,
            thrown_layer,
            flash_layer,
            flashes,
            team_tint,
            events_in,
            setup,
            half,
            period,
            hz,
            clock,
            drawn_vehicles,
            back_s,
            ..
        } = self;
        let (half, period, hz) = (*half, *period, *hz);
        let a = &hist[ai];
        let b = &hist[bi];
        let cam = cull.cam;
        let (lod0_2, lod1_2, draw_2) = (lod0 * lod0, lod1 * lod1, draw * draw);
        let dist2 = |p: [f32; 3]| {
            let d = [p[0] - cam[0], p[1] - cam[1], p[2] - cam[2]];
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
        };
        let tint = |owner: u8| team_tint.get(owner as usize).copied().unwrap_or(0.0);
        let dt_anim = if paused { 0.0 } else { dt };

        for l in soldier_layers.iter_mut() {
            l.begin();
        }
        soldier_casters.begin();
        anchors.clear();
        drawn.clear();
        let mut leaders: BTreeMap<u32, u32> = BTreeMap::new();
        for s in &b.squads {
            leaders.insert(s.leader, s.id);
        }
        let put = |gp: [f32; 3],
                   yaw: f32,
                   custom: [f32; 4],
                   layers: &mut [Layer; 3],
                   casters: &mut Layer,
                   cast: bool| {
            let centre = [gp[0], gp[1] + 0.9, gp[2]];
            if cast
                && cull.casts_into_view(centre, 1.2, 1.9)
                && let Some(slot) = casters.push(centre, 1.2)
            {
                write_yaw_instance(slot, gp, yaw, 1.0, custom);
            }
            if !frustum.sphere_visible(centre, 1.2) {
                return;
            }
            let d2 = dist2(gp);
            let lod = if d2 <= lod0_2 {
                0
            } else if d2 <= lod1_2 {
                1
            } else if d2 <= draw_2 {
                2
            } else {
                return;
            };
            if let Some(slot) = layers[lod].push(centre, 1.2) {
                write_yaw_instance(slot, gp, yaw, 1.0, custom);
            }
        };
        for (i, (&s, &c)) in start.iter().zip(&b.soldiers).enumerate() {
            let id = b.ids[i];
            let v = [
                lerp(s[0], c[0], alpha),
                lerp(s[1], c[1], alpha),
                lerp(s[2], c[2], alpha),
                lerp_angle(s[3], c[3], alpha),
            ];
            let gp = sim_to_godot([v[0], v[1], v[2]], half);
            if let Some(&sq) = leaders.get(&id) {
                anchors.insert(sq, gp);
            }
            drawn.push((id, gp));
            if !b.visible[i] {
                continue;
            }
            // The sim's stance already follows stance orders (S1b), so the
            // pose clips show them. There is no crouch-walk or crawl clip:
            // a crouched man walking keeps half the crouch pose, and a prone
            // man crawling runs the prone loop faster.
            let stance = b.stance[i].min(2);
            let spd = if stance == 2 { 0.0 } else { b.speeds[i] };
            let crawl = if stance == 2 { b.speeds[i] * 2.0 } else { 0.0 };
            let ph = &mut phases[id as usize];
            ph[0] = (ph[0] + dt_anim * spd / rig::STRIDE_M).fract();
            ph[1] = (ph[1] + dt_anim * (0.25 + crawl)).fract();
            let walk_max = if stance == 1 { 0.5 } else { 1.0 };
            let walk = ((spd - 0.25) / 0.75).clamp(0.0, walk_max);
            let custom = [
                walk,
                ph[0],
                stance as f32 + ph[1].min(0.999),
                tint(b.owner[i]),
            ];
            put(
                gp,
                model_yaw(v[3]),
                custom,
                soldier_layers,
                soldier_casters,
                true,
            );
        }
        // Corpses: the death fall, then lying still.
        let corpse_ticks = setup.rules.combat.corpse_ticks as f32;
        let sub_tick = if paused { 0.0 } else { alpha * period };
        for (k, c) in b.corpses.iter().enumerate() {
            if !b.corpse_visible[k] {
                continue;
            }
            let age = (corpse_ticks - c.ticks_left as f32) / hz + sub_tick;
            let fall = (age / rig::DEATH_FALL_S).clamp(0.0, 1.0);
            let gp = sim_to_godot([c.pos[0], c.pos[1], c.pos[2]], half);
            let custom = [0.0, 0.0, 3.0 + fall, tint(c.owner)];
            put(
                gp,
                model_yaw(c.pos[3]),
                custom,
                soldier_layers,
                soldier_casters,
                false,
            );
        }

        // Vehicles (S3), at the same render time: GDScript draws them from
        // the state, so only the pose is interpolated here (the turret and
        // the hull's tilt are smoothed there).
        drawn_vehicles.clear();
        for v in &b.vehicles {
            let prev = a
                .vehicles
                .binary_search_by_key(&v.id, |p| p.id)
                .ok()
                .map(|k| &a.vehicles[k])
                .filter(|p| {
                    let (dx, dy) = (p.pos[0] - v.pos[0], p.pos[1] - v.pos[1]);
                    dx * dx + dy * dy < MAX_JUMP_M * MAX_JUMP_M
                })
                .unwrap_or(v);
            let pos = [
                lerp(prev.pos[0], v.pos[0], alpha),
                lerp(prev.pos[1], v.pos[1], alpha),
                lerp(prev.pos[2], v.pos[2], alpha),
            ];
            drawn_vehicles.push(DrawnVehicle {
                id: v.id,
                pos: sim_to_godot(pos, half),
                yaw: model_yaw(lerp_angle(prev.yaw, v.yaw, alpha)),
                pitch: lerp(prev.pitch, v.pitch, alpha),
                roll: lerp(prev.roll, v.roll, alpha),
                turret_yaw: lerp_angle(prev.turret_yaw, v.turret_yaw, alpha),
                gun_pitch: lerp(prev.gun_pitch, v.gun_pitch, alpha),
            });
        }

        // Rounds in flight, at the same render time as the soldiers.
        tracer_layer.begin();
        shell_layer.begin();
        let back = if paused {
            0.0
        } else {
            ((stamp_b - r) as f32).max(-period)
        };
        *back_s = back;
        for p in &b.rounds {
            if !p.visible {
                continue;
            }
            let vel = p.vel;
            let speed = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
            let flown_s = p.age - back;
            if speed < 1.0 || flown_s <= 0.0 {
                continue;
            }
            let pos = [
                p.pos[0] - vel[0] * back,
                p.pos[1] - vel[1] * back,
                p.pos[2] - vel[2] * back,
            ];
            let head = sim_to_godot(pos, half);
            let dir = [vel[0] / speed, vel[2] / speed, -vel[1] / speed];
            let (len, width, layer) = if p.he {
                (0.45, 0.09, &mut *shell_layer)
            } else {
                (
                    (speed * tracer_s).clamp(0.5, 15.0).min(speed * flown_s),
                    0.0,
                    &mut *tracer_layer,
                )
            };
            if len < 0.05 {
                continue;
            }
            let mid = [
                head[0] - dir[0] * len * 0.5,
                head[1] - dir[1] * len * 0.5,
                head[2] - dir[2] * len * 0.5,
            ];
            let d2 = dist2(mid);
            if d2 > draw_2 || !frustum.sphere_visible(mid, len * 0.5 + 0.5) {
                continue;
            }
            let width = if p.he {
                width
            } else {
                (d2.sqrt() * 0.0012).max(0.04)
            };
            let Some(slot) = layer.push(mid, len * 0.5 + width) else {
                continue;
            };
            write_streak_instance(slot, head, dir, len, width, [1.0, 0.0, 0.0, 0.0]);
        }

        // Thrown items: a small dark body (in the shell layer) and, while
        // in the air, a pale trail along the motion since the last frame.
        thrown_layer.begin();
        for t in b.thrown.iter().filter(|t| t.visible) {
            let prev = a
                .thrown
                .iter()
                .find(|p| p.id == t.id)
                .map_or(t.pos, |p| p.pos);
            let p = [
                lerp(prev[0], t.pos[0], alpha),
                lerp(prev[1], t.pos[1], alpha),
                lerp(prev[2], t.pos[2], alpha),
            ];
            let head = sim_to_godot(p, half);
            if dist2(head) > draw_2 || !frustum.sphere_visible(head, 3.0) {
                continue;
            }
            // Real size up close; never under about 2 px at RTS heights.
            let dist = dist2(head).sqrt();
            let size = (dist * 0.0045).max(0.13);
            let lift = if t.landed { size * 0.6 } else { 0.0 };
            let body = [head[0], head[1] + lift, head[2]];
            if let Some(slot) = shell_layer.push(body, size * 2.0) {
                write_streak_instance(slot, body, [1.0, 0.0, 0.0], size * 1.8, size, [0.0; 4]);
            }
            // Along the item's velocity (not the frame-to-frame motion: at
            // sim speeds above 1 the interpolation pair can be one frame).
            let d = sim_to_godot(t.vel, 0.0);
            let speed = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if t.landed || speed < 0.5 {
                continue;
            }
            let dir = [d[0] / speed, d[1] / speed, d[2] / speed];
            // About 0.15 s of flight, at least 1.5 m, so a lob reads at RTS
            // camera heights.
            let trail = (speed * 0.15).clamp(1.5, 6.0);
            if let Some(slot) = thrown_layer.push(head, trail) {
                write_streak_instance(slot, head, dir, trail, size * 0.8, [1.0, 0.0, 0.0, 0.0]);
            }
        }

        // Muzzle flashes for this frame's visible shots.
        let now = *clock;
        for ev in events_in.iter() {
            for s in ev.shots.iter().filter(|s| s.visible) {
                let class = setup.weapons.get(s.weapon as usize).map(|w| w.class);
                // Main guns and launchers flash in match/vehicle_fx.gd.
                if matches!(class, Some(WeaponClass::Cannon | WeaponClass::Launcher)) {
                    continue;
                }
                let he = class == Some(WeaponClass::Mortar);
                let v = s.vel;
                let sp = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
                let dir = [v[0] / sp, v[2] / sp, -v[1] / sp];
                let muzzle = sim_to_godot(s.from, half);
                let reach = if he { 0.9 } else { 0.55 };
                flashes.push(Flash {
                    head: [
                        muzzle[0] + dir[0] * reach,
                        muzzle[1] + dir[1] * reach,
                        muzzle[2] + dir[2] * reach,
                    ],
                    dir,
                    born: now,
                });
            }
        }
        flashes.retain(|f| now - f.born < FLASH_S as f64);
        flash_layer.begin();
        for f in flashes.iter() {
            let k = 1.0 - ((now - f.born) as f32 / FLASH_S);
            let len = 0.5 + 0.3 * k;
            let mid = [
                f.head[0] - f.dir[0] * len * 0.5,
                f.head[1] - f.dir[1] * len * 0.5,
                f.head[2] - f.dir[2] * len * 0.5,
            ];
            if !frustum.sphere_visible(mid, 1.0) || dist2(mid) > draw_2 {
                continue;
            }
            let Some(slot) = flash_layer.push(mid, 1.0) else {
                break;
            };
            write_streak_instance(
                slot,
                f.head,
                f.dir,
                len,
                0.12 + 0.12 * k,
                [1.0, 0.0, 0.0, 0.0],
            );
        }

        for l in soldier_layers.iter_mut() {
            l.commit(&mut rs);
        }
        soldier_casters.commit(&mut rs);
        tracer_layer.commit(&mut rs);
        shell_layer.commit(&mut rs);
        thrown_layer.commit(&mut rs);
        flash_layer.commit(&mut rs);

        // Dig cues: about once a second per digging squad the player sees.
        let digging: Vec<(u32, [f32; 3])> = b
            .squads
            .iter()
            .filter(|s| {
                s.visible
                    && s.order == OrderKind::Dig
                    && s.phase == NavPhase::Holding
                    && s.dig_pct < 100
            })
            .map(|s| (s.id, s.anchor))
            .collect();
        self.dig_cue
            .retain(|id, _| digging.iter().any(|d| d.0 == *id));
        for (id, pos) in digging {
            let t = self.dig_cue.entry(id).or_insert(hash01(id) * DIG_CUE_S);
            *t -= dt_anim;
            if *t <= 0.0 {
                *t += DIG_CUE_S;
                self.cues.push(EffectCue::new("dig", "", pos));
            }
        }
    }

    fn free(&mut self) {
        self.sim.stop();
        let mut rs = RenderingServer::singleton();
        for l in &mut self.soldier_layers {
            l.free(&mut rs);
        }
        self.soldier_casters.free(&mut rs);
        self.tracer_layer.free(&mut rs);
        self.shell_layer.free(&mut rs);
        self.thrown_layer.free(&mut rs);
        self.flash_layer.free(&mut rs);
        for c in self.chunks.iter_mut().flatten() {
            rs.free_rid(c.inst);
        }
        self.chunks.clear();
    }

    // ------------------------------------------------------------ state --

    fn state_dict(&mut self) -> VarDictionary {
        let armour_out = std::mem::take(&mut self.armour_out);
        let f = &self.hist[HIST - 1];
        let setup = &self.setup;
        let local = self.local as usize;
        let hz = self.hz;
        let rules = &setup.rules;
        let fx = |v: frontline_content::Fixed| Fx::from(v).to_f32();

        let local_faction = setup.players.get(local).map_or(0, |p| p.faction as usize);
        let mortar = setup.factions[local_faction]
            .roster
            .iter()
            .map(|&t| &setup.squad_types[t as usize])
            .find(|t| t.role == SquadRole::Mortar)
            .map(|t| &setup.weapons[t.weapon as usize]);

        let mut map = vdict! {
            "id" => short_id(&setup.map.id),
            "name" => setup.map.name.as_str(),
            "size_m" => setup.map.size_m as f32,
        };
        if let Some(t) = &self.minimap {
            map.set("minimap", &t.to_variant());
        }
        // Throw ranges: the local faction's item of each kind (the first
        // squad type in the roster that carries one).
        let mut throw_range = VarDictionary::new();
        for kind in ItemKind::ALL {
            let item = setup.factions[local_faction]
                .roster
                .iter()
                .find_map(|&t| setup.squad_types[t as usize].kit(&setup.items, kind));
            if let Some((_, it)) = item {
                throw_range.set(
                    item_name(kind),
                    Vector2::new(fx(it.range_min), fx(it.range_max)),
                );
            }
        }
        let profile = |d: &frontline_content::setup::DigSetup| {
            let mut out = vdict! {
                "max_m" => fx(d.max_length),
                "depth_m" => fx(d.depth),
                "width_m" => fx(d.floor_width) + 2.0 * fx(d.wall_run),
            };
            if d.engineers_only {
                out.set("engineers_only", true);
            }
            out
        };
        let rules_d = vdict! {
            "pop_cap" => rules.pop_cap as i64,
            "tickets_max" => rules.tickets as i64,
            "dig_max_m" => fx(rules.dig.max_length),
            "suppress_line_max_m" => rules.combat.suppress.line_max_m as f32,
            "mortar_min_m" => mortar.map_or(60.0, |w| fx(w.range_min)),
            "mortar_max_m" => mortar.map_or(600.0, |w| fx(w.range_max)),
            "capture_radius_m" => fx(rules.capture_radius),
            "throw_range_m" => &throw_range,
            "vehicles" => &vdict! {
                "unlock_s" => rules.vehicles.unlock_ticks as f32 / hz,
                "limit" => rules.vehicles.limit as i64,
            },
            "dig_profiles" => &vdict! {
                "trench" => &profile(&rules.dig),
                "at_ditch" => &profile(&rules.at_ditch),
            },
            "vehicle_types" => &self.vehicle_types,
        };

        let mut players = VarArray::new();
        for (p, ps) in f.players.iter().enumerate() {
            let ps_setup = &setup.players[p];
            let fac = short_id(&setup.factions[ps_setup.faction as usize].id);
            let name = match &self.net {
                Some(n) if !n.names[p % 2].is_empty() => n.names[p % 2].clone(),
                _ if p == local => "You".to_owned(),
                _ if !ps_setup.is_ai => "Player".to_owned(),
                _ => format!("AI ({:?})", ps_setup.difficulty).to_lowercase(),
            };
            players.push(
                &vdict! {
                    "id" => p as i64,
                    "faction" => fac,
                    "name" => name.as_str(),
                    "is_ai" => ps_setup.is_ai,
                    "tickets" => ps.tickets as f32,
                    "manpower" => ps.manpower as f32,
                    "income" => ps.income_per_min as f32 / 60.0,
                    "pop" => ps.pop as i64,
                    "points_held" => ps.points_held as i64,
                    "ticket_drain" => ps.drain_per_min as f32 / 60.0,
                    "vehicles" => ps.vehicles_counted as i64,
                    "vehicle_limit" => ps.vehicle_limit as i64,
                }
                .to_variant(),
            );
        }

        let mut points = VarArray::new();
        for (k, p) in f.points.iter().enumerate() {
            let name = setup.map.points.get(k).map_or("?", |p| p.name.as_str());
            points.push(
                &vdict! {
                    "id" => name,
                    "pos" => self.ground(p.pos),
                    "radius" => p.radius,
                    "value" => p.value as i64,
                    "owner" => p.owner.map_or(-1, |o| o as i64),
                    "capturing" => p.capturer.map_or(-1, |o| o as i64),
                    "progress" => p.progress_pct as f32 / 100.0,
                    "contested" => p.contested,
                }
                .to_variant(),
            );
        }

        // Soldiers the local player sees (own: all), ascending id; each
        // squad lists the ids it has among them.
        let mut soldiers = VarArray::new();
        let mut units: BTreeMap<u32, PackedInt32Array> = BTreeMap::new();
        let mut detached = f.units.iter().peekable();
        for (i, &id) in f.ids.iter().enumerate() {
            if !f.visible[i] {
                continue;
            }
            let owner = f.owner[i];
            let sq = f.squad[i];
            units.entry(sq).or_default().push(id as i32);
            let pos = match self.drawn.binary_search_by_key(&id, |d| d.0) {
                Ok(k) => self.drawn[k].1,
                Err(_) => sim_to_godot(
                    [f.soldiers[i][0], f.soldiers[i][1], f.soldiers[i][2]],
                    self.half,
                ),
            };
            let mut d = vdict! {
                "id" => id as i64,
                "squad" => sq as i64,
                "owner" => owner as i64,
                "slot" => f.slot[i] as i64,
                "pos" => Vector3::new(pos[0], pos[1], pos[2]),
                "hp" => f.hp[i] as f32,
                "stance" => match f.stance[i] {
                    0 => "stand",
                    1 => "crouch",
                    _ => "prone",
                },
                "pinned" => f.pinned[i],
                "ducked" => f.ducked[i],
                "suppression" => f.suppression[i] as f32 / 100.0,
                "in_trench" => f.in_trench[i],
            };
            if owner == self.local {
                d.set("stance_override", stance_order_name(f.stance_order[i]));
                d.set("detached", f.detached[i]);
                d.set("items", &items_dict(f.items[i].map(u16::from)));
                while detached.next_if(|u| u.id < id).is_some() {}
                if f.throwing[i] {
                    d.set("order", "throw");
                }
                if let Some(u) = detached.next_if(|u| u.id == id) {
                    let (order, target) = match u.order {
                        OrderKind::Move => ("move", true),
                        OrderKind::AttackMove => ("attack_move", true),
                        OrderKind::Retreat => ("retreat", true),
                        OrderKind::Dig => ("dig", true),
                        OrderKind::Follow => ("follow", false),
                        OrderKind::Suppress => ("suppress", true),
                        _ => ("hold", false),
                    };
                    if !f.throwing[i] {
                        d.set("order", order);
                    }
                    if target {
                        d.set("target", self.ground(u.target));
                    }
                    if let Some(v) = u.follow {
                        d.set("follow", v as i64);
                    }
                }
            }
            soldiers.push(&d.to_variant());
        }

        let mut squads = VarArray::new();
        for s in f.squads.iter().filter(|s| s.visible) {
            let mut d = self.squad_dict(s);
            d.set("units", &units.remove(&s.id).unwrap_or_default());
            if s.owner == self.local {
                d.set("items", &items_dict(s.items));
            }
            squads.push(&d.to_variant());
        }

        let mut smokes = VarArray::new();
        for c in f.smoke.iter().filter(|c| c.visible) {
            smokes.push(
                &vdict! {
                    "id" => c.id as i64,
                    "pos" => self.godot(c.pos),
                    "radius" => c.radius,
                    "density" => c.density,
                }
                .to_variant(),
            );
        }
        let mut thrown = VarArray::new();
        for t in f.thrown.iter().filter(|t| t.visible) {
            thrown.push(
                &vdict! {
                    "id" => t.id as i64,
                    "kind" => item_name(t.kind),
                    "owner" => t.owner as i64,
                    "pos" => self.godot(t.pos),
                    "landed" => t.landed,
                }
                .to_variant(),
            );
        }

        let fac = &setup.factions[local_faction];
        let me = f.players.get(local);
        // Vehicle tiles: locked until the unlock tick (S3).
        let locked_s = me.map_or(0.0, |p| p.unlock_tick.saturating_sub(f.tick) as f32 / hz);
        let entry = |t: u16| {
            let ty = &setup.squad_types[t as usize];
            let mut d = vdict! {
                "id" => ty.id.as_str(),
                "type" => role_name(ty.role),
                "name" => ty.name.as_str(),
                "men" => ty.men as i64,
                "pop" => ty.pop(&setup.vehicles) as i64,
            };
            if let Some(v) = ty.vehicle.and_then(|v| setup.vehicles.get(v as usize)) {
                d.set("vehicle", short_id(&v.id));
                d.set("short", v.short_name.as_str());
            }
            d
        };
        let mut catalog = VarArray::new();
        for &t in &fac.roster {
            let ty = &setup.squad_types[t as usize];
            let mut d = entry(t);
            d.set("cost", ty.cost as i64);
            // S3b: no queue and no delivery time; the re-call cooldown
            // left and its full length (the tile's fill).
            let left = me
                .and_then(|p| p.cooldowns.get(t as usize).copied())
                .unwrap_or(0);
            if left > 0 {
                d.set("cooldown_s", left as f32 / hz);
                d.set(
                    "cooldown_total_s",
                    (ty.recall_cooldown_ticks
                        .max(ty.initial_cooldown_ticks)
                        .max(left)) as f32
                        / hz,
                );
            }
            d.set("recall_s", ty.recall_cooldown_ticks as f32 / hz);
            if ty.vehicle.is_some() && locked_s > 0.0 {
                d.set("locked_s", locked_s);
            }
            catalog.push(&d.to_variant());
        }

        let mut events = VarArray::new();
        for e in self.ui_events.drain(..) {
            let mut d = vdict! {
                "kind" => e.kind,
                "text" => e.text.as_str(),
                "time_s" => e.tick as f32 / hz,
                "player" => e.player as i64,
            };
            if let Some(p) = e.pos {
                let h = self.terrain.height_at(p[0], p[1]);
                let g = sim_to_godot([p[0], p[1], h], self.half);
                d.set("pos", Vector3::new(g[0], g[1], g[2]));
            }
            events.push(&d.to_variant());
        }
        let mut effects = VarArray::new();
        for c in self.cues.drain(..) {
            let g = sim_to_godot(c.pos, self.half);
            let mut d = vdict! {
                "kind" => c.kind,
                "pos" => Vector3::new(g[0], g[1], g[2]),
            };
            match c.kind {
                "explosion" => d.set("size", c.weapon),
                "cannon" => d.set("round", c.weapon),
                "armour_hit" => d.set("result", c.weapon),
                _ if !c.weapon.is_empty() && c.kind == "shot" => d.set("weapon", c.weapon),
                _ => {}
            }
            if let Some(dir) = c.dir {
                let g = sim_to_godot(dir, 0.0);
                d.set("dir", Vector3::new(g[0], g[1], g[2]));
            }
            if let Some(v) = c.vehicle_type.and_then(|t| setup.vehicles.get(t as usize)) {
                d.set("type", short_id(&v.id));
            }
            effects.push(&d.to_variant());
        }
        let mut shells = VarArray::new();
        let back = self.back_s;
        for sh in &f.shells {
            let p = [
                sh.pos[0] - sh.vel[0] * back,
                sh.pos[1] - sh.vel[1] * back,
                sh.pos[2] - sh.vel[2] * back,
            ];
            let v = sim_to_godot(sh.vel, 0.0);
            shells.push(
                &vdict! {
                    "id" => sh.slot as i64,
                    "kind" => match sh.kind {
                        1 => "he",
                        2 => "heat",
                        _ => "ap",
                    },
                    "owner" => sh.player as i64,
                    "shooter" => sh.shooter as i64,
                    "pos" => self.godot(p),
                    "vel" => Vector3::new(v[0], v[1], v[2]),
                }
                .to_variant(),
            );
        }
        let mut armour_events = VarArray::new();
        for (tick, a) in &armour_out {
            armour_events.push(&self.armour_dict(*tick, a).to_variant());
        }

        let (status, winner, reason) = match f.outcome {
            Outcome::Running => ("running", -1i64, ""),
            Outcome::Won { winner } => {
                let loser = f
                    .players
                    .iter()
                    .enumerate()
                    .find(|(p, _)| *p != winner.0 as usize)
                    .and_then(|(_, ps)| ps.defeat);
                let reason = match loser {
                    Some(frontline_sim::Defeat::Tickets) => "tickets",
                    Some(frontline_sim::Defeat::Wiped) => "annihilation",
                    Some(frontline_sim::Defeat::Surrendered) => "surrender",
                    None => "tickets",
                };
                let st = if winner.0 as usize == local {
                    "won"
                } else {
                    "lost"
                };
                (st, winner.0 as i64, reason)
            }
            Outcome::Draw => ("lost", -1, "annihilation"),
        };
        let mut stats = VarArray::new();
        for (p, ps) in f.players.iter().enumerate() {
            let ex = f.extra.get(p).copied().unwrap_or_default();
            stats.push(
                &vdict! {
                    "kills" => ps.kills as i64,
                    "losses" => ps.losses as i64,
                    "points_held_s" => ps.hold_s as f32,
                    "squads_called" => ex.squads_called as i64,
                    "trench_m" => ex.trench_m,
                }
                .to_variant(),
            );
        }

        let paused = f.paused;
        let mut out = vdict! {
            "tick" => f.tick as i64,
            "time_s" => f.tick as f32 / hz,
            "paused" => paused,
            "local_player" => local as i64,
            "map" => &map,
            "rules" => &rules_d,
            "players" => &players,
            "points" => &points,
            "squads" => &squads,
            "soldiers" => &soldiers,
            "smokes" => &smokes,
            "thrown" => &thrown,
            "callin" => &vdict! {
                "catalog" => &catalog,
                "vehicles" => me.map_or(0, |p| p.vehicles_counted as i64),
                "vehicle_limit" => me.map_or(0, |p| p.vehicle_limit as i64),
            },
            "vehicles" => &self.vehicles_state(),
            "shells" => &shells,
            "armour_events" => &armour_events,
            "events" => &events,
            "effects" => &effects,
            "result" => &vdict! {
                "status" => status,
                "winner" => winner,
                "reason" => reason,
            },
            "stats" => &stats,
        };
        if let Some(sc) = &self.scene {
            out.set(
                "scene",
                &vdict! {
                    "name" => sc.name.as_str(),
                    "ready" => f.scene_ready,
                    "ready_tick" => f.scene_ready_at.map_or(-1, |t| t as i64),
                    "max_ticks" => sc.max_ticks as i64,
                },
            );
        }
        if self.net.is_some() {
            out.set("net", &self.net_dict());
            out.set("pending_orders", &self.pending_orders());
        }
        out
    }

    /// `vehicles` in `get_state()`: the newest frame's vehicles (already
    /// fog-filtered) at their drawn, interpolated poses.
    fn vehicles_state(&self) -> VarArray {
        let f = self.newest();
        let setup = &self.setup;
        let mut out = VarArray::new();
        for v in &f.vehicles {
            let Some(vt) = setup.vehicles.get(v.type_idx as usize) else {
                continue;
            };
            let drawn = self
                .drawn_vehicles
                .binary_search_by_key(&v.id, |d| d.id)
                .ok()
                .map(|k| self.drawn_vehicles[k])
                .unwrap_or_else(|| {
                    let p = sim_to_godot(v.pos, self.half);
                    DrawnVehicle {
                        id: v.id,
                        pos: p,
                        yaw: model_yaw(v.yaw),
                        pitch: v.pitch,
                        roll: v.roll,
                        turret_yaw: v.turret_yaw,
                        gun_pitch: v.gun_pitch,
                    }
                });
            let wreck = v.owner == WRECK_OWNER;
            let abandoned = v.flags & vehicle_flags::ABANDONED != 0;
            let own = !wreck && v.owner == self.local;
            let mut d = vdict! {
                "id" => v.id as i64,
                "squad" => if wreck { -1 } else { v.squad as i64 },
                "owner" => if wreck { -1 } else { v.owner as i64 },
                "type" => short_id(&vt.id),
                "name" => vt.name.as_str(),
                "pos" => Vector3::new(drawn.pos[0], drawn.pos[1], drawn.pos[2]),
                "yaw" => drawn.yaw,
                "pitch" => drawn.pitch,
                "roll" => drawn.roll,
                "turret_yaw" => drawn.turret_yaw,
                "gun_pitch" => drawn.gun_pitch,
                "speed" => v.speed,
                "flags" => v.flags as i64,
                "state" => vehicle_badge(v.flags, v.speed, own),
                // S3c: public on every vehicle.
                "abandoned" => abandoned,
            };
            if abandoned {
                // Its last owner, why, its crew squad, men still inside, and
                // the parts that are out (anyone can see an empty tank's
                // damage up close; the hover text names them).
                d.set("last_owner", v.last_owner as i64);
                d.set(
                    "abandon_cause",
                    ABANDON_KEYS
                        .get(v.abandon_cause as usize)
                        .copied()
                        .unwrap_or(""),
                );
                d.set(
                    "crew_squad",
                    if v.crew_squad == frontline_sim::NO_SQUAD {
                        -1
                    } else {
                        v.crew_squad as i64
                    },
                );
                d.set("inside", v.inside as i64);
                d.set("boarding", v.flags & vehicle_flags::BOARDING != 0);
                let out: PackedStringArray = self
                    .vehicle_parts_out(v.id)
                    .iter()
                    .map(|s| GString::from(*s))
                    .collect();
                d.set("parts_out", &out);
            }
            if own {
                let crew: PackedStringArray = (0..5)
                    .map(|k| {
                        GString::from(match v.seats[k] {
                            x if x == SeatSnap::Ok as u8 => "ok",
                            x if x == SeatSnap::Wounded as u8 => "wounded",
                            x if x == SeatSnap::Empty as u8 => "dead",
                            x if x == SeatSnap::Swapping as u8 => "swapping",
                            _ => "none",
                        })
                    })
                    .collect();
                let mut comps = VarDictionary::new();
                for (k, name) in COMPONENT_KEYS.iter().enumerate() {
                    comps.set(*name, v.components[k] as f32 / 100.0);
                }
                d.set("crew", &crew);
                d.set("components", &comps);
                d.set(
                    "repairing",
                    COMPONENT_KEYS
                        .get(v.repairing as usize)
                        .copied()
                        .unwrap_or(""),
                );
                // S3b: "apcr" only for a gun that carries a special round.
                let mut ammo = vdict! {"ap" => v.ammo[0] as i64, "he" => v.ammo[1] as i64};
                let special = self
                    .setup
                    .vehicles
                    .get(v.type_idx as usize)
                    .is_some_and(|t| t.main_gun().ammo.len() > 2);
                if special {
                    ammo.set("apcr", v.ammo[2] as i64);
                }
                // S3b Fix: MG rounds left in stowage (mounts with a limit).
                if v.mg_rounds != u32::MAX {
                    ammo.set("mg", v.mg_rounds as i64);
                }
                d.set("ammo", &ammo);
                d.set(
                    "loaded",
                    match v.loaded {
                        1 => "ap",
                        2 => "he",
                        3 => "apcr",
                        _ => "",
                    },
                );
                d.set("ammo_choice", ammo_choice_name(v.ammo_choice));
                d.set("reload", v.reload_pct as f32 / 100.0);
                d.set(
                    "target",
                    if v.target == u32::MAX {
                        -1
                    } else {
                        v.target as i64
                    },
                );
                d.set("followers", v.followers as i64);
                if let Some(l) = v.suppress {
                    let line: PackedVector3Array = l.iter().map(|p| self.ground(*p)).collect();
                    d.set("suppress_line", &line);
                }
            }
            out.push(&d.to_variant());
        }
        out
    }

    /// S3c: the destroyed components of vehicle `id` in the newest frame
    /// (`parts_out` of an abandoned vehicle).
    fn vehicle_parts_out(&self, id: u32) -> Vec<&'static str> {
        let f = self.newest();
        let Ok(k) = f.vehicles.binary_search_by_key(&id, |v| v.id) else {
            return Vec::new();
        };
        let v = &f.vehicles[k];
        COMPONENT_KEYS
            .iter()
            .enumerate()
            .filter(|(c, _)| v.components.get(*c).is_some_and(|&hp| hp == 0))
            .map(|(_, n)| *n)
            .collect()
    }

    /// One `armour_events` entry (`godot/docs/match_state.md`).
    fn armour_dict(&self, tick: u64, a: &ArmourView) -> VarDictionary {
        let e = &a.ev;
        let kind = ARMOUR_KINDS.get(e.kind as usize).copied().unwrap_or("");
        let component = match e.kind {
            k if k == ArmourEventKind::Fired as u8 => match e.component {
                2 => "he",
                3 => "apcr",
                _ => "ap",
            },
            k if k == ArmourEventKind::ComponentHit as u8
                || k == ArmourEventKind::Disabled as u8 =>
            {
                COMPONENT_KEYS
                    .get(e.component as usize)
                    .copied()
                    .unwrap_or("")
            }
            k if k == ArmourEventKind::CrewHit as u8 => SeatRole::ALL
                .get(e.component as usize)
                .map_or("", |r| SEAT_KEYS[r.index()]),
            k if k == ArmourEventKind::KnockedOut as u8 => match e.component {
                1 => "ammo_rack",
                2 => "fire",
                3 => "ammo_fire",
                _ => "crew",
            },
            // S3c.
            k if k == ArmourEventKind::Abandoned as u8 => ABANDON_KEYS
                .get(e.component as usize)
                .copied()
                .unwrap_or(""),
            k if k == ArmourEventKind::CrewOut as u8 => SeatRole::ALL
                .get(e.component as usize)
                .map_or("", |r| SEAT_KEYS[r.index()]),
            k if k == ArmourEventKind::Recrewed as u8 => {
                if e.component == frontline_sim::bail::RECREW_CAPTURED {
                    "captured"
                } else {
                    "own"
                }
            }
            _ => "",
        };
        let zone = if e.zone == NONE_U8 {
            ""
        } else {
            ArmourZone::ALL
                .get(e.zone as usize)
                .map_or("", |z| ZONE_KEYS[z.index()])
        };
        vdict! {
            "kind" => kind,
            "vehicle" => e.vehicle as i64,
            "type" => self
                .setup
                .vehicles
                .get(a.type_idx as usize)
                .map_or("", |v| short_id(&v.id)),
            "owner" => e.owner as i64,
            "attacker" => if e.attacker == NONE_U8 { -1 } else { e.attacker as i64 },
            "component" => component,
            "zone" => zone,
            "ammo" => if e.ammo == u16::MAX { "" } else { ammo_word(&self.setup, e.ammo) },
            "pos" => self.godot(e.pos),
            "seq" => e.seq as i64,
            "time_s" => tick as f32 / self.hz,
        }
    }

    /// `net` in `get_state()` (`godot/docs/match_state.md`, "Net mode").
    fn net_dict(&self) -> VarDictionary {
        let c = &self.sim.counters;
        let names = self
            .net
            .as_ref()
            .map(|n| n.names.clone())
            .unwrap_or_default();
        let name = |s: u8| names.get(s as usize).cloned().unwrap_or_default();
        let state = NetState::from_u8(c.net_state.load(Ordering::Relaxed));
        let detail = c.net.lock().map(|d| d.clone()).unwrap_or_default();
        let d = c.input_delay.load(Ordering::Relaxed);
        let rtt = c.rtt_ms.load(Ordering::Relaxed);
        let mut out = vdict! {
            "ping_ms" => if rtt < 0 { -1.0 } else { rtt as f32 },
            "input_delay" => d as i64,
            "delay_ms" => (d as f32 * 1000.0 / self.hz).round() as i64,
            "playout" => c.playout.load(Ordering::Relaxed) as i64,
            "restamps" => c.restamps.load(Ordering::Relaxed) as i64,
            "stalls" => c.stall_ticks.load(Ordering::Relaxed) as i64,
            "state" => state.name(),
            "paused_by" => detail.paused_by.map(name).unwrap_or_default().as_str(),
            "waiting_for" => detail.waiting_for.map(name).unwrap_or_default().as_str(),
            "waiting_s_left" => detail.waiting_s_left,
            "desync_tick" => c.desync_tick.load(Ordering::Relaxed),
            "desync_dump" => detail.desync_dump.clone().unwrap_or_default().as_str(),
            "disconnect_reason" => detail.disconnect.unwrap_or(""),
        };
        if let Some(p) = detail.paused_by {
            out.set("paused_slot", p as i64);
        }
        if let Some(w) = detail.waiting_for {
            out.set("waiting_slot", w as i64);
        }
        if let Some(e) = &detail.dump_error {
            out.set("desync_dump_error", e.as_str());
        }
        out
    }

    /// Own positional orders sent but not run yet (oldest first).
    fn pending_orders(&mut self) -> VarArray {
        let done = self.sim.counters.own_executed.load(Ordering::Relaxed);
        let mut out = VarArray::new();
        if let Some(n) = self.net.as_mut() {
            n.pending.retain(|p| p.0 >= done);
            for (_, kind, pos) in &n.pending {
                out.push(
                    &vdict! {
                        "pos" => *pos,
                        "kind" => *kind,
                    }
                    .to_variant(),
                );
            }
        }
        out
    }

    fn squad_dict(&self, s: &SquadView) -> VarDictionary {
        let setup = &self.setup;
        let ty = &setup.squad_types[s.type_idx as usize];
        let men = s.men.max(1) as u32;
        let supp = s.suppression_sum as f32 / (100.0 * men as f32);
        let status = if s.order == OrderKind::Retreat {
            "retreating"
        } else if s.pinned as u32 * 2 >= men {
            "pinned"
        } else if s.ducked as u32 * 2 >= men {
            // S3b Fix: half the squad ducked below the lip is not
            // returning fire.
            "heads_down"
        } else if supp * 100.0 >= setup.rules.combat.crouch_suppression as f32 {
            "suppressed"
        } else {
            "ok"
        };
        let order = if s.setup {
            "setup"
        } else {
            match s.order {
                OrderKind::Idle => "idle",
                OrderKind::Move => "move",
                OrderKind::AttackMove => "attack_move",
                OrderKind::Retreat => "retreat",
                OrderKind::Dig => "dig",
                OrderKind::Barrage => "barrage",
                OrderKind::Follow => "follow",
                OrderKind::Suppress => "suppress",
                OrderKind::Recrew => "recrew",
            }
        };
        let pos = match self.anchors.get(&s.id) {
            Some(g) => Vector3::new(g[0], g[1], g[2]),
            None => self.godot(s.anchor),
        };
        let hp_max = (ty.hp.max(1) * ty.men.max(1) as u32) as f32;
        let mut d = vdict! {
            "id" => s.id as i64,
            "owner" => s.owner as i64,
            "type" => role_name(ty.role),
            "name" => ty.name.as_str(),
            "men" => s.men as i64,
            "men_max" => ty.men as i64,
            "health" => (s.hp_sum as f32 / hp_max).clamp(0.0, 1.0),
            "suppression" => supp.clamp(0.0, 1.0),
            "status" => status,
            "order" => order,
            "pos" => pos,
        };
        if let Some(t) = s.target {
            d.set("target", self.ground(t));
        }
        if s.owner == self.local && !s.dig_line.is_empty() {
            let line: PackedVector3Array = s.dig_line.iter().map(|p| self.ground(*p)).collect();
            d.set("dig_line", &line);
            d.set("dig_progress", s.dig_pct as f32 / 100.0);
        }
        if s.order == OrderKind::Barrage {
            d.set("rounds", s.rounds as i64);
        }
        if let Some(v) = s.follow {
            // S3c: a `Recrew` order shares the field (the vehicle it walks
            // to); `recrew` says which.
            d.set(
                if s.order == OrderKind::Recrew {
                    "recrew"
                } else {
                    "follow"
                },
                v as i64,
            );
        }
        if let Some(l) = s.suppress_line {
            let line: PackedVector3Array = l.iter().map(|p| self.ground(*p)).collect();
            d.set("suppress_line", &line);
        }
        // S3c: a crew squad's abandoned vehicle (the card's "Crew —
        // Panzer IV" and its jump button).
        if let Some(v) = self
            .newest()
            .vehicles
            .iter()
            .find(|v| v.crew_squad == s.id && v.flags & vehicle_flags::ABANDONED != 0)
        {
            d.set("crew_of", v.id as i64);
            d.set("crew_of_name", vehicle_short(&self.setup, v.type_idx));
        }
        d
    }

    fn map_info(&self) -> VarDictionary {
        let fx = |v: frontline_content::Fixed| Fx::from(v).to_f32();
        let m = &self.setup.map;
        let mut woods = VarArray::new();
        for w in &m.woods {
            let c = frontline_sim::FxVec2::from(w.center).to_f32();
            woods.push(&vdict! {"center" => self.ground(c), "radius" => fx(w.radius)}.to_variant());
        }
        let mut points = VarArray::new();
        for p in &m.points {
            let c = frontline_sim::FxVec2::from(p.pos).to_f32();
            points.push(
                &vdict! {"id" => p.name.as_str(), "pos" => self.ground(c), "value" => p.value as i64}
                    .to_variant(),
            );
        }
        let mut bases = VarArray::new();
        for b in &m.bases {
            let c = frontline_sim::FxVec2::from(b.center).to_f32();
            bases.push(&vdict! {"pos" => self.ground(c), "radius" => fx(b.radius)}.to_variant());
        }
        let own_base = self
            .setup
            .players
            .get(self.local as usize)
            .map_or(0, |p| p.base as usize);
        let mut factions = VarArray::new();
        for p in &self.setup.players {
            let f = &self.setup.factions[p.faction as usize];
            factions.push(
                &vdict! {
                    "id" => short_id(&f.id),
                    "name" => f.name.as_str(),
                    "color" => Color::from_rgb(
                        ((f.color_rgb >> 16) & 0xff) as f32 / 255.0,
                        ((f.color_rgb >> 8) & 0xff) as f32 / 255.0,
                        (f.color_rgb & 0xff) as f32 / 255.0,
                    ),
                }
                .to_variant(),
            );
        }
        let mut out = vdict! {
            "id" => short_id(&m.id),
            "name" => m.name.as_str(),
            "size_m" => m.size_m as f32,
            "half" => self.half,
            "woods" => &woods,
            "points" => &points,
            "bases" => &bases,
            "own_base" => own_base as i64,
            "factions" => &factions,
            "local_player" => self.local as i64,
            "tick_hz" => self.hz,
        };
        if let Some(sc) = &self.scene {
            // A staged scene: what the camera should look at.
            out.set("scene", sc.name.as_str());
            out.set("scene_focus", self.ground(sc.focus));
        }
        out
    }

    // ---------------------------------------------------------- intents --

    fn handle_intent(&mut self, kind: &str, data: &VarDictionary) {
        let ids: Vec<u32> = data
            .get("squads")
            .and_then(|v| v.try_to::<PackedInt32Array>().ok())
            .map(|a| a.as_slice().iter().map(|&i| i as u32).collect())
            .unwrap_or_default();
        let target = data.get("target").and_then(|v| v.try_to::<Vector3>().ok());
        let to_sim = |v: Vector3, half: f32| [v.x + half, -v.z + half];
        // Own squads only, as the newest frame knows them.
        let own: Vec<&SquadView> = {
            let f = self.newest();
            ids.iter()
                .filter_map(|id| {
                    f.squads
                        .iter()
                        .find(|s| s.id == *id && s.owner == self.local)
                })
                .collect()
        };
        let mut out: Vec<CommandKind> = Vec::new();
        match kind {
            "move" | "attack_move" => {
                let Some(t) = target else { return };
                let anchors: Vec<[f32; 2]> =
                    own.iter().map(|s| [s.anchor[0], s.anchor[1]]).collect();
                let goals = spread_targets(&anchors, to_sim(t, self.half), GROUP_SPACING_M);
                for (s, g) in own.iter().zip(goals) {
                    let squad = SquadId(s.id);
                    let target = sim_to_fx(g);
                    out.push(if kind == "move" {
                        CommandKind::Move { squad, target }
                    } else {
                        CommandKind::AttackMove { squad, target }
                    });
                }
            }
            "stop" => out.extend(own.iter().map(|s| CommandKind::Stop {
                squad: SquadId(s.id),
            })),
            "retreat" => out.extend(own.iter().map(|s| CommandKind::Retreat {
                squad: SquadId(s.id),
            })),
            "dig" => {
                let Some(line) = data
                    .get("polyline")
                    .and_then(|v| v.try_to::<PackedVector3Array>().ok())
                else {
                    return;
                };
                let line: Vec<[f32; 2]> = line
                    .as_slice()
                    .iter()
                    .map(|v| to_sim(*v, self.half))
                    .collect();
                let diggers: Vec<&&SquadView> = own
                    .iter()
                    .filter(|s| self.setup.squad_types[s.type_idx as usize].dig_rate_pct > 0)
                    .collect();
                let profile = dig_profile(
                    &data
                        .get("profile")
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                );
                let pieces = split_polyline(&line, diggers.len(), MIN_DIG_PIECE_M);
                for (k, s) in diggers.iter().enumerate() {
                    // More squads than pieces: the extras share the last.
                    let piece = &pieces[k.min(pieces.len() - 1)];
                    out.push(dig_command(s.id, piece, profile));
                }
            }
            "barrage" => {
                let Some(t) = target else { return };
                let rounds = self.setup.rules.ai.barrage_rounds.max(1);
                for s in own.iter().filter(|s| self.role(s.type_idx) == "mortar") {
                    out.push(CommandKind::Barrage {
                        squad: SquadId(s.id),
                        target: sim_to_fx(to_sim(t, self.half)),
                        rounds,
                    });
                }
            }
            "call_in" => {
                // The catalog `id` (S3) wins over the role.
                let id = data.get("id").map(|v| v.to_string()).unwrap_or_default();
                let ty = data.get("type").map(|v| v.to_string()).unwrap_or_default();
                let found = call_in_by_id(&self.setup, self.local, &id)
                    .or_else(|| call_in_type(&self.setup, self.local, &ty));
                match found {
                    Some(t) => out.push(CommandKind::CallIn { squad_type: t }),
                    None => {
                        godot_warn!("FrontlineMatch: no {ty:?} squad in the roster");
                    }
                }
            }
            "regroup" => out.extend(own.iter().map(|s| CommandKind::Regroup {
                squad: SquadId(s.id),
            })),
            // S3c: the selected infantry squads re-crew abandoned vehicle
            // `vehicle` (the sim refuses squads that cannot; the feed says
            // why).
            "recrew" => {
                let Some(v) = data
                    .get("vehicle")
                    .and_then(|v| v.try_to::<i64>().ok())
                    .filter(|&i| i >= 0)
                else {
                    return;
                };
                out.extend(
                    own.iter()
                        .filter(|s| {
                            self.setup.squad_types[s.type_idx as usize]
                                .vehicle
                                .is_none()
                        })
                        .map(|s| CommandKind::Recrew {
                            squad: SquadId(s.id),
                            vehicle: frontline_sim::SimId(v as u32),
                        }),
                );
            }
            // S3c: the crews of the selected own vehicles (`units`) bail out.
            "abandon" => {
                let raw = data
                    .get("units")
                    .and_then(|v| v.try_to::<PackedInt32Array>().ok())
                    .unwrap_or_default();
                let f = self.newest();
                for &id in raw.as_slice() {
                    if let Ok(k) = f.vehicles.binary_search_by_key(&(id as u32), |v| v.id)
                        && f.vehicles[k].owner == self.local
                    {
                        out.push(CommandKind::Abandon {
                            squad: SquadId(f.vehicles[k].squad),
                        });
                    }
                }
            }
            "move_units" | "attack_move_units" | "stop_units" | "retreat_units" | "dig_units"
            | "set_stance" | "throw" | "throw_each" | "reverse_units" | "face_units"
            | "attack_unit" | "set_ammo" | "repair_units" | "follow_units" | "suppress_units" => {
                let raw = data
                    .get("units")
                    .and_then(|v| v.try_to::<PackedInt32Array>().ok())
                    .unwrap_or_default();
                // The local player's living men and vehicles, as the newest
                // frame knows them.
                let f = self.newest();
                let units = unit_set(raw.as_slice(), |id| {
                    f.ids
                        .binary_search(&id)
                        .is_ok_and(|i| f.owner[i] == self.local)
                        || f.vehicles
                            .binary_search_by_key(&id, |v| v.id)
                            .is_ok_and(|k| f.vehicles[k].owner == self.local)
                });
                if units.is_empty() {
                    return;
                }
                let t = target.map(|t| to_sim(t, self.half));
                if matches!(
                    kind,
                    "reverse_units"
                        | "face_units"
                        | "attack_unit"
                        | "set_ammo"
                        | "repair_units"
                        | "follow_units"
                ) {
                    // `attack_unit` names its target vehicle in `target`,
                    // `follow_units` in `vehicle`.
                    let id = data
                        .get(if kind == "follow_units" {
                            "vehicle"
                        } else {
                            "target"
                        })
                        .and_then(|v| v.try_to::<i64>().ok())
                        .filter(|&i| i >= 0)
                        .map(|i| i as u32);
                    let word = data.get("ammo").map(|v| v.to_string()).unwrap_or_default();
                    match vehicle_command(kind, units, t, id, &word) {
                        Some(c) => out.push(c),
                        None => {
                            godot_warn!("FrontlineMatch: incomplete {kind} intent");
                        }
                    }
                    let mut last = None;
                    for c in out {
                        last = self.sim.send(c).or(last);
                    }
                    self.mark_pending(kind, last, target, data);
                    return;
                }
                let line: Vec<[f32; 2]> = data
                    .get("polyline")
                    .and_then(|v| v.try_to::<PackedVector3Array>().ok())
                    .map(|l| l.as_slice().iter().map(|v| to_sim(*v, self.half)).collect())
                    .unwrap_or_default();
                let word = data
                    .get(match kind {
                        "set_stance" => "stance",
                        "dig_units" => "profile",
                        _ => "item",
                    })
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                match unit_command(kind, units, t, &line, &word) {
                    Some(c) => out.push(c),
                    None => {
                        godot_warn!("FrontlineMatch: incomplete {kind} intent");
                    }
                }
            }
            "surrender" => out.push(CommandKind::Surrender),
            "pause" => {
                let p = data
                    .get("paused")
                    .and_then(|v| v.try_to::<bool>().ok())
                    .unwrap_or(false);
                self.user_paused = p;
                // Net: asks the relay to pause or resume both players.
                if self.warmup == 0 {
                    self.sim.set_paused(p);
                }
            }
            _ => {}
        }
        let mut last = None;
        for c in out {
            last = self.sim.send(c).or(last);
        }
        self.mark_pending(kind, last, target, data);
    }

    /// Net: a marker at the clicked spot until the intent's last command
    /// (send ordinal `last`) ran.
    fn mark_pending(
        &mut self,
        kind: &str,
        last: Option<u64>,
        target: Option<Vector3>,
        data: &VarDictionary,
    ) {
        if let (Some(n), Some(ord), Some(pk)) = (self.net.as_mut(), last, pending_kind(kind)) {
            let pos = target.or_else(|| {
                data.get("polyline")
                    .and_then(|v| v.try_to::<PackedVector3Array>().ok())
                    .and_then(|l| l.as_slice().get(l.len() / 2).copied())
            });
            if let Some(pos) = pos {
                if n.pending.len() == MAX_PENDING_MARKERS {
                    n.pending.pop_front();
                }
                n.pending.push_back((ord, pk, pos));
            }
        }
    }
}

/// `{frag, smoke, satchel}` counts.
fn items_dict(n: [u16; frontline_sim::ITEM_KINDS]) -> VarDictionary {
    let mut d = VarDictionary::new();
    for kind in ItemKind::ALL {
        d.set(item_name(kind), n[kind.index()] as i64);
    }
    d
}

/// Shaded top-down map image (north up), woods and contour lines, for the
/// minimap and menu preview.
fn minimap_texture(t: &TerrainStore, setup: &MatchSetup, px: usize) -> Option<Gd<ImageTexture>> {
    let size = t.size_m();
    let step = size / px as f32;
    let light = meshgen::normalize([-0.5, -0.45, 0.75]);
    let woods: Vec<([f32; 2], f32)> = setup
        .map
        .woods
        .iter()
        .map(|w| {
            (
                frontline_sim::FxVec2::from(w.center).to_f32(),
                Fx::from(w.radius).to_f32(),
            )
        })
        .collect();
    let mut bytes = Vec::with_capacity(px * px * 3);
    for j in 0..px {
        for i in 0..px {
            // Row 0 is north (top of the image).
            let x = (i as f32 + 0.5) * step;
            let y = size - (j as f32 + 0.5) * step;
            let h = t.base_at(x, y);
            let hx = t.base_at(x + step, y) - h;
            let hy = t.base_at(x, y + step) - h;
            let n = meshgen::normalize([-hx, -hy, step]);
            let shade = (meshgen::dot(n, light) * 1.25).clamp(0.35, 1.15);
            let k = ((h - 10.0) / 30.0).clamp(0.0, 1.0);
            let mut c = [0.36 + 0.19 * k, 0.42 + 0.10 * k, 0.26 + 0.10 * k];
            if woods
                .iter()
                .any(|(w, r)| (x - w[0]).powi(2) + (y - w[1]).powi(2) < r * r)
            {
                c = [0.20, 0.27, 0.17];
            }
            if h.rem_euclid(4.0) < 0.3 {
                c = [c[0] * 0.82, c[1] * 0.82, c[2] * 0.82];
            }
            for v in c {
                bytes.push(((v * shade).clamp(0.0, 1.0) * 255.0) as u8);
            }
        }
    }
    let data = PackedByteArray::from(bytes.as_slice());
    let image = Image::create_from_data(px as i32, px as i32, false, ImageFormat::RGB8, &data)?;
    ImageTexture::create_from_image(&image)
}
