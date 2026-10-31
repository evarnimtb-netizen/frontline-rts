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
//! GDScript from the state's `effects` and `get_map_info()`. Vehicles are
//! out of S1: this node draws none (the bench node keeps its placeholders).
//!
//! The node must sit at the world origin with an identity transform.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use frontline_content::{MatchSetup, SquadRole, WeaponClass};
use frontline_sim::game::MAX_CALL_IN_QUEUE;
use frontline_sim::{CommandKind, Fx, NavPhase, OrderKind, Outcome, SquadId, World};
use godot::classes::base_material_3d::{Flags as MatFlags, ShadingMode};
use godot::classes::image::Format as ImageFormat;
use godot::classes::light_3d::Param as LightParam;
use godot::classes::rendering_server::ShadowCastingSetting;
use godot::classes::{
    ArrayMesh, Camera3D, DirectionalLight3D, Engine, INode3D, Image, ImageTexture, Node3D,
    ProjectSettings, RenderingServer, Shader, ShaderMaterial, StandardMaterial3D,
};
use godot::prelude::*;

use crate::gfx::{Layer, build_mesh, soldier_material};
use crate::intents::{
    GROUP_SPACING_M, MIN_DIG_PIECE_M, call_in_type, dig_command, role_name, sim_to_fx,
    split_polyline, spread_targets,
};
use crate::match_events::{EffectCue, EventTracker, UiEvent, effect_cues};
use crate::match_frame::{MatchFrame, SquadView, TickEvents};
use crate::match_setup::{MenuChoice, find_mods_dir, mods_candidates, resolve, short_id};
use crate::match_thread::MatchHandle;
use crate::meshgen;
use crate::present::{
    CasterCull, Frustum, carry_over, hash01, interp_pair, lerp, lerp_angle, model_yaw,
    sim_to_godot, write_streak_instance, write_yaw_instance,
};
use crate::rig;
use crate::terrain_view::TerrainStore;

const SOLDIER_SHADER: &str = "res://shaders/soldier_skinned.gdshader";
const TRACER_SHADER: &str = "res://shaders/tracer.gdshader";
/// Planar jump (m) between two snapshots treated as a new entity.
const MAX_JUMP_M: f32 = 10.0;
/// Frames kept for interpolation (oldest first).
const HIST: usize = 3;
/// Seconds a muzzle flash shows.
const FLASH_S: f32 = 0.06;
/// Seconds between `dig` sound cues of one digging squad.
const DIG_CUE_S: f32 = 1.1;

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
    /// Player slot of this client (set by `start_match`).
    #[var]
    local_player: i32,
    rt: Option<MatchRuntime>,
    last_error: GString,
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
            local_player: 0,
            rt: None,
            last_error: GString::new(),
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
            rt.delay = rt.period as f64 + margin_s;
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
        let cam = self.base().get_viewport().and_then(|vp| vp.get_camera_3d());
        match self.rt.as_mut() {
            Some(rt) => rt.state_dict(cam.as_ref()),
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
            "sim_late_ticks" => c.late_ticks.load(Relaxed) as i64,
            "commands_sent" => c.commands_sent.load(Relaxed) as i64,
            "soldiers" => f.soldiers.len() as i64,
            "corpses" => f.corpses.len() as i64,
            "rounds" => f.rounds.len() as i64,
            "soldiers_drawn" => rt.soldier_layers.iter().map(|l| l.count as i64).sum::<i64>(),
            "tracers_drawn" => rt.tracer_layer.count as i64,
            "detail_tiles" => rt.terrain.tile_count() as i64,
            "chunks_pending" => rt.dirty_chunks.len() as i64,
            "chunk_rebuilds" => rt.chunk_rebuilds as i64,
            "presenter_us" => rt.present_us,
            "speed" => rt.sim.speed(),
        }
    }
}

impl FrontlineMatch {
    fn try_start(&mut self, d: &VarDictionary) -> Result<(), String> {
        let s = |k: &str, def: &str| {
            d.get(k)
                .map(|v| v.to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| def.to_owned())
        };
        let n = |k: &str| d.get(k).and_then(|v| v.try_to::<i64>().ok());
        let choice = MenuChoice {
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
        };
        let explicit = self.mods_dir.to_string();
        let project = ProjectSettings::singleton()
            .globalize_path("res://")
            .to_string();
        let candidates = mods_candidates(
            (!explicit.is_empty()).then(|| std::path::Path::new(&explicit)),
            Some(std::path::Path::new(&project)),
        );
        let mods = find_mods_dir(&candidates)
            .ok_or_else(|| format!("cannot find mods/ww2 (looked in {candidates:?})"))?;
        let setup = resolve(&mods, &choice).map_err(|e| format!("{}: {e}", mods.display()))?;
        let world = World::new_match(setup, self.sim_threads.max(1) as usize)
            .map_err(|e| format!("invalid match setup: {e}"))?;
        let world3d = self
            .base()
            .get_world_3d()
            .ok_or("node is not in a 3D world")?;
        let soldier_sh =
            shader_or_load(&self.soldier_shader, SOLDIER_SHADER).ok_or("missing soldier shader")?;
        let tracer_sh =
            shader_or_load(&self.tracer_shader, TRACER_SHADER).ok_or("missing tracer shader")?;
        let local = 0u8;
        let rt = MatchRuntime::new(world, local, world3d.get_scenario(), soldier_sh, tracer_sh)
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
    terrain: TerrainStore,
    scenario: Rid,
    terrain_mat: Gd<StandardMaterial3D>,
    chunks: Vec<Option<Chunk>>,
    dirty_chunks: BTreeSet<(i32, i32)>,
    chunk_rebuilds: u64,
    soldier_layers: [Layer; 3],
    soldier_casters: Layer,
    tracer_layer: Layer,
    shell_layer: Layer,
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
    _resources: Vec<Gd<Resource>>,
}

impl MatchRuntime {
    fn new(
        world: World,
        local: u8,
        scenario: Rid,
        soldier_sh: Gd<Shader>,
        tracer_sh: Gd<Shader>,
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

        let mut terrain_mat = StandardMaterial3D::new_gd();
        terrain_mat.set_flag(MatFlags::ALBEDO_FROM_VERTEX_COLOR, true);
        terrain_mat.set_roughness(1.0);

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
        let mut sim = MatchHandle::spawn(world, local)?;
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
            let mesh = build_mesh(&md, Some(self.terrain_mat.clone().upcast()));
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
            soldier_layers,
            soldier_casters,
            tracer_layer,
            shell_layer,
            flash_layer,
            flashes,
            team_tint,
            events_in,
            setup,
            half,
            period,
            hz,
            clock,
            ..
        } = self;
        let (half, period, hz) = (*half, *period, *hz);
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
            if !b.visible[i] {
                continue;
            }
            let stance = b.stance[i].min(2);
            let spd = if stance == 2 { 0.0 } else { b.speeds[i] };
            let ph = &mut phases[id as usize];
            ph[0] = (ph[0] + dt_anim * spd / rig::STRIDE_M).fract();
            ph[1] = (ph[1] + dt_anim * 0.25).fract();
            let walk = ((spd - 0.25) / 0.75).clamp(0.0, 1.0);
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

        // Rounds in flight, at the same render time as the soldiers.
        tracer_layer.begin();
        shell_layer.begin();
        let back = if paused {
            0.0
        } else {
            ((stamp_b - r) as f32).max(-period)
        };
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

        // Muzzle flashes for this frame's visible shots.
        let now = *clock;
        for ev in events_in.iter() {
            for s in ev.shots.iter().filter(|s| s.visible) {
                let he = setup
                    .weapons
                    .get(s.weapon as usize)
                    .is_some_and(|w| w.class == WeaponClass::Mortar);
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
                self.cues.push(EffectCue {
                    kind: "dig",
                    weapon: "",
                    pos,
                });
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
        self.flash_layer.free(&mut rs);
        for c in self.chunks.iter_mut().flatten() {
            rs.free_rid(c.inst);
        }
        self.chunks.clear();
    }

    // ------------------------------------------------------------ state --

    fn state_dict(&mut self, cam: Option<&Gd<Camera3D>>) -> VarDictionary {
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
        let rules_d = vdict! {
            "pop_cap" => rules.pop_cap as i64,
            "tickets_max" => rules.tickets as i64,
            "dig_max_m" => fx(rules.dig.max_length),
            "mortar_min_m" => mortar.map_or(60.0, |w| fx(w.range_min)),
            "mortar_max_m" => mortar.map_or(600.0, |w| fx(w.range_max)),
            "capture_radius_m" => fx(rules.capture_radius),
        };

        let mut players = VarArray::new();
        for (p, ps) in f.players.iter().enumerate() {
            let ps_setup = &setup.players[p];
            let queued: u32 = ps
                .call_ins
                .iter()
                .map(|c| setup.squad_types[c.0 as usize].men as u32)
                .sum();
            let fac = short_id(&setup.factions[ps_setup.faction as usize].id);
            let name = if p == local {
                "You".to_owned()
            } else {
                format!("AI ({:?})", ps_setup.difficulty).to_lowercase()
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
                    "pop" => ps.pop.saturating_sub(queued) as i64,
                    "points_held" => ps.points_held as i64,
                    "ticket_drain" => ps.drain_per_min as f32 / 60.0,
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

        let mut squads = VarArray::new();
        for s in f.squads.iter().filter(|s| s.visible) {
            squads.push(&self.squad_dict(s, cam).to_variant());
        }

        let fac = &setup.factions[local_faction];
        let mut catalog = VarArray::new();
        for &t in &fac.roster {
            let ty = &setup.squad_types[t as usize];
            catalog.push(
                &vdict! {
                    "type" => role_name(ty.role),
                    "name" => ty.name.as_str(),
                    "cost" => ty.cost as i64,
                    "delay_s" => ty.call_in_ticks as f32 / hz,
                    "men" => ty.men as i64,
                }
                .to_variant(),
            );
        }
        let mut queue = VarArray::new();
        if let Some(me) = f.players.get(local) {
            for &(t, left) in &me.call_ins {
                let ty = &setup.squad_types[t as usize];
                queue.push(
                    &vdict! {
                        "type" => role_name(ty.role),
                        "remaining_s" => left as f32 / hz,
                        "total_s" => ty.call_in_ticks as f32 / hz,
                        "men" => ty.men as i64,
                    }
                    .to_variant(),
                );
            }
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
            if !c.weapon.is_empty() {
                d.set("weapon", c.weapon);
            }
            effects.push(&d.to_variant());
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

        vdict! {
            "tick" => f.tick as i64,
            "time_s" => f.tick as f32 / hz,
            "paused" => f.paused,
            "local_player" => local as i64,
            "map" => &map,
            "rules" => &rules_d,
            "players" => &players,
            "points" => &points,
            "squads" => &squads,
            "callin" => &vdict! {
                "catalog" => &catalog,
                "queue" => &queue,
                "queue_max" => MAX_CALL_IN_QUEUE as i64,
            },
            "events" => &events,
            "effects" => &effects,
            "result" => &vdict! {
                "status" => status,
                "winner" => winner,
                "reason" => reason,
            },
            "stats" => &stats,
        }
    }

    fn squad_dict(&self, s: &SquadView, cam: Option<&Gd<Camera3D>>) -> VarDictionary {
        let setup = &self.setup;
        let ty = &setup.squad_types[s.type_idx as usize];
        let men = s.men.max(1) as u32;
        let supp = s.suppression_sum as f32 / (100.0 * men as f32);
        let status = if s.order == OrderKind::Retreat {
            "retreating"
        } else if s.pinned as u32 * 2 >= men {
            "pinned"
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
        if let Some(cam) = cam {
            let top = pos + Vector3::new(0.0, 1.8, 0.0);
            if !cam.is_position_behind(top) {
                d.set("screen", cam.unproject_position(top));
            }
        }
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
        vdict! {
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
        }
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
                let pieces = split_polyline(&line, diggers.len(), MIN_DIG_PIECE_M);
                for (k, s) in diggers.iter().enumerate() {
                    // More squads than pieces: the extras share the last.
                    let piece = &pieces[k.min(pieces.len() - 1)];
                    out.push(dig_command(s.id, piece));
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
                let ty = data.get("type").map(|v| v.to_string()).unwrap_or_default();
                match call_in_type(&self.setup, self.local, &ty) {
                    Some(t) => out.push(CommandKind::CallIn { squad_type: t }),
                    None => {
                        godot_warn!("FrontlineMatch: no {ty:?} squad in the roster");
                    }
                }
            }
            "surrender" => out.push(CommandKind::Surrender),
            "pause" => {
                let p = data
                    .get("paused")
                    .and_then(|v| v.try_to::<bool>().ok())
                    .unwrap_or(false);
                self.sim.set_paused(p);
            }
            _ => {}
        }
        for c in out {
            self.sim.send(c);
        }
    }
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
